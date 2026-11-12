//! Stock reports for the owner and the accountant: what was bought, what
//! went into work orders, and what is left on the shelves and is worth.
//!
//! Stock is valued at the last known net purchase cost of each part (from
//! a receipt or a stock count with a cost) before the moment asked for; a
//! part whose first cost was recorded only later is valued at that first
//! cost, so a part fitted before its invoice was typed in still counts.
//! Months follow the time zone the browser sends, since the database runs
//! in UTC and an invoice at 23:30 on the 31st belongs to that month.

use axum::{
    extract::{Query, State},
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Postgres};
use time::{Date, Month, OffsetDateTime};
use uuid::Uuid;

use crate::{
    auth::Owner,
    error::{AppError, AppResult},
    routes::settings,
    state::AppState,
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/months", get(months))
        .route("/stock", get(stock))
}

/// Unit cost of `m.part_id` as of `$T`: the latest cost recorded before
/// it, else the first one recorded after.
const COST_AT: &str = "COALESCE(
    (SELECT c.unit_cost_minor FROM stock_movements c
      WHERE c.part_id = m.part_id AND c.unit_cost_minor IS NOT NULL AND c.created_at <= {t}
      ORDER BY c.created_at DESC LIMIT 1),
    (SELECT c.unit_cost_minor FROM stock_movements c
      WHERE c.part_id = m.part_id AND c.unit_cost_minor IS NOT NULL
      ORDER BY c.created_at LIMIT 1))";

fn cost_at(t: &str) -> String {
    COST_AT.replace("{t}", t)
}

async fn valid_tz(db: &PgPool, tz: Option<&str>) -> AppResult<String> {
    let tz = tz.unwrap_or("UTC");
    let ok: bool =
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM pg_timezone_names WHERE name = $1)")
            .bind(tz)
            .fetch_one(db)
            .await?;
    ok.then(|| tz.to_string())
        .ok_or(AppError::Validation("invalid_time_zone"))
}

/// Start of `date` (local midnight in `tz`) as an instant.
async fn local_midnight(db: &PgPool, date: Date, tz: &str) -> AppResult<OffsetDateTime> {
    Ok(
        sqlx::query_scalar("SELECT ($1::date::timestamp AT TIME ZONE $2)")
            .bind(date)
            .bind(tz)
            .fetch_one(db)
            .await?,
    )
}

async fn local_today(db: &PgPool, tz: &str) -> AppResult<Date> {
    Ok(sqlx::query_scalar("SELECT (now() AT TIME ZONE $1)::date")
        .bind(tz)
        .fetch_one(db)
        .await?)
}

#[derive(Serialize, Default)]
struct StockValue {
    value_minor: i64,
    /// Parts on stock that have no cost at all: their value is missing.
    unvalued_parts: i64,
    /// Parts below zero: used, but their delivery was never recorded.
    negative_parts: i64,
}

async fn stock_value<'e, E>(db: E, at: OffsetDateTime) -> AppResult<StockValue>
where
    E: sqlx::Executor<'e, Database = Postgres>,
{
    let (value, unvalued, negative): (i64, i64, i64) = sqlx::query_as(&format!(
        "SELECT COALESCE(sum(round(s.qty::numeric * s.cost / 1000)) FILTER (WHERE s.qty > 0), 0)::bigint,
                count(*) FILTER (WHERE s.qty > 0 AND s.cost IS NULL),
                count(*) FILTER (WHERE s.qty < 0)
           FROM (SELECT m.part_id, sum(m.quantity_milli) AS qty, {cost} AS cost
                   FROM stock_movements m
                  WHERE m.created_at < $1
                  GROUP BY m.part_id) s",
        cost = cost_at("$1")
    ))
    .bind(at)
    .fetch_one(db)
    .await?;
    Ok(StockValue {
        value_minor: value,
        unvalued_parts: unvalued,
        negative_parts: negative,
    })
}

// ---------- months ----------

#[derive(Deserialize)]
struct MonthsQuery {
    year: i32,
    tz: Option<String>,
}

#[derive(Serialize)]
struct MonthRow {
    /// First day of the month.
    month: Date,
    /// Net purchase cost of everything received: by invoice date for
    /// deliveries, by the day it was recorded for a single receipt.
    purchased_net_minor: i64,
    purchased_lines: i64,
    /// Receipts recorded without a cost: their value is missing above.
    purchased_unpriced: i64,
    /// Parts that went into work orders, at purchase cost.
    used_cost_minor: i64,
    /// The same parts at the prices in the work orders.
    used_sale_minor: i64,
    used_lines: i64,
    /// Part lines without a price in their work order.
    used_unpriced: i64,
    /// Part lines whose part has no cost at all.
    used_unvalued: i64,
    /// Stock at the end of the month (now, for the current month).
    stock_end: StockValue,
}

#[derive(Serialize)]
struct MonthsReport {
    year: i32,
    currency: String,
    months: Vec<MonthRow>,
}

fn first_of(year: i32, month: Month) -> AppResult<Date> {
    Date::from_calendar_date(year, month, 1).map_err(|_| AppError::Validation("invalid_year"))
}

