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
}

struct Session(String);

impl Api {
    async fn new(db: PgPool) -> Self {
        let state = AppState {
            db: db.clone(),
            settings: Arc::new(crate::state::Settings {
                cookie_secure: false,
                session_ttl: time::Duration::days(1),
            }),
            limiter: Arc::new(auth::LoginLimiter::default()),
        };
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
            app: super::router(state, None),
        }
    }

    async fn login(&self, username: &str) -> Session {
        let res = self
            .app
            .clone()
            .oneshot(
                Request::post("/api/auth/login")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        json!({"username": username, "password": "password-123"}).to_string(),
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

fn s(v: &Value) -> &str {
    v.as_str().unwrap()
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
