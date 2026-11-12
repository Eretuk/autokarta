//! Spare parts: catalogue, barcodes and the stock ledger.
//!
//! The stock level is the sum of `stock_movements`, never a stored counter.
//! It may go below zero: a mechanic fitting a part whose delivery was never
//! recorded must not be blocked; the owner sees the negative stock instead.
//! Purchase costs and sale prices are financial data: owners only.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sqlx::PgConnection;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::{
    auth::{CurrentUser, Owner},
    error::{conflict_on, AppError, AppResult},
    routes::{deserialize_some, like_pattern},
    state::AppState,
    validate,
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(search).post(create))
        .route("/{id}", get(detail).put(update).delete(remove))
        .route("/{id}/barcodes", post(add_barcode).delete(remove_barcode))
        .route("/{id}/movements", post(add_movement))
}

const UNIQUE: &[(&str, &str)] = &[
    ("parts_brand_number_idx", "part_number_taken"),
    ("part_barcodes_pkey", "barcode_taken"),
];

/// Current stock of `p.id`, in thousandths of the part's unit.
const STOCK: &str =
    "COALESCE((SELECT sum(m.quantity_milli) FROM stock_movements m WHERE m.part_id = p.id), 0)::bigint";

// ---------- search ----------

#[derive(Deserialize)]
struct SearchQuery {
    #[serde(default)]
    q: String,
    /// "low": only parts below their minimum stock.
    filter: Option<String>,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct PartSummary {
    id: Uuid,
    name: String,
    brand: Option<String>,
    part_number: Option<String>,
    condition: String,
    unit: String,
    location: Option<String>,
    stock_milli: i64,
    min_stock_milli: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    sell_price_minor: Option<i64>,
    /// The query is exactly one of this part's barcodes: a scan found it.
    barcode_match: bool,
}

async fn search(
    State(state): State<AppState>,
    user: CurrentUser,
    Query(query): Query<SearchQuery>,
) -> AppResult<Json<Vec<PartSummary>>> {
    let q = query.q.trim();
    let low = match query.filter.as_deref() {
        None | Some("all") => false,
        Some("low") => true,
        Some(_) => return Err(AppError::Validation("invalid_filter")),
    };
    let key = validate::part_number_key(q);
    let mut rows = sqlx::query_as::<_, PartSummary>(&format!(
        "SELECT * FROM (
            SELECT p.id, p.name, p.brand, p.part_number, p.condition, p.unit, p.location,
                   p.min_stock_milli, p.sell_price_minor, {STOCK} AS stock_milli,
                   EXISTS (SELECT 1 FROM part_barcodes b WHERE b.part_id = p.id AND b.barcode = $1)
                       AS barcode_match,
                   regexp_replace(coalesce(p.part_number, ''), '[\\s.-]', '', 'g') = $4 AS number_match
              FROM parts p
             WHERE $1 = ''
                OR lower(p.name) LIKE $2
                OR lower(coalesce(p.brand, '')) LIKE $2
                OR ($4 <> '' AND regexp_replace(p.part_number, '[\\s.-]', '', 'g') LIKE $3)
                OR EXISTS (SELECT 1 FROM part_barcodes b WHERE b.part_id = p.id AND b.barcode = $1)
         ) s
         WHERE NOT $5 OR (s.min_stock_milli > 0 AND s.stock_milli < s.min_stock_milli)
         ORDER BY s.barcode_match DESC, s.number_match DESC, lower(s.name), s.brand, s.condition
         LIMIT 200"
    ))
    .bind(q)
    .bind(like_pattern(&q.to_lowercase()))
    .bind(like_pattern(&key))
    .bind(&key)
    .bind(low)
    .fetch_all(&state.db)
    .await?;
    if !user.is_owner() {
        rows.iter_mut().for_each(|r| r.sell_price_minor = None);
    }
    Ok(Json(rows))
}

// ---------- create / update / delete ----------

