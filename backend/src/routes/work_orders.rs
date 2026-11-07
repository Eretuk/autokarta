//! Work orders and their lines: labour and parts from stock.
//!
//! Prices are financial data: mechanics never receive them and cannot set
//! them. They record what was done; the owner prices it. A part line takes
//! the part's catalogue price by default and comes off the stock while the
//! order is not cancelled.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, patch},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sqlx::PgConnection;
use time::Date;
use uuid::Uuid;

use crate::{
    auth::CurrentUser,
    error::{AppError, AppResult},
    routes::{clients::Client, deserialize_some, parts, settings, vehicles},
    state::AppState,
    validate,
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(list).post(create))
        .route("/{id}", get(detail).patch(update))
        .route("/{id}/items", axum::routing::post(add_item))
        .route(
            "/{id}/items/{item_id}",
            patch(update_item).delete(delete_item),
        )
}

// ---------- list ----------

#[derive(Deserialize)]
struct ListQuery {
    status: Option<String>,
}

#[derive(Serialize, sqlx::FromRow)]
struct ListRow {
    id: Uuid,
    number: i64,
    status: String,
    opened_on: Date,
    complaint: Option<String>,
    vehicle_id: Uuid,
    plate: Option<String>,
    vin: Option<String>,
    frame_number: Option<String>,
    make: Option<String>,
    model: Option<String>,
    client_name: Option<String>,
    item_count: i64,
}

async fn list(
    State(state): State<AppState>,
    _user: CurrentUser,
    Query(query): Query<ListQuery>,
) -> AppResult<Json<Vec<ListRow>>> {
    let status = match query.status.as_deref() {
        None | Some("open") => Some("open".to_string()),
        Some("all") => None,
        Some(s) => Some(
            validate::one_of(s, validate::WORK_ORDER_STATUSES, "invalid_status")
                .map_err(AppError::Validation)?,
        ),
    };
    let rows = sqlx::query_as::<_, ListRow>(
        "SELECT w.id, w.number, w.status, w.opened_on, w.complaint,
                v.id AS vehicle_id, v.plate, v.vin, v.frame_number, v.make, v.model,
                c.name AS client_name,
                (SELECT count(*) FROM work_order_items i WHERE i.work_order_id = w.id) AS item_count
           FROM work_orders w
           JOIN vehicles v ON v.id = w.vehicle_id
           LEFT JOIN clients c ON c.id = w.client_id
          WHERE $1::text IS NULL OR w.status = $1
          ORDER BY w.opened_on DESC, w.number DESC
          LIMIT 200",
    )
    .bind(status)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(rows))
}

// ---------- create / update ----------

#[derive(Deserialize)]
struct CreateWorkOrder {
    vehicle_id: Uuid,
    odometer: Option<i64>,
    complaint: Option<String>,
    #[serde(default)]
    confirm_lower: bool,
}

async fn create(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(req): Json<CreateWorkOrder>,
) -> AppResult<(StatusCode, Json<WorkOrderDetail>)> {
    let complaint =
        validate::optional_text(req.complaint.as_deref(), 2000).map_err(AppError::Validation)?;
    let odometer = req
        .odometer
        .map(validate::odometer)
        .transpose()
        .map_err(AppError::Validation)?;

    let mut tx = state.db.begin().await?;
    vehicles::lock_vehicle(&mut tx, req.vehicle_id).await?;
    // Check before inserting the order: a rolled-back insert would still
    // use up an order number and leave a gap.
    if let (Some(reading), false) = (odometer, req.confirm_lower) {
        vehicles::ensure_not_lower(&mut tx, req.vehicle_id, reading, None).await?;
    }
    // Remember who brought the car this time: owners change later.
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO work_orders (vehicle_id, client_id, complaint, created_by)
         VALUES ($1, (SELECT client_id FROM vehicle_ownerships
                       WHERE vehicle_id = $1 AND ended_on IS NULL), $2, $3)
         RETURNING id",
    )
    .bind(req.vehicle_id)
    .bind(complaint)
    .bind(user.id)
    .fetch_one(&mut *tx)
    .await?;
    if let Some(reading) = odometer {
        vehicles::insert_reading(
            &mut tx,
            req.vehicle_id,
            reading,
            None,
            Some(id),
            user.id,
            req.confirm_lower,
        )
        .await?;
    }
    vehicles::touch_vehicle(&mut tx, req.vehicle_id).await?;
    tx.commit().await?;

    Ok((
        StatusCode::CREATED,
        Json(load_detail(&state, id, &user).await?),
    ))
}

