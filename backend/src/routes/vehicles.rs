//! Vehicles, their owners over time and odometer readings.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{delete, get, post, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sqlx::{PgConnection, Postgres, Transaction};
use time::{Date, OffsetDateTime};
use uuid::Uuid;

use crate::{
    auth::{CurrentUser, Owner},
    error::{conflict_on, AppError, AppResult},
    routes::{clients::Client, like_pattern, settings},
    state::AppState,
    validate,
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(search).post(create))
        .route("/{id}", get(detail).put(update))
        .route("/{id}/owner", put(set_owner))
        .route("/{id}/odometer", post(add_reading))
        .route("/{id}/odometer/{reading_id}", delete(delete_reading))
}

const UNIQUE: &[(&str, &str)] = &[
    ("vehicles_vin_idx", "vin_taken"),
    ("vehicles_frame_number_idx", "frame_number_taken"),
];

// ---------- search ----------

#[derive(Serialize, sqlx::FromRow)]
pub struct VehicleSummary {
    pub id: Uuid,
    pub plate: Option<String>,
    pub vin: Option<String>,
    pub frame_number: Option<String>,
    pub make: Option<String>,
    pub model: Option<String>,
    pub year: Option<i16>,
    pub role: String,
    pub odometer_unit: String,
    pub owner_name: Option<String>,
    pub last_odometer: Option<i32>,
    pub open_work_orders: i64,
}

const SUMMARY_SELECT: &str = "
    SELECT v.id, v.plate, v.vin, v.frame_number, v.make, v.model, v.year, v.role, v.odometer_unit,
           c.name AS owner_name,
           (SELECT r.reading FROM odometer_readings r WHERE r.vehicle_id = v.id
             ORDER BY r.read_on DESC, r.reading DESC LIMIT 1) AS last_odometer,
           (SELECT count(*) FROM work_orders w WHERE w.vehicle_id = v.id AND w.status = 'open') AS open_work_orders
      FROM vehicles v
      LEFT JOIN vehicle_ownerships o ON o.vehicle_id = v.id AND o.ended_on IS NULL
      LEFT JOIN clients c ON c.id = o.client_id";

#[derive(Deserialize)]
struct SearchQuery {
    #[serde(default)]
    q: String,
}

/// Finds vehicles by VIN, frame number or plate (ignoring spaces and
/// dashes), make/model, or the current owner's name or phone.
/// Without a query: the most recently touched vehicles.
async fn search(
    State(state): State<AppState>,
    _user: CurrentUser,
    Query(query): Query<SearchQuery>,
) -> AppResult<Json<Vec<VehicleSummary>>> {
    let q = query.q.trim();
    let compact = validate::plate_search_key(&q.to_uppercase());
    let digits: String = q.chars().filter(char::is_ascii_digit).collect();
    let rows = sqlx::query_as::<_, VehicleSummary>(&format!(
        "{SUMMARY_SELECT}
          WHERE $1 = ''
             OR ($2 <> '' AND (v.vin LIKE $3
                 OR replace(v.frame_number, '-', '') LIKE $3
                 OR regexp_replace(v.plate, '[\\s-]', '', 'g') LIKE $3))
             OR lower(concat_ws(' ', v.make, v.model)) LIKE $4
             OR lower(c.name) LIKE $4
             OR (length($5) >= 5 AND regexp_replace(coalesce(c.phone, ''), '\\D', '', 'g') LIKE '%' || $5 || '%')
          ORDER BY v.updated_at DESC
          LIMIT 50"
    ))
    .bind(q)
    .bind(&compact)
    .bind(like_pattern(&compact))
    .bind(like_pattern(&q.to_lowercase()))
    .bind(digits)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(rows))
}

// ---------- create / update ----------