async fn months(
    State(state): State<AppState>,
    _owner: Owner,
    Query(query): Query<MonthsQuery>,
) -> AppResult<Json<MonthsReport>> {
    if !(2000..=2100).contains(&query.year) {
        return Err(AppError::Validation("invalid_year"));
    }
    let tz = valid_tz(&state.db, query.tz.as_deref()).await?;
    let today = local_today(&state.db, &tz).await?;
    let now = OffsetDateTime::now_utc();

    let mut months = Vec::new();
    let mut month = Month::January;
    loop {
        let start = first_of(query.year, month)?;
        if start > today {
            break;
        }
        let next = if month == Month::December {
            first_of(query.year + 1, Month::January)?
        } else {
            first_of(query.year, month.next())?
        };
        let t_start = local_midnight(&state.db, start, &tz).await?;
        let t_end = local_midnight(&state.db, next, &tz).await?;

        let (purchased, purchased_lines, purchased_unpriced): (i64, i64, i64) = sqlx::query_as(
            "SELECT COALESCE(sum(round(m.quantity_milli::numeric * m.unit_cost_minor / 1000)), 0)::bigint,
                    count(*) FILTER (WHERE m.unit_cost_minor IS NOT NULL),
                    count(*) FILTER (WHERE m.unit_cost_minor IS NULL)
               FROM stock_movements m
               LEFT JOIN delivery_items di ON di.id = m.delivery_item_id
               LEFT JOIN deliveries d ON d.id = di.delivery_id
              WHERE m.kind = 'receipt'
                AND CASE WHEN d.id IS NOT NULL
                         THEN d.invoice_date >= $1 AND d.invoice_date < $2
                         ELSE m.created_at >= $3 AND m.created_at < $4 END",
        )
        .bind(start)
        .bind(next)
        .bind(t_start)
        .bind(t_end)
        .fetch_one(&state.db)
        .await?;

        let (used_cost, used_sale, used_lines, used_unpriced, used_unvalued): (i64, i64, i64, i64, i64) =
            sqlx::query_as(&format!(
                "SELECT COALESCE(sum(round(-u.qty::numeric * u.cost / 1000)), 0)::bigint,
                        COALESCE(sum(round(i.quantity_milli::numeric * i.unit_price_minor / 1000)), 0)::bigint,
                        count(*),
                        count(*) FILTER (WHERE i.unit_price_minor IS NULL),
                        count(*) FILTER (WHERE u.cost IS NULL)
                   FROM (SELECT m.work_order_item_id, m.quantity_milli AS qty, {cost} AS cost
                           FROM stock_movements m
                          WHERE m.kind = 'consumption' AND m.created_at >= $1 AND m.created_at < $2) u
                   JOIN work_order_items i ON i.id = u.work_order_item_id",
                cost = cost_at("m.created_at")
            ))
            .bind(t_start)
            .bind(t_end)
            .fetch_one(&state.db)
            .await?;

        let stock_end = stock_value(&state.db, t_end.min(now)).await?;
        months.push(MonthRow {
            month: start,
            purchased_net_minor: purchased,
            purchased_lines,
            purchased_unpriced,
            used_cost_minor: used_cost,
            used_sale_minor: used_sale,
            used_lines,
            used_unpriced,
            used_unvalued,
            stock_end,
        });
        if month == Month::December {
            break;
        }
        month = month.next();
    }

    Ok(Json(MonthsReport {
        year: query.year,
        currency: settings::load(&state.db).await?.currency,
        months,
    }))
}

// ---------- stock on a date ----------

#[derive(Deserialize)]
struct StockQuery {
    /// Stock at the end of this day; today if absent.
    date: Option<Date>,
    tz: Option<String>,
}

#[derive(Serialize, sqlx::FromRow)]
struct StockRow {
    id: Uuid,
    name: String,
    brand: Option<String>,
    part_number: Option<String>,
    condition: String,
    unit: String,
    location: Option<String>,
    quantity_milli: i64,
    unit_cost_minor: Option<i64>,
    value_minor: Option<i64>,
}

#[derive(Serialize)]
struct StockReport {
    date: Date,
    currency: String,
    total: StockValue,
    /// Parts with stock above zero, by name.
    rows: Vec<StockRow>,
}

async fn stock(
    State(state): State<AppState>,
    _owner: Owner,
    Query(query): Query<StockQuery>,
) -> AppResult<Json<StockReport>> {
    let tz = valid_tz(&state.db, query.tz.as_deref()).await?;
    let date = match query.date {
        Some(d) => d,
        None => local_today(&state.db, &tz).await?,
    };
    let next = date
        .next_day()
        .ok_or(AppError::Validation("invalid_date"))?;
    let at = local_midnight(&state.db, next, &tz)
        .await?
        .min(OffsetDateTime::now_utc());

    // One snapshot for the rows and the total, so they always agree.
    let mut tx = state.db.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY")
        .execute(&mut *tx)
        .await?;
    let rows = sqlx::query_as::<_, StockRow>(&format!(
        "SELECT p.id, p.name, p.brand, p.part_number, p.condition, p.unit, p.location,
                s.qty AS quantity_milli, s.cost AS unit_cost_minor,
                round(s.qty::numeric * s.cost / 1000)::bigint AS value_minor
           FROM (SELECT m.part_id, sum(m.quantity_milli)::bigint AS qty, {cost} AS cost
                   FROM stock_movements m
                  WHERE m.created_at < $1
                  GROUP BY m.part_id) s
           JOIN parts p ON p.id = s.part_id
          WHERE s.qty > 0
          ORDER BY lower(p.name), p.brand, p.condition",
        cost = cost_at("$1")
    ))
    .bind(at)
    .fetch_all(&mut *tx)
    .await?;
    let total = stock_value(&mut *tx, at).await?;
    tx.commit().await?;

    Ok(Json(StockReport {
        date,
        currency: settings::load(&state.db).await?.currency,
        total,
        rows,
    }))
}