#[derive(Deserialize)]
struct UpdateWorkOrder {
    /// Absent: unchanged. Empty string: cleared.
    complaint: Option<String>,
    notes: Option<String>,
    status: Option<String>,
    odometer: Option<i64>,
    #[serde(default)]
    confirm_lower: bool,
}

async fn update(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateWorkOrder>,
) -> AppResult<Json<WorkOrderDetail>> {
    let complaint = req
        .complaint
        .as_deref()
        .map(|c| validate::optional_text(Some(c), 2000))
        .transpose()
        .map_err(AppError::Validation)?;
    let notes = req
        .notes
        .as_deref()
        .map(|n| validate::optional_text(Some(n), 4000))
        .transpose()
        .map_err(AppError::Validation)?;
    let status = req
        .status
        .as_deref()
        .map(|s| validate::one_of(s, validate::WORK_ORDER_STATUSES, "invalid_status"))
        .transpose()
        .map_err(AppError::Validation)?;
    let odometer = req
        .odometer
        .map(validate::odometer)
        .transpose()
        .map_err(AppError::Validation)?;

    let mut tx = state.db.begin().await?;
    let (vehicle_id, old_status) = lock_order(&mut tx, id).await?;

    sqlx::query(
        "UPDATE work_orders
            SET complaint = CASE WHEN $2 THEN $3 ELSE complaint END,
                notes = CASE WHEN $4 THEN $5 ELSE notes END,
                status = COALESCE($6, status),
                closed_at = CASE
                    WHEN $6 IS NULL OR $6 = status THEN closed_at
                    WHEN $6 = 'open' THEN NULL
                    ELSE now() END,
                updated_at = now()
          WHERE id = $1",
    )
    .bind(id)
    .bind(complaint.is_some())
    .bind(complaint.flatten())
    .bind(notes.is_some())
    .bind(notes.flatten())
    .bind(&status)
    .execute(&mut *tx)
    .await?;
    if let Some(new_status) = &status {
        sync_stock(&mut tx, id, &old_status, new_status, user.id).await?;
    }

    if let Some(reading) = odometer {
        vehicles::insert_reading(
            &mut tx,
            vehicle_id,
            reading,
            None,
            Some(id),
            user.id,
            req.confirm_lower,
        )
        .await?;
    }
    tx.commit().await?;

    Ok(Json(load_detail(&state, id, &user).await?))
}

/// A cancelled order used no parts: cancelling puts its parts back in
/// stock, reopening takes them off again.
async fn sync_stock(
    conn: &mut PgConnection,
    id: Uuid,
    old_status: &str,
    new_status: &str,
    user_id: Uuid,
) -> AppResult<()> {
    let (was, is) = (old_status == "cancelled", new_status == "cancelled");
    if was == is {
        return Ok(());
    }
    // Same lock order as everywhere else: the order first, then its parts.
    sqlx::query(
        "SELECT 1 FROM parts WHERE id IN
            (SELECT part_id FROM work_order_items WHERE work_order_id = $1 AND part_id IS NOT NULL)
          ORDER BY id FOR UPDATE",
    )
    .bind(id)
    .execute(&mut *conn)
    .await?;
    if is {
        sqlx::query(
            "DELETE FROM stock_movements
              WHERE work_order_item_id IN (SELECT id FROM work_order_items WHERE work_order_id = $1)",
        )
        .bind(id)
        .execute(conn)
        .await?;
    } else {
        sqlx::query(
            "INSERT INTO stock_movements (part_id, kind, quantity_milli, work_order_item_id, created_by)
             SELECT part_id, 'consumption', -quantity_milli, id, $2
               FROM work_order_items WHERE work_order_id = $1 AND kind = 'part'",
        )
        .bind(id)
        .bind(user_id)
        .execute(conn)
        .await?;
    }
    Ok(())
}