#[derive(Deserialize)]
struct VehicleInput {
    vin: Option<String>,
    frame_number: Option<String>,
    plate: Option<String>,
    make: Option<String>,
    model: Option<String>,
    year: Option<i32>,
    engine: Option<String>,
    note: Option<String>,
    role: Option<String>,
    odometer_unit: Option<String>,
}

struct CleanVehicle {
    vin: Option<String>,
    frame_number: Option<String>,
    plate: Option<String>,
    make: Option<String>,
    model: Option<String>,
    year: Option<i16>,
    engine: Option<String>,
    note: Option<String>,
    role: String,
    odometer_unit: Option<String>,
}

fn clean_vehicle(req: &VehicleInput) -> Result<CleanVehicle, AppError> {
    let v = (|| {
        Ok::<_, &'static str>(CleanVehicle {
            vin: validate::vin(req.vin.as_deref())?,
            frame_number: validate::frame_number(req.frame_number.as_deref())?,
            plate: validate::plate(req.plate.as_deref())?,
            make: validate::optional_text(req.make.as_deref(), 60)?,
            model: validate::optional_text(req.model.as_deref(), 60)?,
            year: validate::year(req.year)?,
            engine: validate::optional_text(req.engine.as_deref(), 60)?,
            note: validate::optional_text(req.note.as_deref(), 2000)?,
            role: validate::one_of(
                req.role.as_deref().unwrap_or("customer"),
                validate::VEHICLE_ROLES,
                "invalid_vehicle_role",
            )?,
            odometer_unit: req
                .odometer_unit
                .as_deref()
                .map(|u| validate::one_of(u, validate::DISTANCE_UNITS, "invalid_distance_unit"))
                .transpose()?,
        })
    })()
    .map_err(AppError::Validation)?;

    if v.vin.is_none() && v.frame_number.is_none() && v.plate.is_none() {
        return Err(AppError::Validation("vehicle_identifier_required"));
    }
    Ok(v)
}

#[derive(Deserialize)]
struct CreateVehicle {
    #[serde(flatten)]
    vehicle: VehicleInput,
    owner_id: Option<Uuid>,
    odometer: Option<i64>,
}

async fn create(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(req): Json<CreateVehicle>,
) -> AppResult<(StatusCode, Json<VehicleDetail>)> {
    let v = clean_vehicle(&req.vehicle)?;
    let odometer = req
        .odometer
        .map(validate::odometer)
        .transpose()
        .map_err(AppError::Validation)?;
    let unit = match v.odometer_unit {
        Some(u) => u,
        None => settings::load(&state.db).await?.distance_unit,
    };

    let mut tx = state.db.begin().await?;
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO vehicles (vin, frame_number, plate, make, model, year, engine, note, role, odometer_unit)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
         RETURNING id",
    )
    .bind(&v.vin)
    .bind(&v.frame_number)
    .bind(&v.plate)
    .bind(&v.make)
    .bind(&v.model)
    .bind(v.year)
    .bind(&v.engine)
    .bind(&v.note)
    .bind(&v.role)
    .bind(&unit)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| conflict_on(e, UNIQUE))?;

    if let Some(owner_id) = req.owner_id {
        ensure_client(&mut tx, owner_id).await?;
        sqlx::query("INSERT INTO vehicle_ownerships (vehicle_id, client_id) VALUES ($1, $2)")
            .bind(id)
            .bind(owner_id)
            .execute(&mut *tx)
            .await?;
    }
    if let Some(reading) = odometer {
        insert_reading(&mut tx, id, reading, None, None, user.id, true).await?;
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
    Json(req): Json<VehicleInput>,
) -> AppResult<Json<VehicleDetail>> {
    let v = clean_vehicle(&req)?;
    let updated = sqlx::query(
        "UPDATE vehicles SET vin = $2, frame_number = $3, plate = $4, make = $5, model = $6,
                             year = $7, engine = $8, note = $9, role = $10,
                             odometer_unit = COALESCE($11, odometer_unit), updated_at = now()
          WHERE id = $1",
    )
    .bind(id)
    .bind(&v.vin)
    .bind(&v.frame_number)
    .bind(&v.plate)
    .bind(&v.make)
    .bind(&v.model)
    .bind(v.year)
    .bind(&v.engine)
    .bind(&v.note)
    .bind(&v.role)
    .bind(&v.odometer_unit)
    .execute(&state.db)
    .await
    .map_err(|e| conflict_on(e, UNIQUE))?;
    if updated.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    Ok(Json(load_detail(&state, id, &user).await?))
}

