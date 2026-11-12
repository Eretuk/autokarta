//! Stock reports for the owner and the accountant: what was bought, what
//! went into work orders, and what is left on the shelves and is worth.
//!
//! Stock and the parts used are valued first in, first out at their net
//! purchase cost (see `crate::fifo`): used pieces are the oldest ones, so
//! what was used plus what is left adds up to what was paid. The layers are
//! rebuilt from the whole ledger on every request; a small workshop has a
//! few thousand movements at most. Move it to SQL if it ever gets slow.
//! Months follow the time zone the browser sends, since the database runs
//! in UTC and an invoice at 23:30 on the 31st belongs to that month.

use axum::{
    extract::{Query, State},
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use sqlx::{PgPool, Postgres};
use time::{Date, Month, OffsetDateTime};
use uuid::Uuid;

use crate::{
    auth::Owner,
    error::{AppError, AppResult},
    fifo,
    routes::settings,
    state::AppState,
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/months", get(months))
        .route("/stock", get(stock))
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

impl StockValue {
    fn add(&mut self, s: &fifo::Snapshot) {
        if s.quantity_milli > 0 {
            match s.value_minor {
                Some(v) => self.value_minor += v,
                None => self.unvalued_parts += 1,
            }
        } else if s.quantity_milli < 0 {
            self.negative_parts += 1;
        }
    }
}

#[derive(sqlx::FromRow)]
struct MoveRow {
    id: Uuid,
    part_id: Uuid,
    kind: String,
    quantity_milli: i64,
    unit_cost_minor: Option<i64>,
    created_at: OffsetDateTime,
}

/// The whole stock ledger valued FIFO, part by part, with snapshots at
/// `moments` (ascending).
struct Ledger {
    parts: Vec<PartLedger>,
}

struct PartLedger {
    part_id: Uuid,
    valuation: fifo::Valuation,
    /// Its consumption movements: id and when.
    used: Vec<(Uuid, OffsetDateTime)>,
}

impl Ledger {
    async fn load<'e, E>(db: E, moments: &[OffsetDateTime]) -> AppResult<Ledger>
    where
        E: sqlx::Executor<'e, Database = Postgres>,
    {
        let rows = sqlx::query_as::<_, MoveRow>(
            "SELECT id, part_id, kind, quantity_milli, unit_cost_minor, created_at
               FROM stock_movements
              ORDER BY part_id, created_at, id",
        )
        .fetch_all(db)
        .await?;
        let mut parts = Vec::new();
        for chunk in rows.chunk_by(|a, b| a.part_id == b.part_id) {
            let moves: Vec<fifo::Move> = chunk
                .iter()
                .map(|r| fifo::Move {
                    id: r.id,
                    at: r.created_at,
                    quantity_milli: r.quantity_milli,
                    unit_cost_minor: r.unit_cost_minor,
                })
                .collect();
            let used = chunk
                .iter()
                .filter(|r| r.kind == "consumption")
                .map(|r| (r.id, r.created_at))
                .collect();
            parts.push(PartLedger {
                part_id: chunk[0].part_id,
                valuation: fifo::value(&moves, moments),
                used,
            });
        }
        Ok(Ledger { parts })
    }

    fn stock(&self, moment: usize) -> StockValue {
        let mut total = StockValue::default();
        for p in &self.parts {
            total.add(&p.valuation.snapshots[moment]);
        }
        total
    }

    /// Cost of the parts used in `[from, to)`, and how many lines had no cost.
    fn used(&self, from: OffsetDateTime, to: OffsetDateTime) -> (i64, i64) {
        let (mut cost, mut unvalued) = (0, 0);
        for p in &self.parts {
            for (id, at) in &p.used {
                if *at < from || *at >= to {
                    continue;
                }
                // Consumption stores negative quantities: its cost is positive.
                match p.valuation.out_cost.get(id).copied().flatten() {
                    Some(c) => cost += c,
                    None => unvalued += 1,
                }
            }
        }
        (cost, unvalued)
    }
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

    // (first day, first day of the next month, both as instants)
    let mut windows = Vec::new();
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
        windows.push((start, next, t_start, t_end));
        if month == Month::December {
            break;
        }
        month = month.next();
    }

    // One snapshot of the ledger for every month, so the months agree.
    let mut tx = state.db.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY")
        .execute(&mut *tx)
        .await?;
    // Stock at the end of each month; now, for the current one.
    let moments: Vec<OffsetDateTime> = windows.iter().map(|w| w.3.min(now)).collect();
    let ledger = Ledger::load(&mut *tx, &moments).await?;

    let mut months = Vec::new();
    for (k, &(start, next, t_start, t_end)) in windows.iter().enumerate() {
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
        .fetch_one(&mut *tx)
        .await?;

        let (used_sale, used_lines, used_unpriced): (i64, i64, i64) = sqlx::query_as(
            "SELECT COALESCE(sum(round(i.quantity_milli::numeric * i.unit_price_minor / 1000)), 0)::bigint,
                    count(*),
                    count(*) FILTER (WHERE i.unit_price_minor IS NULL)
               FROM stock_movements m
               JOIN work_order_items i ON i.id = m.work_order_item_id
              WHERE m.kind = 'consumption' AND m.created_at >= $1 AND m.created_at < $2",
        )
        .bind(t_start)
        .bind(t_end)
        .fetch_one(&mut *tx)
        .await?;
        let (used_cost, used_unvalued) = ledger.used(t_start, t_end);

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
            stock_end: ledger.stock(k),
        });
    }
    tx.commit().await?;

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

#[derive(sqlx::FromRow)]
struct PartInfo {
    id: Uuid,
    name: String,
    brand: Option<String>,
    part_number: Option<String>,
    condition: String,
    unit: String,
    location: Option<String>,
}

#[derive(Serialize)]
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
    let ledger = Ledger::load(&mut *tx, &[at]).await?;
    let parts = sqlx::query_as::<_, PartInfo>(
        "SELECT id, name, brand, part_number, condition, unit, location FROM parts",
    )
    .fetch_all(&mut *tx)
    .await?;
    let mut info: HashMap<Uuid, PartInfo> = parts.into_iter().map(|p| (p.id, p)).collect();
    let mut total = StockValue::default();
    let mut rows = Vec::new();
    for pl in &ledger.parts {
        let snap = pl.valuation.snapshots[0];
        total.add(&snap);
        if snap.quantity_milli <= 0 {
            continue;
        }
        let Some(p) = info.remove(&pl.part_id) else {
            continue;
        };
        rows.push(StockRow {
            id: p.id,
            name: p.name,
            brand: p.brand,
            part_number: p.part_number,
            condition: p.condition,
            unit: p.unit,
            location: p.location,
            quantity_milli: snap.quantity_milli,
            // What is left may come from several purchases: the average.
            unit_cost_minor: snap
                .value_minor
                .map(|v| (v as i128 * 1000 / snap.quantity_milli as i128) as i64),
            value_minor: snap.value_minor,
        });
    }
    rows.sort_by(|a, b| {
        (a.name.to_lowercase(), &a.brand, &a.condition).cmp(&(
            b.name.to_lowercase(),
            &b.brand,
            &b.condition,
        ))
    });
    tx.commit().await?;

    Ok(Json(StockReport {
        date,
        currency: settings::load(&state.db).await?.currency,
        total,
        rows,
    }))
}
