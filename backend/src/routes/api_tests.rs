//! API tests against a real PostgreSQL. `#[sqlx::test]` creates a fresh
//! database per test and runs the migrations; it needs DATABASE_URL.

use std::sync::Arc;

use axum::{
    body::{to_bytes, Body},
    http::{header, Request, StatusCode},
    Router,
};
use serde_json::{json, Value};
use sqlx::PgPool;
use tower::ServiceExt;

use crate::{auth, state::AppState};

struct Api {
    app: Router,
    latest_release: crate::updates::LatestRelease,
}

struct Session(String);

impl Api {
    async fn new(db: PgPool) -> Self {
        let state = AppState {
            db: db.clone(),
            settings: Arc::new(crate::state::Settings {
                cookie_secure: false,
                session_ttl: time::Duration::days(1),
                trusted_proxies: Default::default(),
            }),
            limiter: Arc::new(auth::LoginLimiter::default()),
            latest_release: Default::default(),
        };
        let latest_release = state.latest_release.clone();
        for (name, role) in [("boss", "owner"), ("mech", "mechanic")] {
            let hash = auth::hash_password("password-123".into()).await.unwrap();
            sqlx::query("INSERT INTO users (username, display_name, role, password_hash) VALUES ($1, $1, $2, $3)")
                .bind(name)
                .bind(role)
                .bind(hash)
                .execute(&db)
                .await
                .unwrap();
        }
        Api {
            latest_release,
            app: super::router(state, None),
        }
    }

    async fn login(&self, username: &str) -> Session {
        self.login_with(username, "password-123").await
    }

    async fn login_with(&self, username: &str, password: &str) -> Session {
        let res = self
            .app
            .clone()
            .oneshot(
                Request::post("/api/auth/login")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        json!({"username": username, "password": password}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let cookie = res.headers()[header::SET_COOKIE].to_str().unwrap();
        Session(cookie.split(';').next().unwrap().to_string())
    }

    async fn call(
        &self,
        s: &Session,
        method: &str,
        uri: &str,
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        let mut req = Request::builder()
            .method(method)
            .uri(uri)
            .header(header::COOKIE, &s.0);
        if body.is_some() {
            req = req.header(header::CONTENT_TYPE, "application/json");
        }
        let req = req
            .body(body.map_or_else(Body::empty, |b| Body::from(b.to_string())))
            .unwrap();
        let res = self.app.clone().oneshot(req).await.unwrap();
        let status = res.status();
        let bytes = to_bytes(res.into_body(), 1 << 20).await.unwrap();
        let json = if bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&bytes).unwrap()
        };
        (status, json)
    }
}

impl Api {
    /// A request with a raw body; returns status, headers and the body bytes.
    async fn raw(
        &self,
        s: &Session,
        method: &str,
        uri: &str,
        body: Vec<u8>,
    ) -> (StatusCode, axum::http::HeaderMap, Vec<u8>) {
        let req = Request::builder()
            .method(method)
            .uri(uri)
            .header(header::COOKIE, &s.0)
            .header(header::CONTENT_TYPE, "application/octet-stream")
            .body(Body::from(body))
            .unwrap();
        let res = self.app.clone().oneshot(req).await.unwrap();
        let status = res.status();
        let headers = res.headers().clone();
        let bytes = to_bytes(res.into_body(), 64 << 20).await.unwrap();
        (status, headers, bytes.to_vec())
    }
}

fn s(v: &Value) -> &str {
    v.as_str().unwrap()
}

#[sqlx::test]
async fn owners_see_available_updates(db: PgPool) {
    let api = Api::new(db).await;
    let boss = api.login("boss").await;
    let mech = api.login("mech").await;

    // Before the first check (or with it turned off): just the version.
    let (st, v) = api.call(&boss, "GET", "/api/version", None).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(s(&v["version"]), env!("CARGO_PKG_VERSION"));
    assert_eq!(v["update_available"], false);
    assert!(v.get("latest").is_none());

    *api.latest_release.write().unwrap() = Some(crate::updates::Release {
        version: "999.0.0".into(),
        url: "https://example.com/release".into(),
        published_at: None,
    });
    let (_, v) = api.call(&boss, "GET", "/api/version", None).await;
    assert_eq!(v["update_available"], true);
    assert_eq!(s(&v["latest"]["version"]), "999.0.0");

    // Mechanics cannot update the server: they only see the version.
    let (_, v) = api.call(&mech, "GET", "/api/version", None).await;
    assert_eq!(v["update_available"], false);
    assert!(v.get("latest").is_none());
}

#[sqlx::test]
async fn first_start_creates_owner_who_must_change_password(db: PgPool) {
    let password = auth::create_first_owner(&db).await.unwrap().unwrap();
    // Only the very first start: any account means nothing is created.
    assert!(auth::create_first_owner(&db).await.unwrap().is_none());

    let api = Api::new(db).await;
    let admin = api.login_with(auth::FIRST_OWNER, &password).await;

    let (st, me) = api.call(&admin, "GET", "/api/auth/me", None).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(
        (s(&me["role"]), me["must_change_password"].as_bool()),
        ("owner", Some(true))
    );

    // Everything else waits for a new password.
    for (method, uri) in [
        ("GET", "/api/vehicles"),
        ("GET", "/api/users"),
        ("PATCH", "/api/auth/me"),
    ] {
        let body = (method == "PATCH").then(|| json!({"locale": "ru"}));
        let (st, err) = api.call(&admin, method, uri, body).await;
        assert_eq!(
            (st, s(&err["error"])),
            (StatusCode::FORBIDDEN, "password_change_required"),
            "{uri}"
        );
    }
    let same = json!({"current_password": password, "new_password": password});
    let (st, err) = api
        .call(&admin, "POST", "/api/auth/password", Some(same))
        .await;
    assert_eq!(
        (st, s(&err["error"])),
        (StatusCode::UNPROCESSABLE_ENTITY, "password_unchanged")
    );

    let change = json!({"current_password": password, "new_password": "my-own-pass-1"});
    let (st, _) = api
        .call(&admin, "POST", "/api/auth/password", Some(change))
        .await;
    assert_eq!(st, StatusCode::NO_CONTENT);
    let (_, me) = api.call(&admin, "GET", "/api/auth/me", None).await;
    assert_eq!(me["must_change_password"].as_bool(), Some(false));
    let (st, _) = api.call(&admin, "GET", "/api/users", None).await;
    assert_eq!(st, StatusCode::OK);
}