#[derive(Deserialize)]
struct PartInput {
    name: String,
    brand: Option<String>,
    part_number: Option<String>,
    /// "new" (default) or "used".
    condition: Option<String>,
    unit: Option<String>,
    location: Option<String>,
    min_stock_milli: Option<i64>,
    note: Option<String>,
    /// Absent: unchanged (and allowed for mechanics). null: no price.
    #[serde(default, deserialize_with = "deserialize_some")]
    sell_price_minor: Option<Option<i64>>,
}

struct CleanPart {
    name: String,
    brand: Option<String>,
    part_number: Option<String>,
    condition: String,
    unit: String,
    location: Option<String>,
    min_stock_milli: i64,
    note: Option<String>,
    sell_price_minor: Option<Option<i64>>,
}

fn v<T>(r: Result<T, &'static str>) -> AppResult<T> {
    r.map_err(AppError::Validation)
}

fn clean(req: &PartInput, user: &CurrentUser) -> AppResult<CleanPart> {
    if req.sell_price_minor.is_some() && !user.is_owner() {
        return Err(AppError::Forbidden);
    }
    Ok(CleanPart {
        name: v(validate::part_name(&req.name))?,
        brand: v(validate::optional_text(req.brand.as_deref(), 60))?,
        part_number: v(validate::part_number(req.part_number.as_deref()))?,
        condition: v(validate::one_of(
            req.condition.as_deref().unwrap_or("new"),
            validate::PART_CONDITIONS,
            "invalid_condition",
        ))?,
        unit: v(validate::one_of(
            req.unit.as_deref().unwrap_or("pcs"),
            validate::PART_UNITS,
            "invalid_unit",
        ))?,
        location: v(validate::optional_text(req.location.as_deref(), 60))?,
        min_stock_milli: v(validate::stock_quantity_milli(
            req.min_stock_milli.unwrap_or(0),
        ))?,
        note: v(validate::optional_text(req.note.as_deref(), 2000))?,
        sell_price_minor: req
            .sell_price_minor
            .map(validate::price_minor)
            .transpose()
            .map_err(AppError::Validation)?,
    })
}

#[derive(Deserialize)]
struct CreatePart {
    #[serde(flatten)]
    part: PartInput,
    #[serde(default)]
    barcodes: Vec<String>,
    /// How many are on the shelf right now; recorded as a stock count.
    stock_milli: Option<i64>,
    /// Net purchase cost of that stock, for its value. Owners only.
    unit_cost_minor: Option<i64>,
}

async fn create(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(req): Json<CreatePart>,
) -> AppResult<(StatusCode, Json<PartDetail>)> {
    let p = clean(&req.part, &user)?;
    if req.unit_cost_minor.is_some() && !user.is_owner() {
        return Err(AppError::Forbidden);
    }
    let cost = validate::price_minor(req.unit_cost_minor).map_err(AppError::Validation)?;
    let mut barcodes = req
        .barcodes
        .iter()
        .map(|b| validate::barcode(b))
        .collect::<Result<Vec<_>, _>>()
        .map_err(AppError::Validation)?;
    barcodes.sort();
    barcodes.dedup();
    let stock = req
        .stock_milli
        .map(validate::stock_quantity_milli)
        .transpose()
        .map_err(AppError::Validation)?;

    let mut tx = state.db.begin().await?;
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO parts (name, brand, part_number, condition, unit, location, min_stock_milli, note, sell_price_minor)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
         RETURNING id",
    )
    .bind(&p.name)
    .bind(&p.brand)
    .bind(&p.part_number)
    .bind(&p.condition)
    .bind(&p.unit)
    .bind(&p.location)
    .bind(p.min_stock_milli)
    .bind(&p.note)
    .bind(p.sell_price_minor.flatten())
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| conflict_on(e, UNIQUE))?;
    for b in &barcodes {
        sqlx::query("INSERT INTO part_barcodes (barcode, part_id) VALUES ($1, $2)")
            .bind(b)
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(|e| conflict_on(e, UNIQUE))?;
    }
    if let Some(stock) = stock.filter(|s| *s > 0) {
        insert_movement(&mut tx, id, "adjustment", stock, cost, None, user.id).await?;
    }
    tx.commit().await?;

    Ok((
        StatusCode::CREATED,
        Json(load_detail(&state, id, &user).await?),
    ))
}