async fn ensure_client(conn: &mut PgConnection, id: Uuid) -> AppResult<()> {
    let exists: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM clients WHERE id = $1)")
        .bind(id)
        .fetch_one(conn)
        .await?;
    if exists {
        Ok(())
    } else {
        Err(AppError::Validation("unknown_client"))
    }
}

// ---------- owner ----------

#[derive(Deserialize)]
struct SetOwner {
    /// None: the car no longer has a known owner (e.g. scrapped, left).
    client_id: Option<Uuid>,
    /// Date of the change; today by default.
    since: Option<Date>,
}

/// Ends the current ownership and starts a new one. Past owners stay in the
/// history but are never shown on the public page.
async fn set_owner(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<Uuid>,
    Json(req): Json<SetOwner>,
) -> AppResult<Json<VehicleDetail>> {
    let since = req
        .since
        .unwrap_or_else(|| OffsetDateTime::now_utc().date());
    let mut tx = state.db.begin().await?;

    lock_vehicle(&mut tx, id).await?;
    let current: Option<(Uuid, Uuid, Date)> = sqlx::query_as(
        "SELECT id, client_id, started_on FROM vehicle_ownerships
          WHERE vehicle_id = $1 AND ended_on IS NULL",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?;

    if current.map(|c| c.1) == req.client_id {
        return Ok(Json(load_detail(&state, id, &user).await?));
    }
    if let Some((ownership_id, _, started_on)) = current {
        if since < started_on {
            return Err(AppError::Validation("owner_change_before_start"));
        }
        sqlx::query("UPDATE vehicle_ownerships SET ended_on = $2 WHERE id = $1")
            .bind(ownership_id)
            .bind(since)
            .execute(&mut *tx)
            .await?;
    }
    if let Some(client_id) = req.client_id {
        ensure_client(&mut tx, client_id).await?;
        sqlx::query("INSERT INTO vehicle_ownerships (vehicle_id, client_id, started_on) VALUES ($1, $2, $3)")
            .bind(id)
            .bind(client_id)
            .bind(since)
            .execute(&mut *tx)
            .await?;
    }
    touch_vehicle(&mut tx, id).await?;
    tx.commit().await?;

    Ok(Json(load_detail(&state, id, &user).await?))
}

// ---------- odometer ----------

#[derive(Deserialize)]
struct AddReading {
    reading: i64,
    read_on: Option<Date>,
    /// The client confirmed that a reading lower than the last one is
    /// correct (replaced instrument cluster, earlier typo).
    #[serde(default)]
    confirm_lower: bool,
}

async fn add_reading(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<Uuid>,
    Json(req): Json<AddReading>,
) -> AppResult<(StatusCode, Json<VehicleDetail>)> {
    let reading = validate::odometer(req.reading).map_err(AppError::Validation)?;
    let mut tx = state.db.begin().await?;
    lock_vehicle(&mut tx, id).await?;
    insert_reading(
        &mut tx,
        id,
        reading,
        req.read_on,
        None,
        user.id,
        req.confirm_lower,
    )
    .await?;
    tx.commit().await?;
    Ok((
        StatusCode::CREATED,
        Json(load_detail(&state, id, &user).await?),
    ))
}

/// Inserts (or, for a work order, replaces) an odometer reading. Refuses a
/// reading lower than the latest one unless `confirm_lower` is set: going
/// backwards is almost always a typo.
pub(crate) async fn insert_reading(
    tx: &mut Transaction<'static, Postgres>,
    vehicle_id: Uuid,
    reading: i32,
    read_on: Option<Date>,
    work_order_id: Option<Uuid>,
    user_id: Uuid,
    confirm_lower: bool,
) -> AppResult<()> {
    if !confirm_lower {
        let latest: Option<i32> = sqlx::query_scalar(
            "SELECT reading FROM odometer_readings
              WHERE vehicle_id = $1 AND ($2::uuid IS NULL OR work_order_id IS DISTINCT FROM $2)
              ORDER BY read_on DESC, reading DESC LIMIT 1",
        )
        .bind(vehicle_id)
        .bind(work_order_id)
        .fetch_optional(&mut **tx)
        .await?;
        if latest.is_some_and(|l| reading < l) {
            return Err(AppError::Conflict("odometer_lower_than_previous"));
        }
    }

    match work_order_id {
        Some(wo) => {
            sqlx::query(
                "INSERT INTO odometer_readings (vehicle_id, reading, read_on, work_order_id, created_by)
                 VALUES ($1, $2, COALESCE($3, current_date), $4, $5)
                 ON CONFLICT (work_order_id) DO UPDATE SET reading = EXCLUDED.reading",
            )
            .bind(vehicle_id)
            .bind(reading)
            .bind(read_on)
            .bind(wo)
            .bind(user_id)
            .execute(&mut **tx)
            .await?;
        }
        None => {
            sqlx::query(
                "INSERT INTO odometer_readings (vehicle_id, reading, read_on, created_by)
                 VALUES ($1, $2, COALESCE($3, current_date), $4)",
            )
            .bind(vehicle_id)
            .bind(reading)
            .bind(read_on)
            .bind(user_id)
            .execute(&mut **tx)
            .await?;
        }
    }
    touch_vehicle(tx, vehicle_id).await
}

/// Owners may delete a mistyped manual reading. Readings of a work order are
/// changed through the work order.
async fn delete_reading(
    State(state): State<AppState>,
    Owner(owner): Owner,
    Path((id, reading_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<VehicleDetail>> {
    let linked: Option<Option<Uuid>> = sqlx::query_scalar(
        "SELECT work_order_id FROM odometer_readings WHERE id = $1 AND vehicle_id = $2",
    )
    .bind(reading_id)
    .bind(id)
    .fetch_optional(&state.db)
    .await?;
    match linked {
        None => return Err(AppError::NotFound),
        Some(Some(_)) => return Err(AppError::Conflict("reading_belongs_to_work_order")),
        Some(None) => {}
    }
    sqlx::query("DELETE FROM odometer_readings WHERE id = $1")
        .bind(reading_id)
        .execute(&state.db)
        .await?;
    Ok(Json(load_detail(&state, id, &owner).await?))
}

pub(crate) async fn lock_vehicle(conn: &mut PgConnection, id: Uuid) -> AppResult<()> {
    sqlx::query("SELECT 1 FROM vehicles WHERE id = $1 FOR UPDATE")
        .bind(id)
        .fetch_optional(conn)
        .await?
        .map(|_| ())
        .ok_or(AppError::NotFound)
}

/// Recently touched vehicles come first in the default list.
pub(crate) async fn touch_vehicle(conn: &mut PgConnection, id: Uuid) -> AppResult<()> {
    sqlx::query("UPDATE vehicles SET updated_at = now() WHERE id = $1")
        .bind(id)
        .execute(conn)
        .await?;
    Ok(())
}

// ---------- detail ----------

#[derive(Serialize, sqlx::FromRow)]
struct VehicleRow {
    id: Uuid,
    vin: Option<String>,
    frame_number: Option<String>,
    plate: Option<String>,
    make: Option<String>,
    model: Option<String>,
    year: Option<i16>,
    engine: Option<String>,
    note: Option<String>,
    role: String,
    odometer_unit: String,
    #[serde(with = "time::serde::rfc3339")]
    created_at: OffsetDateTime,
}

#[derive(Serialize, sqlx::FromRow)]
struct Ownership {
    client_id: Uuid,
    name: String,
    phone: Option<String>,
    started_on: Date,
    ended_on: Option<Date>,
}

#[derive(Serialize, sqlx::FromRow)]
struct Reading {
    id: Uuid,
    reading: i32,
    read_on: Date,
    work_order_id: Option<Uuid>,
    work_order_number: Option<i64>,
}

#[derive(Serialize, sqlx::FromRow)]
struct WorkOrderSummary {
    id: Uuid,
    number: i64,
    status: String,
    opened_on: Date,
    complaint: Option<String>,
    odometer: Option<i32>,
    item_count: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    total_minor: Option<i64>,
}

#[derive(Serialize)]
pub struct VehicleDetail {
    #[serde(flatten)]
    vehicle: VehicleRow,
    owner: Option<Client>,
    owners: Vec<Ownership>,
    odometer: Vec<Reading>,
    work_orders: Vec<WorkOrderSummary>,
}

async fn detail(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<VehicleDetail>> {
    Ok(Json(load_detail(&state, id, &user).await?))
}

async fn load_detail(state: &AppState, id: Uuid, user: &CurrentUser) -> AppResult<VehicleDetail> {
    let vehicle = sqlx::query_as::<_, VehicleRow>(
        "SELECT id, vin, frame_number, plate, make, model, year, engine, note, role, odometer_unit, created_at
           FROM vehicles WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;

    let owners = sqlx::query_as::<_, Ownership>(
        "SELECT o.client_id, c.name, c.phone, o.started_on, o.ended_on
           FROM vehicle_ownerships o JOIN clients c ON c.id = o.client_id
          WHERE o.vehicle_id = $1
          ORDER BY o.ended_on IS NULL DESC, o.started_on DESC",
    )
    .bind(id)
    .fetch_all(&state.db)
    .await?;

    let owner = sqlx::query_as::<_, Client>(
        "SELECT c.id, c.name, c.phone, c.note
           FROM vehicle_ownerships o JOIN clients c ON c.id = o.client_id
          WHERE o.vehicle_id = $1 AND o.ended_on IS NULL",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?;

    let odometer = sqlx::query_as::<_, Reading>(
        "SELECT r.id, r.reading, r.read_on, r.work_order_id, w.number AS work_order_number
           FROM odometer_readings r LEFT JOIN work_orders w ON w.id = r.work_order_id
          WHERE r.vehicle_id = $1
          ORDER BY r.read_on DESC, r.reading DESC
          LIMIT 50",
    )
    .bind(id)
    .fetch_all(&state.db)
    .await?;

    // Totals are financial data: owners only.
    let mut work_orders = sqlx::query_as::<_, WorkOrderSummary>(
        "SELECT w.id, w.number, w.status, w.opened_on, w.complaint,
                (SELECT reading FROM odometer_readings r WHERE r.work_order_id = w.id) AS odometer,
                (SELECT count(*) FROM work_order_items i WHERE i.work_order_id = w.id) AS item_count,
                (SELECT sum((i.quantity_milli::numeric * i.unit_price_minor / 1000)::numeric(20,0))::bigint
                   FROM work_order_items i WHERE i.work_order_id = w.id) AS total_minor
           FROM work_orders w
          WHERE w.vehicle_id = $1
          ORDER BY w.opened_on DESC, w.number DESC",
    )
    .bind(id)
    .fetch_all(&state.db)
    .await?;
    if !user.is_owner() {
        work_orders.iter_mut().for_each(|w| w.total_minor = None);
    }

    Ok(VehicleDetail {
        vehicle,
        owner,
        owners,
        odometer,
        work_orders,
    })
}