#[sqlx::test]
async fn owner_resets_staff_password(db: PgPool) {
    let api = Api::new(db).await;
    let boss = api.login("boss").await;
    let mech = api.login("mech").await;
    let anon = Session("none=1".into());
    let login = |password: &str| {
        let body = json!({"username": "Mech", "password": password});
        api.call(&anon, "POST", "/api/auth/login", Some(body))
    };

    let (_, me) = api.call(&mech, "GET", "/api/auth/me", None).await;
    let mech_id = s(&me["id"]).to_string();
    let (_, me) = api.call(&boss, "GET", "/api/auth/me", None).await;
    let boss_id = s(&me["id"]).to_string();
    let reset = |id: &str| format!("/api/users/{id}/password");

    // The mechanic forgot the password and locked the account out.
    for _ in 0..5 {
        assert_eq!(login("wrong-guess").await.0, StatusCode::UNAUTHORIZED);
    }
    assert_eq!(login("password-123").await.0, StatusCode::TOO_MANY_REQUESTS);

    // Only owners reset passwords, and the length rule still applies.
    let body = json!({"password": "new-pass-456"});
    let (st, _) = api
        .call(&mech, "POST", &reset(&mech_id), Some(body.clone()))
        .await;
    assert_eq!(st, StatusCode::FORBIDDEN);
    let (st, err) = api
        .call(
            &boss,
            "POST",
            &reset(&mech_id),
            Some(json!({"password": "short"})),
        )
        .await;
    assert_eq!(
        (st, s(&err["error"])),
        (StatusCode::UNPROCESSABLE_ENTITY, "password_too_short")
    );

    // The reset signs the mechanic out everywhere and lifts the lockout.
    let (st, _) = api
        .call(&boss, "POST", &reset(&mech_id), Some(body.clone()))
        .await;
    assert_eq!(st, StatusCode::NO_CONTENT);
    let (st, _) = api.call(&mech, "GET", "/api/auth/me", None).await;
    assert_eq!(st, StatusCode::UNAUTHORIZED);
    assert_eq!(login("password-123").await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(login("new-pass-456").await.0, StatusCode::OK);

    // Your own password needs the current one (account page), not a reset.
    let (st, err) = api
        .call(&boss, "POST", &reset(&boss_id), Some(body.clone()))
        .await;
    assert_eq!(
        (st, s(&err["error"])),
        (
            StatusCode::UNPROCESSABLE_ENTITY,
            "cannot_reset_own_password"
        )
    );
    let unknown = reset("00000000-0000-0000-0000-000000000000");
    let (st, _) = api.call(&boss, "POST", &unknown, Some(body)).await;
    assert_eq!(st, StatusCode::NOT_FOUND);
}

#[sqlx::test]
async fn vehicle_search_and_identifiers(db: PgPool) {
    let api = Api::new(db).await;
    let mech = api.login("mech").await;

    let (st, _) = api
        .call(
            &mech,
            "POST",
            "/api/vehicles",
            Some(json!({"make": "Toyota"})),
        )
        .await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);

    let (st, v) = api
        .call(
            &mech,
            "POST",
            "/api/vehicles",
            Some(json!({"plate": " wa 12345 ", "vin": "jtd bt923-071234567", "make": "Toyota", "model": "Yaris", "odometer": 120500})),
        )
        .await;
    assert_eq!(st, StatusCode::CREATED, "{v}");
    assert_eq!(s(&v["plate"]), "WA 12345");
    assert_eq!(s(&v["vin"]), "JTDBT923071234567");
    assert_eq!(s(&v["odometer_unit"]), "km"); // business default
    assert_eq!(v["odometer"][0]["reading"], 120500);

    // A Japanese import with only a frame number.
    let (st, _) = api
        .call(
            &mech,
            "POST",
            "/api/vehicles",
            Some(json!({"frame_number": "gx100-1234567"})),
        )
        .await;
    assert_eq!(st, StatusCode::CREATED);

    let (st, e) = api
        .call(
            &mech,
            "POST",
            "/api/vehicles",
            Some(json!({"vin": "JTDBT923071234567"})),
        )
        .await;
    assert_eq!(st, StatusCode::CONFLICT);
    assert_eq!(e["error"], "vin_taken");

    // The same frame number typed without the dash is the same car.
    let (st, e) = api
        .call(
            &mech,
            "POST",
            "/api/vehicles",
            Some(json!({"frame_number": "GX1001234567"})),
        )
        .await;
    assert_eq!(st, StatusCode::CONFLICT);
    assert_eq!(e["error"], "frame_number_taken");

    for q in ["wa12345", "WA-123", "t923-07", "GX1001234567", "yaris"] {
        let (_, found) = api
            .call(&mech, "GET", &format!("/api/vehicles?q={q}"), None)
            .await;
        assert_eq!(found.as_array().unwrap().len(), 1, "query {q}: {found}");
    }
}