async fn update(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<Uuid>,
    Json(req): Json<PartInput>,
) -> AppResult<Json<PartDetail>> {
    let p = clean(&req, &user)?;
    let updated = sqlx::query(
        "UPDATE parts
            SET name = $2, brand = $3, part_number = $4, unit = $5, location = $6,
                min_stock_milli = $7, note = $8,
                sell_price_minor = CASE WHEN $9 THEN $10 ELSE sell_price_minor END,
                condition = $11,
                updated_at = now()
          WHERE id = $1",
    )
    .bind(id)
    .bind(&p.name)
    .bind(&p.brand)
    .bind(&p.part_number)
    .bind(&p.unit)
    .bind(&p.location)
    .bind(p.min_stock_milli)
    .bind(&p.note)
    .bind(p.sell_price_minor.is_some())
    .bind(p.sell_price_minor.flatten())
    .bind(&p.condition)
    .execute(&state.db)
    .await
    .map_err(|e| conflict_on(e, UNIQUE))?;
    if updated.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    Ok(Json(load_detail(&state, id, &user).await?))
}

/// Owners may delete a part created by mistake, as long as no work order
/// uses it. Its own stock records go with it.
async fn remove(
    State(state): State<AppState>,
    Owner(_): Owner,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    let mut tx = state.db.begin().await?;
    lock_part(&mut tx, id).await?;
    let used: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM work_order_items WHERE part_id = $1)
             OR EXISTS (SELECT 1 FROM delivery_items WHERE part_id = $1)",
    )
    .bind(id)
    .fetch_one(&mut *tx)
    .await?;
    if used {
        return Err(AppError::Conflict("part_in_use"));
    }
    sqlx::query("DELETE FROM stock_movements WHERE part_id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM parts WHERE id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

// ---------- barcodes ----------

#[derive(Deserialize)]
struct BarcodeInput {
    barcode: String,
}

async fn add_barcode(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<Uuid>,
    Json(req): Json<BarcodeInput>,
) -> AppResult<Json<PartDetail>> {
    let barcode = validate::barcode(&req.barcode).map_err(AppError::Validation)?;
    let mut tx = state.db.begin().await?;
    lock_part(&mut tx, id).await?;
    // Scanning a code the part already has is not an error.
    let owner: Uuid = sqlx::query_scalar(
        "WITH ins AS (
            INSERT INTO part_barcodes (barcode, part_id) VALUES ($1, $2)
            ON CONFLICT (barcode) DO NOTHING RETURNING part_id)
         SELECT part_id FROM ins
         UNION ALL SELECT part_id FROM part_barcodes WHERE barcode = $1
         LIMIT 1",
    )
    .bind(&barcode)
    .bind(id)
    .fetch_one(&mut *tx)
    .await?;
    if owner != id {
        return Err(AppError::Conflict("barcode_taken"));
    }
    touch_part(&mut tx, id).await?;
    tx.commit().await?;
    Ok(Json(load_detail(&state, id, &user).await?))
}

async fn remove_barcode(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<Uuid>,
    Json(req): Json<BarcodeInput>,
) -> AppResult<Json<PartDetail>> {
    let deleted = sqlx::query("DELETE FROM part_barcodes WHERE barcode = $1 AND part_id = $2")
        .bind(req.barcode.trim())
        .bind(id)
        .execute(&state.db)
        .await?;
    if deleted.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    Ok(Json(load_detail(&state, id, &user).await?))
}

// ---------- stock ----------