/// Locks the order row; returns (vehicle_id, status).
async fn lock_order(conn: &mut PgConnection, id: Uuid) -> AppResult<(Uuid, String)> {
    sqlx::query_as("SELECT vehicle_id, status FROM work_orders WHERE id = $1 FOR UPDATE")
        .bind(id)
        .fetch_optional(conn)
        .await?
        .ok_or(AppError::NotFound)
}

async fn lock_open_order(conn: &mut PgConnection, id: Uuid) -> AppResult<()> {
    let (_, status) = lock_order(conn, id).await?;
    if status != "open" {
        return Err(AppError::Conflict("work_order_closed"));
    }
    Ok(())
}

// ---------- items ----------

#[derive(Deserialize)]
struct AddItem {
    /// Required for labour; for a part, defaults to the part's name.
    description: Option<String>,
    part_id: Option<Uuid>,
    quantity_milli: Option<i64>,
    unit_price_minor: Option<i64>,
}

async fn add_item(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<Uuid>,
    Json(req): Json<AddItem>,
) -> AppResult<(StatusCode, Json<WorkOrderDetail>)> {
    if req.unit_price_minor.is_some() && !user.is_owner() {
        return Err(AppError::Forbidden);
    }
    let description = req
        .description
        .as_deref()
        .filter(|d| !d.trim().is_empty() || req.part_id.is_none())
        .map(validate::item_description)
        .transpose()
        .map_err(AppError::Validation)?;
    if description.is_none() && req.part_id.is_none() {
        return Err(AppError::Validation("invalid_description"));
    }
    let quantity = validate::quantity_milli(req.quantity_milli.unwrap_or(1000))
        .map_err(AppError::Validation)?;
    let price = validate::price_minor(req.unit_price_minor).map_err(AppError::Validation)?;

    let mut tx = state.db.begin().await?;
    lock_open_order(&mut tx, id).await?;
    let (kind, description, price) = match req.part_id {
        Some(part_id) => {
            let part = parts::lock_part(&mut tx, part_id)
                .await
                .map_err(|e| match e {
                    AppError::NotFound => AppError::Validation("unknown_part"),
                    e => e,
                })?;
            (
                "part",
                description.unwrap_or(part.name),
                price.or(part.sell_price_minor),
            )
        }
        None => ("labour", description.unwrap_or_default(), price),
    };
    let item_id: Uuid = sqlx::query_scalar(
        "INSERT INTO work_order_items
             (work_order_id, kind, part_id, description, quantity_milli, unit_price_minor, created_by)
         VALUES ($1, $2, $3, $4, $5, $6, $7)
         RETURNING id",
    )
    .bind(id)
    .bind(kind)
    .bind(req.part_id)
    .bind(description)
    .bind(quantity)
    .bind(price)
    .bind(user.id)
    .fetch_one(&mut *tx)
    .await?;
    if let Some(part_id) = req.part_id {
        sqlx::query(
            "INSERT INTO stock_movements (part_id, kind, quantity_milli, work_order_item_id, created_by)
             VALUES ($1, 'consumption', $2, $3, $4)",
        )
        .bind(part_id)
        .bind(-quantity)
        .bind(item_id)
        .bind(user.id)
        .execute(&mut *tx)
        .await?;
    }
    touch_order(&mut tx, id).await?;
    tx.commit().await?;

    Ok((
        StatusCode::CREATED,
        Json(load_detail(&state, id, &user).await?),
    ))
}