#[sqlx::test]
async fn mechanics_never_see_or_set_prices(db: PgPool) {
    let api = Api::new(db).await;
    let boss = api.login("boss").await;
    let mech = api.login("mech").await;

    let (_, v) = api
        .call(
            &mech,
            "POST",
            "/api/vehicles",
            Some(json!({"plate": "KR 1234", "odometer": 1000})),
        )
        .await;
    let vid = s(&v["id"]).to_string();

    // Lower odometer than last time: refused unless confirmed.
    let (st, e) = api
        .call(
            &mech,
            "POST",
            "/api/work-orders",
            Some(json!({"vehicle_id": vid, "odometer": 900})),
        )
        .await;
    assert_eq!(
        (st, s(&e["error"])),
        (StatusCode::CONFLICT, "odometer_lower_than_previous")
    );
    let (st, wo) = api
        .call(
            &mech,
            "POST",
            "/api/work-orders",
            Some(json!({"vehicle_id": vid, "odometer": 1500, "complaint": "Brakes squeak"})),
        )
        .await;
    assert_eq!(st, StatusCode::CREATED);
    // The refused attempt above must not use up an order number.
    assert_eq!(wo["number"], 1);
    let wid = s(&wo["id"]).to_string();
    let items = format!("/api/work-orders/{wid}/items");

    let (st, _) = api
        .call(
            &mech,
            "POST",
            &items,
            Some(json!({"description": "Pads", "unit_price_minor": 100})),
        )
        .await;
    assert_eq!(st, StatusCode::FORBIDDEN);

    let (st, wo) = api
        .call(
            &mech,
            "POST",
            &items,
            Some(json!({"description": "Replace front pads", "quantity_milli": 1500})),
        )
        .await;
    assert_eq!(st, StatusCode::CREATED);
    let item_id = s(&wo["items"][0]["id"]).to_string();

    let (st, _) = api
        .call(
            &mech,
            "PATCH",
            &format!("{items}/{item_id}"),
            Some(json!({"unit_price_minor": 5})),
        )
        .await;
    assert_eq!(st, StatusCode::FORBIDDEN);

    // Owner prices it: 1.5 x 120.33 = 180.495 -> 180.50
    let (st, wo) = api
        .call(
            &boss,
            "PATCH",
            &format!("{items}/{item_id}"),
            Some(json!({"unit_price_minor": 12033})),
        )
        .await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(wo["items"][0]["line_total_minor"], 18050);
    assert_eq!(wo["totals"]["total_minor"], 18050);
    assert_eq!(wo["totals"]["currency"], "PLN");

    let (_, wo) = api
        .call(&mech, "GET", &format!("/api/work-orders/{wid}"), None)
        .await;
    assert!(wo.get("totals").is_none(), "{wo}");
    assert!(wo["items"][0].get("unit_price_minor").is_none(), "{wo}");
    assert!(wo["items"][0].get("line_total_minor").is_none(), "{wo}");
    let (_, v) = api
        .call(&mech, "GET", &format!("/api/vehicles/{vid}"), None)
        .await;
    assert!(v["work_orders"][0].get("total_minor").is_none(), "{v}");
    let (_, v) = api
        .call(&boss, "GET", &format!("/api/vehicles/{vid}"), None)
        .await;
    assert_eq!(v["work_orders"][0]["total_minor"], 18050);

    // Closed orders cannot be changed until reopened.
    let (st, _) = api
        .call(
            &mech,
            "PATCH",
            &format!("/api/work-orders/{wid}"),
            Some(json!({"status": "done"})),
        )
        .await;
    assert_eq!(st, StatusCode::OK);
    let (st, e) = api
        .call(&mech, "POST", &items, Some(json!({"description": "More"})))
        .await;
    assert_eq!(
        (st, s(&e["error"])),
        (StatusCode::CONFLICT, "work_order_closed")
    );
}

#[sqlx::test]
async fn ownership_history_and_work_order_snapshot(db: PgPool) {
    let api = Api::new(db).await;
    let boss = api.login("boss").await;

    let (_, anna) = api
        .call(
            &boss,
            "POST",
            "/api/clients",
            Some(json!({"name": "Anna Demo", "phone": "+48 600 100 200"})),
        )
        .await;
    let (_, piotr) = api
        .call(
            &boss,
            "POST",
            "/api/clients",
            Some(json!({"name": "Piotr Demo"})),
        )
        .await;

    let (_, v) = api
        .call(
            &boss,
            "POST",
            "/api/vehicles",
            Some(json!({"plate": "PO 777", "owner_id": anna["id"]})),
        )
        .await;
    let vid = s(&v["id"]).to_string();
    assert_eq!(v["owner"]["name"], "Anna Demo");

    let (_, wo) = api
        .call(
            &boss,
            "POST",
            "/api/work-orders",
            Some(json!({"vehicle_id": vid})),
        )
        .await;

    let (st, v) = api
        .call(
            &boss,
            "PUT",
            &format!("/api/vehicles/{vid}/owner"),
            Some(json!({"client_id": piotr["id"]})),
        )
        .await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(v["owner"]["name"], "Piotr Demo");
    assert_eq!(v["owners"].as_array().unwrap().len(), 2);
    assert!(v["owners"][1]["ended_on"].is_string());

    // The old order still shows who brought the car back then.
    let (_, wo) = api
        .call(
            &boss,
            "GET",
            &format!("/api/work-orders/{}", s(&wo["id"])),
            None,
        )
        .await;
    assert_eq!(wo["client"]["name"], "Anna Demo");

    // Search by the current owner's phone digits.
    let (_, found) = api.call(&boss, "GET", "/api/vehicles?q=piotr", None).await;
    assert_eq!(found.as_array().unwrap().len(), 1);
    let (_, found) = api.call(&boss, "GET", "/api/vehicles?q=600100", None).await;
    assert_eq!(
        found.as_array().unwrap().len(),
        0,
        "past owner's phone must not match"
    );
}