#[derive(Deserialize)]
struct MovementInput {
    /// "count": what is on the shelf now (the difference is recorded as an
    /// adjustment). Buying goes through a delivery, never through here.
    kind: String,
    quantity_milli: i64,
    /// Net purchase price per unit, owners only. It values the parts found
    /// (first stocktaking) and is kept only when stock goes up.
    unit_cost_minor: Option<i64>,
    note: Option<String>,
}

async fn add_movement(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<Uuid>,
    Json(req): Json<MovementInput>,
) -> AppResult<(StatusCode, Json<PartDetail>)> {
    if req.unit_cost_minor.is_some() && !user.is_owner() {
        return Err(AppError::Forbidden);
    }
    let note = validate::optional_text(req.note.as_deref(), 500).map_err(AppError::Validation)?;

    let mut tx = state.db.begin().await?;
    // Serializes with other movements of this part, so a count sees a
    // stock level nobody is changing at the same moment.
    lock_part(&mut tx, id).await?;
    let cost = validate::price_minor(req.unit_cost_minor).map_err(AppError::Validation)?;
    match req.kind.as_str() {
        // One way to buy parts: a delivery document with its invoice, so
        // the cost, the shelf price and the paper always go together.
        "receipt" => return Err(AppError::Validation("receipt_needs_delivery")),
        "count" => {
            let counted =
                validate::stock_quantity_milli(req.quantity_milli).map_err(AppError::Validation)?;
            let current = stock_of(&mut tx, id).await?;
            let diff = counted - current;
            if diff != 0 {
                let cost = cost.filter(|_| diff > 0);
                insert_movement(&mut tx, id, "adjustment", diff, cost, note, user.id).await?;
            }
        }
        _ => return Err(AppError::Validation("invalid_movement_kind")),
    }
    touch_part(&mut tx, id).await?;
    tx.commit().await?;
    Ok((
        StatusCode::CREATED,
        Json(load_detail(&state, id, &user).await?),
    ))
}