#[derive(Deserialize)]
struct UpdateItem {
    description: Option<String>,
    quantity_milli: Option<i64>,
    /// Absent: unchanged. null: price removed. Owners only.
    #[serde(default, deserialize_with = "deserialize_some")]
    unit_price_minor: Option<Option<i64>>,
}

async fn update_item(
    State(state): State<AppState>,
    user: CurrentUser,
    Path((id, item_id)): Path<(Uuid, Uuid)>,
    Json(req): Json<UpdateItem>,
) -> AppResult<Json<WorkOrderDetail>> {
    if req.unit_price_minor.is_some() && !user.is_owner() {
        return Err(AppError::Forbidden);
    }
    let description = req
        .description
        .as_deref()
        .map(validate::item_description)
        .transpose()
        .map_err(AppError::Validation)?;
    let quantity = req
        .quantity_milli
        .map(validate::quantity_milli)
        .transpose()
        .map_err(AppError::Validation)?;
    let price = req
        .unit_price_minor
        .map(validate::price_minor)
        .transpose()
        .map_err(AppError::Validation)?;

    let mut tx = state.db.begin().await?;
    lock_open_order(&mut tx, id).await?;
    let part_id: Option<Uuid> = sqlx::query_scalar(
        "UPDATE work_order_items
            SET description = COALESCE($3, description),
                quantity_milli = COALESCE($4, quantity_milli),
                unit_price_minor = CASE WHEN $5 THEN $6 ELSE unit_price_minor END,
                updated_at = now()
          WHERE id = $2 AND work_order_id = $1
          RETURNING part_id",
    )
    .bind(id)
    .bind(item_id)
    .bind(description)
    .bind(quantity)
    .bind(price.is_some())
    .bind(price.flatten())
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(AppError::NotFound)?;
    if let (Some(part_id), Some(quantity)) = (part_id, quantity) {
        parts::lock_part(&mut tx, part_id).await?;
        sqlx::query("UPDATE stock_movements SET quantity_milli = $2 WHERE work_order_item_id = $1")
            .bind(item_id)
            .bind(-quantity)
            .execute(&mut *tx)
            .await?;
    }
    touch_order(&mut tx, id).await?;
    tx.commit().await?;

    Ok(Json(load_detail(&state, id, &user).await?))
}