#[sqlx::test]
async fn confirmed_lower_reading_becomes_the_latest(db: PgPool) {
    let api = Api::new(db).await;
    let mech = api.login("mech").await;
    let (_, v) = api
        .call(
            &mech,
            "POST",
            "/api/vehicles",
            Some(json!({"plate": "GD 555", "odometer": 120000})),
        )
        .await;
    let readings = format!("/api/vehicles/{}/odometer", s(&v["id"]));
    let (st, _) = api
        .call(
            &mech,
            "POST",
            &readings,
            Some(json!({"reading": 119000, "confirm_lower": true})),
        )
        .await;
    assert_eq!(st, StatusCode::CREATED);
    let (_, found) = api.call(&mech, "GET", "/api/vehicles?q=gd555", None).await;
    assert_eq!(found[0]["last_odometer"], 119000);
    // The correction is now the reference for the next check.
    let (st, _) = api
        .call(&mech, "POST", &readings, Some(json!({"reading": 119500})))
        .await;
    assert_eq!(st, StatusCode::CREATED);
}

#[sqlx::test]
async fn security_headers_do_not_depend_on_the_proxy(db: PgPool) {
    let api = Api::new(db).await;
    let res = api
        .app
        .clone()
        .oneshot(Request::get("/api/nope").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let h = res.headers();
    assert_eq!(h["referrer-policy"], "no-referrer");
    assert_eq!(h["x-content-type-options"], "nosniff");
    assert_eq!(h["x-frame-options"], "DENY");
    // Plain HTTP (cookie_secure = false): HSTS would pin localhost to HTTPS.
    assert!(h.get("strict-transport-security").is_none());
}

#[sqlx::test]
async fn parts_catalogue_search_and_prices(db: PgPool) {
    let api = Api::new(db).await;
    let mech = api.login("mech").await;
    let boss = api.login("boss").await;

    let (st, _) = api
        .call(
            &mech,
            "POST",
            "/api/parts",
            Some(json!({"name": "Oil filter", "sell_price_minor": 4500})),
        )
        .await;
    assert_eq!(st, StatusCode::FORBIDDEN, "mechanics cannot set prices");

    let (st, filter) = api
        .call(
            &mech,
            "POST",
            "/api/parts",
            Some(json!({
                "name": "Oil filter", "brand": "Toyota", "part_number": "04152-yzza1",
                "barcodes": ["4987654321098", "4987654321098"], "stock_milli": 3000,
                "location": "A-3"
            })),
        )
        .await;
    assert_eq!(st, StatusCode::CREATED, "{filter}");
    assert_eq!(s(&filter["part_number"]), "04152-YZZA1");
    assert_eq!(filter["barcodes"], json!(["4987654321098"]));
    assert_eq!(filter["stock_milli"], 3000);
    let fid = s(&filter["id"]).to_string();

    // Same brand and number: the same part.
    let (st, err) = api
        .call(
            &mech,
            "POST",
            "/api/parts",
            Some(json!({"name": "Filter", "brand": "toyota", "part_number": "04152 YZZA1"})),
        )
        .await;
    assert_eq!(st, StatusCode::CONFLICT);
    assert_eq!(s(&err["error"]), "part_number_taken");

    // A barcode belongs to one part.
    let (_, other) = api
        .call(
            &mech,
            "POST",
            "/api/parts",
            Some(json!({"name": "Bulb H7"})),
        )
        .await;
    let barcodes = format!("/api/parts/{}/barcodes", s(&other["id"]));
    let (st, err) = api
        .call(
            &mech,
            "POST",
            &barcodes,
            Some(json!({"barcode": "4987654321098"})),
        )
        .await;
    assert_eq!(st, StatusCode::CONFLICT);
    assert_eq!(s(&err["error"]), "barcode_taken");

    // Search ignores separators in part numbers; a scanned barcode is flagged.
    let (_, found) = api
        .call(&mech, "GET", "/api/parts?q=04152yzza1", None)
        .await;
    assert_eq!(s(&found[0]["id"]), fid);
    let (_, found) = api
        .call(&mech, "GET", "/api/parts?q=4987654321098", None)
        .await;
    assert_eq!(found.as_array().unwrap().len(), 1);
    assert_eq!(found[0]["barcode_match"], true);

    // Prices: owners set and see them, mechanics never receive them.
    let part = format!("/api/parts/{fid}");
    let (st, p) = api
        .call(
            &boss,
            "PUT",
            &part,
            Some(
                json!({"name": "Oil filter", "brand": "Toyota", "part_number": "04152-YZZA1",
                        "location": "A-3", "min_stock_milli": 2000, "sell_price_minor": 4500}),
            ),
        )
        .await;
    assert_eq!(st, StatusCode::OK, "{p}");
    assert_eq!(p["sell_price_minor"], 4500);
    // A mechanic editing the part leaves the price alone.
    let (st, p) = api
        .call(
            &mech,
            "PUT",
            &part,
            Some(
                json!({"name": "Oil filter", "brand": "Toyota", "part_number": "04152-YZZA1",
                        "location": "B-1", "min_stock_milli": 2000}),
            ),
        )
        .await;
    assert_eq!(st, StatusCode::OK, "{p}");
    assert!(p.get("sell_price_minor").is_none());
    let (_, p) = api.call(&boss, "GET", &part, None).await;
    assert_eq!(p["sell_price_minor"], 4500);
    assert_eq!(s(&p["location"]), "B-1");
    let (_, found) = api.call(&mech, "GET", "/api/parts?q=oil", None).await;
    assert!(found[0].get("sell_price_minor").is_none());
}

#[sqlx::test]
async fn stock_follows_work_orders(db: PgPool) {
    let api = Api::new(db).await;
    let mech = api.login("mech").await;
    let boss = api.login("boss").await;

    let (_, part) = api
        .call(
            &boss,
            "POST",
            "/api/parts",
            Some(
                json!({"name": "Engine oil 5W-30", "unit": "l", "min_stock_milli": 5000,
                        "sell_price_minor": 3500, "stock_milli": 8000}),
            ),
        )
        .await;
    let pid = s(&part["id"]).to_string();
    let part_url = format!("/api/parts/{pid}");
    let stock = |p: &Value| p["stock_milli"].as_i64().unwrap();

    let (_, v) = api
        .call(
            &mech,
            "POST",
            "/api/vehicles",
            Some(json!({"plate": "TEST 1"})),
        )
        .await;
    let (_, wo) = api
        .call(
            &mech,
            "POST",
            "/api/work-orders",
            Some(json!({"vehicle_id": s(&v["id"])})),
        )
        .await;
    let wo_url = format!("/api/work-orders/{}", s(&wo["id"]));
    let items = format!("{wo_url}/items");

    // A mechanic adds 4.5 l of oil: stock goes down, the catalogue price applies.
    let (st, wo) = api
        .call(
            &mech,
            "POST",
            &items,
            Some(json!({"part_id": pid, "quantity_milli": 4500})),
        )
        .await;
    assert_eq!(st, StatusCode::CREATED, "{wo}");
    let item = &wo["items"][0];
    assert_eq!(s(&item["kind"]), "part");
    assert_eq!(s(&item["description"]), "Engine oil 5W-30");
    assert_eq!(s(&item["unit"]), "l");
    assert!(item.get("unit_price_minor").is_none());
    let item_url = format!("{items}/{}", s(&item["id"]));
    let (_, p) = api.call(&mech, "GET", &part_url, None).await;
    assert_eq!(stock(&p), 3500);
    assert_eq!(s(&p["movements"][0]["kind"]), "consumption");
    assert_eq!(p["movements"][0]["work_order_number"], wo["number"]);
    let (_, wo_boss) = api.call(&boss, "GET", &wo_url, None).await;
    assert_eq!(wo_boss["items"][0]["unit_price_minor"], 3500);
    assert_eq!(wo_boss["totals"]["total_minor"], 15750);

    // Low stock list: 3.5 l is below the 5 l minimum.
    let (_, low) = api.call(&mech, "GET", "/api/parts?filter=low", None).await;
    assert_eq!(s(&low[0]["id"]), pid);

    // Changing the quantity changes the consumption.
    api.call(
        &mech,
        "PATCH",
        &item_url,
        Some(json!({"quantity_milli": 5000})),
    )
    .await;
    let (_, p) = api.call(&mech, "GET", &part_url, None).await;
    assert_eq!(stock(&p), 3000);

    // Cancelling returns the parts; reopening takes them again.
    api.call(
        &mech,
        "PATCH",
        &wo_url,
        Some(json!({"status": "cancelled"})),
    )
    .await;
    let (_, p) = api.call(&mech, "GET", &part_url, None).await;
    assert_eq!(stock(&p), 8000);
    api.call(&mech, "PATCH", &wo_url, Some(json!({"status": "open"})))
        .await;
    let (_, p) = api.call(&mech, "GET", &part_url, None).await;
    assert_eq!(stock(&p), 3000);

    // Deleting the line returns the parts.
    let (st, _) = api.call(&mech, "DELETE", &item_url, None).await;
    assert_eq!(st, StatusCode::OK);
    let (_, p) = api.call(&mech, "GET", &part_url, None).await;
    assert_eq!(stock(&p), 8000);

    // Deliveries: purchase cost is for owners only.
    let movements = format!("{part_url}/movements");
    let (st, _) = api
        .call(
            &mech,
            "POST",
            &movements,
            Some(json!({"kind": "receipt", "quantity_milli": 4000, "unit_cost_minor": 2000})),
        )
        .await;
    assert_eq!(st, StatusCode::FORBIDDEN);
    let (st, p) = api
        .call(
            &boss,
            "POST",
            &movements,
            Some(json!({"kind": "receipt", "quantity_milli": 4000, "unit_cost_minor": 2000})),
        )
        .await;
    assert_eq!(st, StatusCode::CREATED, "{p}");
    assert_eq!(stock(&p), 12000);
    assert_eq!(p["movements"][0]["unit_cost_minor"], 2000);
    let (_, p) = api.call(&mech, "GET", &part_url, None).await;
    assert!(p["movements"][0].get("unit_cost_minor").is_none());

    // A stock count records the difference.
    let (_, p) = api
        .call(
            &mech,
            "POST",
            &movements,
            Some(json!({"kind": "count", "quantity_milli": 11000})),
        )
        .await;
    assert_eq!(stock(&p), 11000);
    assert_eq!(s(&p["movements"][0]["kind"]), "adjustment");
    assert_eq!(p["movements"][0]["quantity_milli"], -1000);

    // A part used in a work order cannot be deleted.
    api.call(&mech, "POST", &items, Some(json!({"part_id": pid})))
        .await;
    let (st, err) = api.call(&boss, "DELETE", &part_url, None).await;
    assert_eq!(st, StatusCode::CONFLICT);
    assert_eq!(s(&err["error"]), "part_in_use");
}

#[sqlx::test]
async fn used_parts_are_separate_items(db: PgPool) {
    let api = Api::new(db).await;
    let mech = api.login("mech").await;
    let new_part = json!({"name": "Alternator", "brand": "Denso", "part_number": "104210-3170"});

    let (st, new) = api
        .call(&mech, "POST", "/api/parts", Some(new_part.clone()))
        .await;
    assert_eq!(st, StatusCode::CREATED, "{new}");
    assert_eq!(s(&new["condition"]), "new");

    // The same part number used: its own item with its own stock.
    let mut used_part = new_part.clone();
    used_part["condition"] = json!("used");
    used_part["stock_milli"] = json!(1000);
    let (st, used) = api
        .call(&mech, "POST", "/api/parts", Some(used_part.clone()))
        .await;
    assert_eq!(st, StatusCode::CREATED, "{used}");
    assert_eq!(s(&used["condition"]), "used");
    assert_ne!(used["id"], new["id"]);

    // But not twice in the same condition.
    let (st, err) = api.call(&mech, "POST", "/api/parts", Some(used_part)).await;
    assert_eq!(st, StatusCode::CONFLICT);
    assert_eq!(s(&err["error"]), "part_number_taken");

    let (st, err) = api
        .call(
            &mech,
            "POST",
            "/api/parts",
            Some(json!({"name": "X", "condition": "refurbished"})),
        )
        .await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(s(&err["error"]), "invalid_condition");

    // Search finds both; a work order line says which one went in.
    let (_, found) = api
        .call(&mech, "GET", "/api/parts?q=1042103170", None)
        .await;
    assert_eq!(found.as_array().unwrap().len(), 2);

    let (_, v) = api
        .call(
            &mech,
            "POST",
            "/api/vehicles",
            Some(json!({"plate": "TEST 2"})),
        )
        .await;
    let (_, wo) = api
        .call(
            &mech,
            "POST",
            "/api/work-orders",
            Some(json!({"vehicle_id": s(&v["id"])})),
        )
        .await;
    let (_, wo) = api
        .call(
            &mech,
            "POST",
            &format!("/api/work-orders/{}/items", s(&wo["id"])),
            Some(json!({"part_id": s(&used["id"])})),
        )
        .await;
    assert_eq!(s(&wo["items"][0]["condition"]), "used");
}

#[sqlx::test]
async fn deliveries_put_parts_on_stock_when_posted(db: PgPool) {
    let api = Api::new(db).await;
    let boss = api.login("boss").await;
    let mech = api.login("mech").await;

    // Deliveries are owner-only: they are all purchase prices.
    let (st, _) = api.call(&mech, "GET", "/api/deliveries", None).await;
    assert_eq!(st, StatusCode::FORBIDDEN);
    let (st, _) = api.call(&mech, "GET", "/api/suppliers", None).await;
    assert_eq!(st, StatusCode::FORBIDDEN);

    let (st, sup) = api
        .call(
            &boss,
            "POST",
            "/api/suppliers",
            Some(json!({"name": "Demo Parts Sp. z o.o.", "tax_id": "PL 123-456-78-90"})),
        )
        .await;
    assert_eq!(st, StatusCode::CREATED, "{sup}");
    assert_eq!(s(&sup["tax_id"]), "1234567890");
    let (st, v) = api
        .call(
            &boss,
            "POST",
            "/api/suppliers",
            Some(json!({"name": "Same NIP", "tax_id": "1234567890"})),
        )
        .await;
    assert_eq!((st, s(&v["error"])), (StatusCode::CONFLICT, "tax_id_taken"));
    let (_, found) = api
        .call(&boss, "GET", "/api/suppliers?q=123-456", None)
        .await;
    assert_eq!(found.as_array().unwrap().len(), 1);

    let (_, filter) = api
        .call(
            &boss,
            "POST",
            "/api/parts",
            Some(
                json!({"name": "Oil filter", "brand": "Demo", "part_number": "OF-1",
                        "sell_price_minor": 3000, "stock_milli": 2000, "unit_cost_minor": 1500}),
            ),
        )
        .await;
    let fid = s(&filter["id"]).to_string();
    assert_eq!(filter["last_cost_minor"], 1500);

    // Markup 30 %: suggested prices appear on the lines.
    let (_, set) = api
        .call(
            &boss,
            "PATCH",
            "/api/settings",
            Some(json!({"markup_percent": 30})),
        )
        .await;
    assert_eq!(set["markup_percent"], 30);
    let (_, set) = api.call(&mech, "GET", "/api/settings", None).await;
    assert!(set.get("markup_percent").is_none());

    let (st, d) = api
        .call(
            &boss,
            "POST",
            "/api/deliveries",
            Some(
                json!({"supplier_id": s(&sup["id"]), "invoice_number": "FV 1/11/2026",
                        "invoice_date": "2026-11-03", "invoice_total_net_minor": 6000}),
            ),
        )
        .await;
    assert_eq!(st, StatusCode::CREATED, "{d}");
    assert_eq!(s(&d["status"]), "draft");
    let url = format!("/api/deliveries/{}", s(&d["id"]));

    // The same invoice of the same supplier cannot be entered twice.
    let (st, v) = api
        .call(
            &boss,
            "POST",
            "/api/deliveries",
            Some(json!({"supplier_id": s(&sup["id"]), "invoice_number": "fv 1/11/2026"})),
        )
        .await;
    assert_eq!(
        (st, s(&v["error"])),
        (StatusCode::CONFLICT, "invoice_already_entered")
    );

    // Two filters at 20.00 net, and a line not matched to a part yet.
    let (st, d) = api
        .call(
            &boss,
            "POST",
            &format!("{url}/items"),
            Some(json!({"part_id": fid, "quantity_milli": 2000, "unit_cost_minor": 2000})),
        )
        .await;
    assert_eq!(st, StatusCode::CREATED, "{d}");
    let line = &d["items"][0];
    assert_eq!(line["previous_cost_minor"], 1500);
    assert_eq!(line["total_net_minor"], 4000);
    // 20.00 + 30 % + 23 % VAT = 31.98, rounded up to 32.00.
    assert_eq!(line["suggested_price_minor"], 3200);
    let (_, d) = api
        .call(
            &boss,
            "POST",
            &format!("{url}/items"),
            Some(
                json!({"description": "Wiper blade 600 mm", "part_number": "WB600",
                        "quantity_milli": 1000, "unit_cost_minor": 2000, "vat_percent": 23}),
            ),
        )
        .await;
    assert_eq!(d["totals"]["net_minor"], 6000);
    assert_eq!(d["totals"]["gross_minor"], 7380);
    let wiper_line = s(&d["items"][1]["id"]).to_string();

    let (st, v) = api
        .call(&boss, "POST", &format!("{url}/post"), Some(json!({})))
        .await;
    assert_eq!(
        (st, s(&v["error"])),
        (StatusCode::CONFLICT, "delivery_unmatched_lines")
    );

    let (_, wiper) = api
        .call(
            &boss,
            "POST",
            "/api/parts",
            Some(json!({"name": "Wiper blade 600 mm", "part_number": "WB600"})),
        )
        .await;
    let wid = s(&wiper["id"]).to_string();
    api.call(
        &boss,
        "PATCH",
        &format!("{url}/items/{wiper_line}"),
        Some(json!({"part_id": wid})),
    )
    .await;

    // Nothing is on stock before posting.
    let (_, p) = api
        .call(&boss, "GET", &format!("/api/parts/{fid}"), None)
        .await;
    assert_eq!(p["stock_milli"], 2000);

    let (st, d) = api
        .call(
            &boss,
            "POST",
            &format!("{url}/post"),
            Some(json!({"prices": [{"part_id": fid, "sell_price_minor": 3200}]})),
        )
        .await;
    assert_eq!(st, StatusCode::OK, "{d}");
    assert_eq!(s(&d["status"]), "posted");
    let (_, p) = api
        .call(&boss, "GET", &format!("/api/parts/{fid}"), None)
        .await;
    assert_eq!(p["stock_milli"], 4000);
    assert_eq!(p["sell_price_minor"], 3200);
    assert_eq!(p["last_cost_minor"], 2000);
    assert_eq!(p["last_cost_vat_percent"], 23);
    assert_eq!(p["movements"][0]["delivery_number"], d["number"]);
    let (_, p) = api
        .call(&mech, "GET", &format!("/api/parts/{fid}"), None)
        .await;
    assert!(p.get("last_cost_minor").is_none());
    assert!(p["movements"][0].get("unit_cost_minor").is_none());

    // A posted delivery is locked; the header can still be corrected.
    let (st, v) = api
        .call(&boss, "DELETE", &format!("{url}/items/{wiper_line}"), None)
        .await;
    assert_eq!(
        (st, s(&v["error"])),
        (StatusCode::CONFLICT, "delivery_posted")
    );
    let (st, _) = api.call(&boss, "DELETE", &url, None).await;
    assert_eq!(st, StatusCode::CONFLICT);
    let (st, d) = api
        .call(&boss, "PATCH", &url, Some(json!({"note": "Paid by card"})))
        .await;
    assert_eq!((st, s(&d["note"])), (StatusCode::OK, "Paid by card"));
    // A part bought in a delivery cannot be deleted.
    let (st, v) = api
        .call(&boss, "DELETE", &format!("/api/parts/{wid}"), None)
        .await;
    assert_eq!((st, s(&v["error"])), (StatusCode::CONFLICT, "part_in_use"));

    // Reopening takes the parts off stock again.
    let (st, _) = api
        .call(&boss, "POST", &format!("{url}/reopen"), None)
        .await;
    assert_eq!(st, StatusCode::OK);
    let (_, p) = api
        .call(&boss, "GET", &format!("/api/parts/{fid}"), None)
        .await;
    assert_eq!(p["stock_milli"], 2000);
    let (_, list) = api
        .call(&boss, "GET", "/api/deliveries?status=draft", None)
        .await;
    assert_eq!(list[0]["item_count"], 2);
    assert_eq!(list[0]["total_net_minor"], 6000);
    let (st, _) = api.call(&boss, "DELETE", &url, None).await;
    assert_eq!(st, StatusCode::NO_CONTENT);
}

#[sqlx::test]
async fn delivery_files_are_checked_and_never_run(db: PgPool) {
    let api = Api::new(db).await;
    let boss = api.login("boss").await;
    let mech = api.login("mech").await;
    let (_, d) = api
        .call(&boss, "POST", "/api/deliveries", Some(json!({})))
        .await;
    let url = format!("/api/deliveries/{}/files", s(&d["id"]));

    let pdf = b"%PDF-1.7\n% demo invoice\n".to_vec();
    let (st, _, body) = api
        .raw(
            &boss,
            "POST",
            &format!("{url}?name=FV%201%2F2026.pdf"),
            pdf.clone(),
        )
        .await;
    assert_eq!(st, StatusCode::CREATED);
    let d: Value = serde_json::from_slice(&body).unwrap();
    let file = &d["files"][0];
    assert_eq!(s(&file["file_name"]), "2026.pdf");
    assert_eq!(s(&file["content_type"]), "application/pdf");
    let file_url = format!("{url}/{}", s(&file["id"]));

    let (st, h, body) = api.raw(&boss, "GET", &file_url, vec![]).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(body, pdf);
    assert_eq!(h[header::CONTENT_TYPE], "application/pdf");
    assert!(h[header::CONTENT_DISPOSITION]
        .to_str()
        .unwrap()
        .starts_with("inline"));
    // The browser's PDF viewer refuses to work in a sandbox.
    assert!(h.get(header::CONTENT_SECURITY_POLICY).is_none());

    // XML (KSeF) is only ever a download.
    let xml = b"<?xml version=\"1.0\"?><Faktura></Faktura>".to_vec();
    let (st, _, body) = api
        .raw(&boss, "POST", &format!("{url}?name=ksef.xml"), xml)
        .await;
    assert_eq!(st, StatusCode::CREATED);
    let d: Value = serde_json::from_slice(&body).unwrap();
    let xml_url = format!("{url}/{}", s(&d["files"][1]["id"]));
    let (_, h, _) = api.raw(&boss, "GET", &xml_url, vec![]).await;
    assert!(h[header::CONTENT_DISPOSITION]
        .to_str()
        .unwrap()
        .starts_with("attachment"));
    assert_eq!(
        h[header::CONTENT_SECURITY_POLICY],
        "sandbox; default-src 'none'"
    );

    // Anything else is refused, whatever its name says.
    let (st, _, body) = api
        .raw(
            &boss,
            "POST",
            &format!("{url}?name=x.pdf"),
            b"MZ\x90\x00".to_vec(),
        )
        .await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        serde_json::from_slice::<Value>(&body).unwrap()["error"],
        "unsupported_file_type"
    );
    let (st, _, _) = api.raw(&mech, "GET", &file_url, vec![]).await;
    assert_eq!(st, StatusCode::FORBIDDEN);
    let (st, _, _) = api
        .raw(
            &boss,
            "POST",
            &url,
            vec![b'%'; super::deliveries::MAX_FILE_BYTES + 1],
        )
        .await;
    assert_eq!(st, StatusCode::PAYLOAD_TOO_LARGE);

    let (st, d) = api.call(&boss, "DELETE", &file_url, None).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(d["files"].as_array().unwrap().len(), 1);
}