async fn insert_movement(
    conn: &mut PgConnection,
    part_id: Uuid,
    kind: &str,
    quantity_milli: i64,
    unit_cost_minor: Option<i64>,
    note: Option<String>,
    user_id: Uuid,
) -> AppResult<()> {
    sqlx::query(
        "INSERT INTO stock_movements (part_id, kind, quantity_milli, unit_cost_minor, note, created_by)
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(part_id)
    .bind(kind)
    .bind(quantity_milli)
    .bind(unit_cost_minor)
    .bind(note)
    .bind(user_id)
    .execute(conn)
    .await?;
    Ok(())
}

async fn stock_of(conn: &mut PgConnection, part_id: Uuid) -> AppResult<i64> {
    Ok(sqlx::query_scalar(
        "SELECT COALESCE(sum(quantity_milli), 0)::bigint FROM stock_movements WHERE part_id = $1",
    )
    .bind(part_id)
    .fetch_one(conn)
    .await?)
}

/// Name and default price of a part, with the row locked for the rest of
/// the transaction (stock changes of one part happen one at a time).
pub(crate) struct LockedPart {
    pub name: String,
    pub sell_price_minor: Option<i64>,
}

pub(crate) async fn lock_part(conn: &mut PgConnection, id: Uuid) -> AppResult<LockedPart> {
    let (name, sell_price_minor) =
        sqlx::query_as("SELECT name, sell_price_minor FROM parts WHERE id = $1 FOR UPDATE")
            .bind(id)
            .fetch_optional(conn)
            .await?
            .ok_or(AppError::NotFound)?;
    Ok(LockedPart {
        name,
        sell_price_minor,
    })
}

async fn touch_part(conn: &mut PgConnection, id: Uuid) -> AppResult<()> {
    sqlx::query("UPDATE parts SET updated_at = now() WHERE id = $1")
        .bind(id)
        .execute(conn)
        .await?;
    Ok(())
}

// ---------- detail ----------

#[derive(Serialize, sqlx::FromRow)]
struct PartRow {
    id: Uuid,
    name: String,
    brand: Option<String>,
    part_number: Option<String>,
    condition: String,
    unit: String,
    location: Option<String>,
    min_stock_milli: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    sell_price_minor: Option<i64>,
    note: Option<String>,
    stock_milli: i64,
    /// Latest net purchase cost. Owners only.
    #[serde(skip_serializing_if = "Option::is_none")]
    last_cost_minor: Option<i64>,
    /// VAT of that cost when it came with a delivery, so the markup can be
    /// computed net to net. Owners only.
    #[serde(skip_serializing_if = "Option::is_none")]
    last_cost_vat_percent: Option<i16>,
}

#[derive(Serialize, sqlx::FromRow)]
struct Movement {
    id: Uuid,
    kind: String,
    quantity_milli: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    unit_cost_minor: Option<i64>,
    note: Option<String>,
    #[serde(with = "time::serde::rfc3339")]
    created_at: OffsetDateTime,
    created_by_name: Option<String>,
    work_order_id: Option<Uuid>,
    work_order_number: Option<i64>,
    delivery_id: Option<Uuid>,
    delivery_number: Option<i64>,
}

#[derive(Serialize)]
pub struct PartDetail {
    #[serde(flatten)]
    part: PartRow,
    barcodes: Vec<String>,
    /// Latest first, at most 100.
    movements: Vec<Movement>,
}

async fn detail(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<PartDetail>> {
    Ok(Json(load_detail(&state, id, &user).await?))
}

async fn load_detail(state: &AppState, id: Uuid, user: &CurrentUser) -> AppResult<PartDetail> {
    let mut part = sqlx::query_as::<_, PartRow>(&format!(
        "SELECT p.id, p.name, p.brand, p.part_number, p.condition, p.unit, p.location, p.min_stock_milli,
                p.sell_price_minor, p.note, {STOCK} AS stock_milli,
                c.unit_cost_minor AS last_cost_minor, c.vat_percent AS last_cost_vat_percent
           FROM parts p
           LEFT JOIN LATERAL (
               SELECT m.unit_cost_minor, di.vat_percent
                 FROM stock_movements m
                 LEFT JOIN delivery_items di ON di.id = m.delivery_item_id
                WHERE m.part_id = p.id AND m.unit_cost_minor IS NOT NULL
                ORDER BY m.created_at DESC LIMIT 1) c ON true
          WHERE p.id = $1"
    ))
    .bind(id)
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;

    let barcodes = sqlx::query_scalar(
        "SELECT barcode FROM part_barcodes WHERE part_id = $1 ORDER BY created_at, barcode",
    )
    .bind(id)
    .fetch_all(&state.db)
    .await?;

    let mut movements = sqlx::query_as::<_, Movement>(
        "SELECT m.id, m.kind, m.quantity_milli, m.unit_cost_minor, m.note, m.created_at,
                u.display_name AS created_by_name,
                w.id AS work_order_id, w.number AS work_order_number,
                d.id AS delivery_id, d.number AS delivery_number
           FROM stock_movements m
           LEFT JOIN users u ON u.id = m.created_by
           LEFT JOIN work_order_items i ON i.id = m.work_order_item_id
           LEFT JOIN work_orders w ON w.id = i.work_order_id
           LEFT JOIN delivery_items di ON di.id = m.delivery_item_id
           LEFT JOIN deliveries d ON d.id = di.delivery_id
          WHERE m.part_id = $1
          ORDER BY m.created_at DESC, m.id
          LIMIT 100",
    )
    .bind(id)
    .fetch_all(&state.db)
    .await?;

    if !user.is_owner() {
        part.sell_price_minor = None;
        part.last_cost_minor = None;
        part.last_cost_vat_percent = None;
        movements.iter_mut().for_each(|m| m.unit_cost_minor = None);
    }
    Ok(PartDetail {
        part,
        barcodes,
        movements,
    })
}