async fn delete_item(
    State(state): State<AppState>,
    user: CurrentUser,
    Path((id, item_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<WorkOrderDetail>> {
    let mut tx = state.db.begin().await?;
    lock_open_order(&mut tx, id).await?;
    let deleted = sqlx::query("DELETE FROM work_order_items WHERE id = $2 AND work_order_id = $1")
        .bind(id)
        .bind(item_id)
        .execute(&mut *tx)
        .await?;
    if deleted.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    touch_order(&mut tx, id).await?;
    tx.commit().await?;

    Ok(Json(load_detail(&state, id, &user).await?))
}

async fn touch_order(conn: &mut PgConnection, id: Uuid) -> AppResult<()> {
    sqlx::query("UPDATE work_orders SET updated_at = now() WHERE id = $1")
        .bind(id)
        .execute(conn)
        .await?;
    Ok(())
}

// ---------- detail ----------

#[derive(Serialize, sqlx::FromRow)]
struct OrderRow {
    id: Uuid,
    number: i64,
    status: String,
    opened_on: Date,
    complaint: Option<String>,
    notes: Option<String>,
    created_by_name: Option<String>,
}

#[derive(Serialize, sqlx::FromRow)]
struct OrderVehicle {
    id: Uuid,
    plate: Option<String>,
    vin: Option<String>,
    frame_number: Option<String>,
    make: Option<String>,
    model: Option<String>,
    year: Option<i16>,
    odometer_unit: String,
}

#[derive(Serialize, sqlx::FromRow)]
struct Item {
    id: Uuid,
    kind: String,
    description: String,
    quantity_milli: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    part_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    part_number: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    brand: Option<String>,
    /// "used" parts are marked on screen and on paper.
    #[serde(skip_serializing_if = "Option::is_none")]
    condition: Option<String>,
    /// Unit of the part ("l" for oil); labour has none.
    #[serde(skip_serializing_if = "Option::is_none")]
    unit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    unit_price_minor: Option<i64>,
    #[sqlx(skip)]
    #[serde(skip_serializing_if = "Option::is_none")]
    line_total_minor: Option<i64>,
}

/// Owners only: sum of priced lines and how many lines still lack a price.
#[derive(Serialize)]
struct Totals {
    currency: String,
    total_minor: i64,
    unpriced_items: usize,
}

#[derive(Serialize)]
pub struct WorkOrderDetail {
    #[serde(flatten)]
    order: OrderRow,
    vehicle: OrderVehicle,
    client: Option<Client>,
    odometer: Option<i32>,
    items: Vec<Item>,
    #[serde(skip_serializing_if = "Option::is_none")]
    totals: Option<Totals>,
}

async fn detail(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<WorkOrderDetail>> {
    Ok(Json(load_detail(&state, id, &user).await?))
}

async fn load_detail(state: &AppState, id: Uuid, user: &CurrentUser) -> AppResult<WorkOrderDetail> {
    let (order, vehicle_id, client_id) = {
        #[derive(sqlx::FromRow)]
        struct Row {
            #[sqlx(flatten)]
            order: OrderRow,
            vehicle_id: Uuid,
            client_id: Option<Uuid>,
        }
        let row = sqlx::query_as::<_, Row>(
            "SELECT w.id, w.number, w.status, w.opened_on, w.complaint, w.notes,
                    u.display_name AS created_by_name, w.vehicle_id, w.client_id
               FROM work_orders w LEFT JOIN users u ON u.id = w.created_by
              WHERE w.id = $1",
        )
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;
        (row.order, row.vehicle_id, row.client_id)
    };

    let vehicle = sqlx::query_as::<_, OrderVehicle>(
        "SELECT id, plate, vin, frame_number, make, model, year, odometer_unit FROM vehicles WHERE id = $1",
    )
    .bind(vehicle_id)
    .fetch_one(&state.db)
    .await?;

    let client = match client_id {
        Some(cid) => {
            sqlx::query_as::<_, Client>("SELECT id, name, phone, note FROM clients WHERE id = $1")
                .bind(cid)
                .fetch_optional(&state.db)
                .await?
        }
        None => None,
    };

    let odometer: Option<i32> =
        sqlx::query_scalar("SELECT reading FROM odometer_readings WHERE work_order_id = $1")
            .bind(id)
            .fetch_optional(&state.db)
            .await?;

    let mut items = sqlx::query_as::<_, Item>(
        "SELECT i.id, i.kind, i.description, i.quantity_milli, i.unit_price_minor,
                i.part_id, p.part_number, p.brand, p.condition, p.unit
           FROM work_order_items i LEFT JOIN parts p ON p.id = i.part_id
          WHERE i.work_order_id = $1
          ORDER BY i.created_at, i.id",
    )
    .bind(id)
    .fetch_all(&state.db)
    .await?;

    let totals = if user.is_owner() {
        let mut total = 0i64;
        let mut unpriced = 0usize;
        for item in &mut items {
            match item.unit_price_minor {
                Some(price) => {
                    let line = validate::line_total(item.quantity_milli, price);
                    item.line_total_minor = Some(line);
                    total += line;
                }
                None => unpriced += 1,
            }
        }
        Some(Totals {
            currency: settings::load(&state.db).await?.currency,
            total_minor: total,
            unpriced_items: unpriced,
        })
    } else {
        items.iter_mut().for_each(|i| i.unit_price_minor = None);
        None
    };

    Ok(WorkOrderDetail {
        order,
        vehicle,
        client,
        odometer,
        items,
        totals,
    })
}