#[sqlx::test]
async fn monthly_report_counts_purchases_use_and_stock(db: PgPool) {
    let api = Api::new(db.clone()).await;
    let boss = api.login("boss").await;
    let mech = api.login("mech").await;

    // First stocktaking: 10 pads found on the shelf, valued at 50.00 each.
    let (_, pads) = api
        .call(
            &boss,
            "POST",
            "/api/parts",
            Some(json!({"name": "Brake pads", "sell_price_minor": 9000})),
        )
        .await;
    let pid = s(&pads["id"]).to_string();
    let (st, _) = api
        .call(
            &boss,
            "POST",
            &format!("/api/parts/{pid}/movements"),
            Some(json!({"kind": "count", "quantity_milli": 10000, "unit_cost_minor": 5000})),
        )
        .await;
    assert_eq!(st, StatusCode::CREATED);
    // Mechanics cannot set costs.
    let (st, _) = api
        .call(
            &mech,
            "POST",
            &format!("/api/parts/{pid}/movements"),
            Some(json!({"kind": "count", "quantity_milli": 10000, "unit_cost_minor": 1})),
        )
        .await;
    assert_eq!(st, StatusCode::FORBIDDEN);

    // A delivery of 4 more at 60.00, dated today.
    let today = time::OffsetDateTime::now_utc().date();
    let (_, d) = api
        .call(
            &boss,
            "POST",
            "/api/deliveries",
            Some(json!({"invoice_date": today.to_string()})),
        )
        .await;
    let url = format!("/api/deliveries/{}", s(&d["id"]));
    api.call(
        &boss,
        "POST",
        &format!("{url}/items"),
        Some(json!({"part_id": pid, "quantity_milli": 4000, "unit_cost_minor": 6000})),
    )
    .await;
    api.call(&boss, "POST", &format!("{url}/post"), Some(json!({})))
        .await;

    // 2 pads go into a work order at 90.00.
    let (_, v) = api
        .call(
            &mech,
            "POST",
            "/api/vehicles",
            Some(json!({"plate": "TEST 2"})),
        )
        .await;
    let (_, wo) = api
        .call(
            &mech,
            "POST",
            "/api/work-orders",
            Some(json!({"vehicle_id": s(&v["id"])})),
        )
        .await;
    api.call(
        &mech,
        "POST",
        &format!("/api/work-orders/{}/items", s(&wo["id"])),
        Some(json!({"part_id": pid, "quantity_milli": 2000})),
    )
    .await;

    let (st, _) = api
        .call(
            &mech,
            "GET",
            &format!("/api/reports/months?year={}", today.year()),
            None,
        )
        .await;
    assert_eq!(st, StatusCode::FORBIDDEN);
    let (st, v) = api
        .call(
            &boss,
            "GET",
            "/api/reports/months?year=2026&tz=Mars/Base",
            None,
        )
        .await;
    assert_eq!(
        (st, s(&v["error"])),
        (StatusCode::UNPROCESSABLE_ENTITY, "invalid_time_zone")
    );

    let (st, r) = api
        .call(
            &boss,
            "GET",
            &format!("/api/reports/months?year={}&tz=UTC", today.year()),
            None,
        )
        .await;
    assert_eq!(st, StatusCode::OK, "{r}");
    let months = r["months"].as_array().unwrap();
    assert_eq!(months.len(), today.month() as usize);
    let m = months.last().unwrap();
    // The stocktaking is not a purchase; the delivery is.
    assert_eq!(m["purchased_net_minor"], 24000);
    // Used at the latest cost before the work order (60.00) and sold at 90.00.
    assert_eq!(m["used_cost_minor"], 12000);
    assert_eq!(m["used_sale_minor"], 18000);
    assert_eq!(m["used_lines"], 1);
    // 12 pads left at 60.00.
    assert_eq!(m["stock_end"]["value_minor"], 72000);
    assert_eq!(m["stock_end"]["unvalued_parts"], 0);

    let (st, r) = api
        .call(&boss, "GET", "/api/reports/stock?tz=Europe/Warsaw", None)
        .await;
    assert_eq!(st, StatusCode::OK, "{r}");
    assert_eq!(r["rows"][0]["quantity_milli"], 12000);
    assert_eq!(r["rows"][0]["value_minor"], 72000);
    assert_eq!(r["total"]["value_minor"], 72000);

    // Stock on a day before anything happened is empty.
    let (_, r) = api
        .call(&boss, "GET", "/api/reports/stock?date=2020-01-01", None)
        .await;
    assert_eq!(r["rows"].as_array().unwrap().len(), 0);
}
