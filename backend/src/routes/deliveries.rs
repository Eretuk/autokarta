//! Supplier deliveries: a purchase invoice (faktura) with its lines and
//! files. A delivery is a draft until it is posted; posting puts every line
//! on stock as a receipt with its net purchase cost, reopening takes them
//! off again. Everything here is financial data: owners only.

use axum::{
    body::Bytes,
    extract::{DefaultBodyLimit, Path, Query, State},
    http::{header, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, patch, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sqlx::PgConnection;
use time::{Date, OffsetDateTime};
use uuid::Uuid;

use crate::{
    auth::Owner,
    error::{conflict_on, AppError, AppResult},
    routes::{deserialize_some, like_pattern, settings},
    state::AppState,
    validate,
};

/// Large enough for a scanned multi-page invoice, small enough that a
/// mistaken upload of a video does not bloat the database (and its backups).
pub const MAX_FILE_BYTES: usize = 15 * 1024 * 1024;
const MAX_FILES_PER_DELIVERY: i64 = 20;

pub fn supplier_routes() -> Router<AppState> {
    Router::new()
        .route("/", get(search_suppliers).post(create_supplier))
        .route("/{id}", axum::routing::put(update_supplier))
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(list).post(create))
        .route("/{id}", get(detail).patch(update).delete(remove))
        .route("/{id}/items", post(add_item))
        .route(
            "/{id}/items/{item_id}",
            patch(update_item).delete(delete_item),
        )
        .route("/{id}/post", post(post_delivery))
        .route("/{id}/reopen", post(reopen))
        .route(
            "/{id}/files",
            post(upload_file).layer(DefaultBodyLimit::max(MAX_FILE_BYTES)),
        )
        .route(
            "/{id}/files/{file_id}",
            get(download_file).delete(delete_file),
        )
}

fn v<T>(r: Result<T, &'static str>) -> AppResult<T> {
    r.map_err(AppError::Validation)
}

// ---------- suppliers ----------

const SUPPLIER_UNIQUE: &[(&str, &str)] = &[("suppliers_tax_id_idx", "tax_id_taken")];

#[derive(Serialize, sqlx::FromRow)]
pub struct Supplier {
    id: Uuid,
    name: String,
    tax_id: Option<String>,
    note: Option<String>,
}

#[derive(Deserialize)]
struct SupplierQuery {
    #[serde(default)]
    q: String,
}

async fn search_suppliers(
    State(state): State<AppState>,
    _owner: Owner,
    Query(query): Query<SupplierQuery>,
) -> AppResult<Json<Vec<Supplier>>> {
    let q = query.q.trim();
    // A typed NIP matches with or without dashes.
    let tax_key: String = q.chars().filter(char::is_ascii_alphanumeric).collect();
    let rows = sqlx::query_as::<_, Supplier>(
        "SELECT id, name, tax_id, note FROM suppliers
          WHERE $1 = '' OR lower(name) LIKE $2 OR ($3 <> '' AND tax_id LIKE $4)
          ORDER BY lower(name)
          LIMIT 50",
    )
    .bind(q)
    .bind(like_pattern(&q.to_lowercase()))
    .bind(&tax_key)
    .bind(like_pattern(&tax_key.to_uppercase()))
    .fetch_all(&state.db)
    .await?;
    Ok(Json(rows))
}

#[derive(Deserialize)]
struct SupplierInput {
    name: String,
    tax_id: Option<String>,
    note: Option<String>,
}

fn clean_supplier(req: &SupplierInput) -> AppResult<(String, Option<String>, Option<String>)> {
    let name = v(validate::optional_text(Some(&req.name), 160))?
        .ok_or(AppError::Validation("invalid_supplier_name"))?;
    Ok((
        name,
        v(validate::tax_id(req.tax_id.as_deref()))?,
        v(validate::optional_text(req.note.as_deref(), 2000))?,
    ))
}

async fn create_supplier(
    State(state): State<AppState>,
    _owner: Owner,
    Json(req): Json<SupplierInput>,
) -> AppResult<(StatusCode, Json<Supplier>)> {
    let (name, tax_id, note) = clean_supplier(&req)?;
    let row = sqlx::query_as::<_, Supplier>(
        "INSERT INTO suppliers (name, tax_id, note) VALUES ($1, $2, $3)
         RETURNING id, name, tax_id, note",
    )
    .bind(name)
    .bind(tax_id)
    .bind(note)
    .fetch_one(&state.db)
    .await
    .map_err(|e| conflict_on(e, SUPPLIER_UNIQUE))?;
    Ok((StatusCode::CREATED, Json(row)))
}

async fn update_supplier(
    State(state): State<AppState>,
    _owner: Owner,
    Path(id): Path<Uuid>,
    Json(req): Json<SupplierInput>,
) -> AppResult<Json<Supplier>> {
    let (name, tax_id, note) = clean_supplier(&req)?;
    let row = sqlx::query_as::<_, Supplier>(
        "UPDATE suppliers SET name = $2, tax_id = $3, note = $4, updated_at = now()
          WHERE id = $1
         RETURNING id, name, tax_id, note",
    )
    .bind(id)
    .bind(name)
    .bind(tax_id)
    .bind(note)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| conflict_on(e, SUPPLIER_UNIQUE))?
    .ok_or(AppError::NotFound)?;
    Ok(Json(row))
}

// ---------- list ----------

const DELIVERY_UNIQUE: &[(&str, &str)] = &[("deliveries_invoice_idx", "invoice_already_entered")];

/// Net and gross totals of a delivery's lines. Each line is rounded on
/// its own, like the line totals on the invoice.
const TOTALS: &str = "
    COALESCE(sum(round(i.quantity_milli::numeric * i.unit_cost_minor / 1000)), 0)::bigint AS total_net_minor,
    COALESCE(sum(round(round(i.quantity_milli::numeric * i.unit_cost_minor / 1000)
                       * (100 + i.vat_percent) / 100)), 0)::bigint AS total_gross_minor";

#[derive(Deserialize)]
struct ListQuery {
    status: Option<String>,
    #[serde(default)]
    q: String,
}

#[derive(Serialize, sqlx::FromRow)]
struct ListRow {
    id: Uuid,
    number: i64,
    status: String,
    supplier_id: Option<Uuid>,
    supplier_name: Option<String>,
    invoice_number: Option<String>,
    invoice_date: Date,
    item_count: i64,
    file_count: i64,
    total_net_minor: i64,
    total_gross_minor: i64,
}

async fn list(
    State(state): State<AppState>,
    _owner: Owner,
    Query(query): Query<ListQuery>,
) -> AppResult<Json<Vec<ListRow>>> {
    let status = query
        .status
        .as_deref()
        .filter(|s| *s != "all")
        .map(|s| validate::one_of(s, &["draft", "posted"], "invalid_status"))
        .transpose()
        .map_err(AppError::Validation)?;
    let q = query.q.trim();
    let rows = sqlx::query_as::<_, ListRow>(&format!(
        "SELECT d.id, d.number, d.status, d.supplier_id, s.name AS supplier_name,
                d.invoice_number, d.invoice_date,
                count(i.id) AS item_count,
                (SELECT count(*) FROM delivery_files f WHERE f.delivery_id = d.id) AS file_count,
                {TOTALS}
           FROM deliveries d
           LEFT JOIN suppliers s ON s.id = d.supplier_id
           LEFT JOIN delivery_items i ON i.delivery_id = d.id
          WHERE ($1::text IS NULL OR d.status = $1)
            AND ($2 = '' OR lower(coalesce(s.name, '')) LIKE $3
                 OR lower(coalesce(d.invoice_number, '')) LIKE $3
                 OR d.number::text = $2)
          GROUP BY d.id, s.name
          ORDER BY d.status = 'draft' DESC, d.invoice_date DESC, d.number DESC
          LIMIT 200"
    ))
    .bind(status)
    .bind(q)
    .bind(like_pattern(&q.to_lowercase()))
    .fetch_all(&state.db)
    .await?;
    Ok(Json(rows))
}

// ---------- create / update / delete ----------

#[derive(Deserialize)]
struct CreateDelivery {
    supplier_id: Option<Uuid>,
    invoice_number: Option<String>,
    invoice_date: Option<Date>,
    invoice_total_net_minor: Option<i64>,
    note: Option<String>,
}

async fn ensure_supplier(conn: &mut PgConnection, id: Option<Uuid>) -> AppResult<()> {
    if let Some(id) = id {
        let exists: bool =
            sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM suppliers WHERE id = $1)")
                .bind(id)
                .fetch_one(conn)
                .await?;
        if !exists {
            return Err(AppError::Validation("unknown_supplier"));
        }
    }
    Ok(())
}

fn invoice_number(raw: Option<&str>) -> AppResult<Option<String>> {
    v(validate::optional_text(raw, 60))
}

async fn create(
    State(state): State<AppState>,
    Owner(owner): Owner,
    Json(req): Json<CreateDelivery>,
) -> AppResult<(StatusCode, Json<DeliveryDetail>)> {
    let invoice_number = invoice_number(req.invoice_number.as_deref())?;
    let total = v(validate::price_minor(req.invoice_total_net_minor))?;
    let note = v(validate::optional_text(req.note.as_deref(), 2000))?;
    let mut tx = state.db.begin().await?;
    ensure_supplier(&mut tx, req.supplier_id).await?;
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO deliveries
             (supplier_id, invoice_number, invoice_date, invoice_total_net_minor, note, created_by)
         VALUES ($1, $2, COALESCE($3, current_date), $4, $5, $6)
         RETURNING id",
    )
    .bind(req.supplier_id)
    .bind(invoice_number)
    .bind(req.invoice_date)
    .bind(total)
    .bind(note)
    .bind(owner.id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| conflict_on(e, DELIVERY_UNIQUE))?;
    tx.commit().await?;
    tracing::info!(delivery = %id, "delivery created");
    Ok((StatusCode::CREATED, Json(load_detail(&state, id).await?)))
}

/// Every field optional; `null` clears the optional ones. The header may
/// be corrected after posting too: it does not change the stock.
#[derive(Deserialize)]
struct UpdateDelivery {
    #[serde(default, deserialize_with = "deserialize_some")]
    supplier_id: Option<Option<Uuid>>,
    #[serde(default, deserialize_with = "deserialize_some")]
    invoice_number: Option<Option<String>>,
    invoice_date: Option<Date>,
    #[serde(default, deserialize_with = "deserialize_some")]
    invoice_total_net_minor: Option<Option<i64>>,
    #[serde(default, deserialize_with = "deserialize_some")]
    note: Option<Option<String>>,
}

async fn update(
    State(state): State<AppState>,
    _owner: Owner,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateDelivery>,
) -> AppResult<Json<DeliveryDetail>> {
    let invoice = req
        .invoice_number
        .as_ref()
        .map(|n| invoice_number(n.as_deref()))
        .transpose()?;
    let total = req
        .invoice_total_net_minor
        .map(|t| v(validate::price_minor(t)))
        .transpose()?;
    let note = req
        .note
        .as_ref()
        .map(|n| v(validate::optional_text(n.as_deref(), 2000)))
        .transpose()?;

    let mut tx = state.db.begin().await?;
    lock_delivery(&mut tx, id).await?;
    if let Some(supplier) = req.supplier_id {
        ensure_supplier(&mut tx, supplier).await?;
    }
    sqlx::query(
        "UPDATE deliveries
            SET supplier_id = CASE WHEN $2 THEN $3 ELSE supplier_id END,
                invoice_number = CASE WHEN $4 THEN $5 ELSE invoice_number END,
                invoice_date = COALESCE($6, invoice_date),
                invoice_total_net_minor = CASE WHEN $7 THEN $8 ELSE invoice_total_net_minor END,
                note = CASE WHEN $9 THEN $10 ELSE note END,
                updated_at = now()
          WHERE id = $1",
    )
    .bind(id)
    .bind(req.supplier_id.is_some())
    .bind(req.supplier_id.flatten())
    .bind(invoice.is_some())
    .bind(invoice.flatten())
    .bind(req.invoice_date)
    .bind(total.is_some())
    .bind(total.flatten())
    .bind(note.is_some())
    .bind(note.flatten())
    .execute(&mut *tx)
    .await
    .map_err(|e| conflict_on(e, DELIVERY_UNIQUE))?;
    tx.commit().await?;
    Ok(Json(load_detail(&state, id).await?))
}

/// Only a draft can be deleted: a posted delivery is reopened first, so
/// stock never loses a receipt without someone deciding so.
async fn remove(
    State(state): State<AppState>,
    _owner: Owner,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    let mut tx = state.db.begin().await?;
    lock_draft(&mut tx, id).await?;
    sqlx::query("DELETE FROM deliveries WHERE id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    tracing::info!(delivery = %id, "draft delivery deleted");
    Ok(StatusCode::NO_CONTENT)
}

/// Locks the delivery row; returns its status.
async fn lock_delivery(conn: &mut PgConnection, id: Uuid) -> AppResult<String> {
    sqlx::query_scalar("SELECT status FROM deliveries WHERE id = $1 FOR UPDATE")
        .bind(id)
        .fetch_optional(conn)
        .await?
        .ok_or(AppError::NotFound)
}

async fn lock_draft(conn: &mut PgConnection, id: Uuid) -> AppResult<()> {
    if lock_delivery(conn, id).await? != "draft" {
        return Err(AppError::Conflict("delivery_posted"));
    }
    Ok(())
}

async fn touch(conn: &mut PgConnection, id: Uuid) -> AppResult<()> {
    sqlx::query("UPDATE deliveries SET updated_at = now() WHERE id = $1")
        .bind(id)
        .execute(conn)
        .await?;
    Ok(())
}

// ---------- items ----------

#[derive(Deserialize)]
struct AddItem {
    part_id: Option<Uuid>,
    /// What the invoice calls it; required while no part is chosen.
    description: Option<String>,
    /// The supplier's article as printed on the invoice.
    part_number: Option<String>,
    quantity_milli: i64,
    unit_cost_minor: i64,
    vat_percent: Option<i64>,
}

async fn ensure_part(conn: &mut PgConnection, id: Option<Uuid>) -> AppResult<()> {
    if let Some(id) = id {
        let exists: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM parts WHERE id = $1)")
            .bind(id)
            .fetch_one(conn)
            .await?;
        if !exists {
            return Err(AppError::Validation("unknown_part"));
        }
    }
    Ok(())
}

async fn add_item(
    State(state): State<AppState>,
    _owner: Owner,
    Path(id): Path<Uuid>,
    Json(req): Json<AddItem>,
) -> AppResult<(StatusCode, Json<DeliveryDetail>)> {
    let description = v(validate::optional_text(req.description.as_deref(), 160))?;
    if description.is_none() && req.part_id.is_none() {
        return Err(AppError::Validation("invalid_description"));
    }
    let part_number = v(validate::optional_text(req.part_number.as_deref(), 60))?;
    let quantity = v(validate::quantity_milli(req.quantity_milli))?;
    let cost = v(validate::price_minor(Some(req.unit_cost_minor)))?;
    let vat = v(validate::vat_percent(req.vat_percent.unwrap_or(23)))?;

    let mut tx = state.db.begin().await?;
    lock_draft(&mut tx, id).await?;
    ensure_part(&mut tx, req.part_id).await?;
    sqlx::query(
        "INSERT INTO delivery_items
             (delivery_id, part_id, description, part_number, quantity_milli, unit_cost_minor, vat_percent)
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(id)
    .bind(req.part_id)
    .bind(description)
    .bind(part_number)
    .bind(quantity)
    .bind(cost)
    .bind(vat)
    .execute(&mut *tx)
    .await?;
    touch(&mut tx, id).await?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(load_detail(&state, id).await?)))
}

#[derive(Deserialize)]
struct UpdateItem {
    #[serde(default, deserialize_with = "deserialize_some")]
    part_id: Option<Option<Uuid>>,
    #[serde(default, deserialize_with = "deserialize_some")]
    description: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_some")]
    part_number: Option<Option<String>>,
    quantity_milli: Option<i64>,
    unit_cost_minor: Option<i64>,
    vat_percent: Option<i64>,
}

async fn update_item(
    State(state): State<AppState>,
    _owner: Owner,
    Path((id, item_id)): Path<(Uuid, Uuid)>,
    Json(req): Json<UpdateItem>,
) -> AppResult<Json<DeliveryDetail>> {
    let description = req
        .description
        .as_ref()
        .map(|d| v(validate::optional_text(d.as_deref(), 160)))
        .transpose()?;
    let part_number = req
        .part_number
        .as_ref()
        .map(|n| v(validate::optional_text(n.as_deref(), 60)))
        .transpose()?;
    let quantity = req
        .quantity_milli
        .map(|q| v(validate::quantity_milli(q)))
        .transpose()?;
    let cost = req
        .unit_cost_minor
        .map(|c| v(validate::price_minor(Some(c))))
        .transpose()?
        .flatten();
    let vat = req
        .vat_percent
        .map(|p| v(validate::vat_percent(p)))
        .transpose()?;

    let mut tx = state.db.begin().await?;
    lock_draft(&mut tx, id).await?;
    if let Some(part) = req.part_id {
        ensure_part(&mut tx, part).await?;
    }
    let updated = sqlx::query(
        "UPDATE delivery_items
            SET part_id = CASE WHEN $3 THEN $4 ELSE part_id END,
                description = CASE WHEN $5 THEN $6 ELSE description END,
                part_number = CASE WHEN $7 THEN $8 ELSE part_number END,
                quantity_milli = COALESCE($9, quantity_milli),
                unit_cost_minor = COALESCE($10, unit_cost_minor),
                vat_percent = COALESCE($11, vat_percent),
                updated_at = now()
          WHERE id = $2 AND delivery_id = $1",
    )
    .bind(id)
    .bind(item_id)
    .bind(req.part_id.is_some())
    .bind(req.part_id.flatten())
    .bind(description.is_some())
    .bind(description.flatten())
    .bind(part_number.is_some())
    .bind(part_number.flatten())
    .bind(quantity)
    .bind(cost)
    .bind(vat)
    .execute(&mut *tx)
    .await
    .map_err(|e| match &e {
        // Clearing both the part and the description.
        sqlx::Error::Database(d) if d.is_check_violation() => {
            AppError::Validation("invalid_description")
        }
        _ => e.into(),
    })?;
    if updated.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    touch(&mut tx, id).await?;
    tx.commit().await?;
    Ok(Json(load_detail(&state, id).await?))
}

async fn delete_item(
    State(state): State<AppState>,
    _owner: Owner,
    Path((id, item_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<DeliveryDetail>> {
    let mut tx = state.db.begin().await?;
    lock_draft(&mut tx, id).await?;
    let deleted = sqlx::query("DELETE FROM delivery_items WHERE id = $2 AND delivery_id = $1")
        .bind(id)
        .bind(item_id)
        .execute(&mut *tx)
        .await?;
    if deleted.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    touch(&mut tx, id).await?;
    tx.commit().await?;
    Ok(Json(load_detail(&state, id).await?))
}

// ---------- post / reopen ----------

#[derive(Deserialize)]
struct NewPrice {
    part_id: Uuid,
    sell_price_minor: Option<i64>,
}

#[derive(Deserialize)]
struct PostDelivery {
    /// New shelf prices to save together with the delivery, usually the
    /// suggested ones the owner accepted.
    #[serde(default)]
    prices: Vec<NewPrice>,
}

/// Locks the parts of a delivery, in id order like every other multi-part
/// stock change, so two postings cannot deadlock.
async fn lock_delivery_parts(conn: &mut PgConnection, id: Uuid) -> AppResult<()> {
    sqlx::query(
        "SELECT 1 FROM parts WHERE id IN
            (SELECT part_id FROM delivery_items WHERE delivery_id = $1 AND part_id IS NOT NULL)
          ORDER BY id FOR UPDATE",
    )
    .bind(id)
    .execute(conn)
    .await?;
    Ok(())
}

async fn post_delivery(
    State(state): State<AppState>,
    Owner(owner): Owner,
    Path(id): Path<Uuid>,
    Json(req): Json<PostDelivery>,
) -> AppResult<Json<DeliveryDetail>> {
    let prices = req
        .prices
        .iter()
        .map(|p| Ok((p.part_id, v(validate::price_minor(p.sell_price_minor))?)))
        .collect::<AppResult<Vec<_>>>()?;

    let mut tx = state.db.begin().await?;
    lock_draft(&mut tx, id).await?;
    let (lines, unmatched): (i64, i64) = sqlx::query_as(
        "SELECT count(*), count(*) FILTER (WHERE part_id IS NULL)
           FROM delivery_items WHERE delivery_id = $1",
    )
    .bind(id)
    .fetch_one(&mut *tx)
    .await?;
    if lines == 0 {
        return Err(AppError::Validation("delivery_empty"));
    }
    if unmatched > 0 {
        return Err(AppError::Conflict("delivery_unmatched_lines"));
    }
    lock_delivery_parts(&mut tx, id).await?;
    sqlx::query(
        "INSERT INTO stock_movements
             (part_id, kind, quantity_milli, unit_cost_minor, delivery_item_id, created_by)
         SELECT part_id, 'receipt', quantity_milli, unit_cost_minor, id, $2
           FROM delivery_items WHERE delivery_id = $1",
    )
    .bind(id)
    .bind(owner.id)
    .execute(&mut *tx)
    .await?;
    for (part_id, price) in prices {
        let updated = sqlx::query(
            "UPDATE parts SET sell_price_minor = $2, updated_at = now()
              WHERE id = $1
                AND id IN (SELECT part_id FROM delivery_items WHERE delivery_id = $3)",
        )
        .bind(part_id)
        .bind(price)
        .bind(id)
        .execute(&mut *tx)
        .await?;
        if updated.rows_affected() == 0 {
            return Err(AppError::Validation("unknown_part"));
        }
    }
    sqlx::query(
        "UPDATE deliveries SET status = 'posted', posted_at = now(), posted_by = $2, updated_at = now()
          WHERE id = $1",
    )
    .bind(id)
    .bind(owner.id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    tracing::info!(delivery = %id, lines, "delivery posted");
    Ok(Json(load_detail(&state, id).await?))
}

/// Back to draft to fix a mistake: the receipts come off stock (which may
/// go negative if parts were already used; that is the truth).
async fn reopen(
    State(state): State<AppState>,
    _owner: Owner,
    Path(id): Path<Uuid>,
) -> AppResult<Json<DeliveryDetail>> {
    let mut tx = state.db.begin().await?;
    if lock_delivery(&mut tx, id).await? != "posted" {
        return Err(AppError::Conflict("delivery_not_posted"));
    }
    lock_delivery_parts(&mut tx, id).await?;
    sqlx::query(
        "DELETE FROM stock_movements
          WHERE delivery_item_id IN (SELECT id FROM delivery_items WHERE delivery_id = $1)",
    )
    .bind(id)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "UPDATE deliveries SET status = 'draft', posted_at = NULL, posted_by = NULL, updated_at = now()
          WHERE id = $1",
    )
    .bind(id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    tracing::info!(delivery = %id, "delivery reopened");
    Ok(Json(load_detail(&state, id).await?))
}

// ---------- files ----------

/// What the bytes are, decided from the bytes: the browser's
/// Content-Type and the file name are not trusted.
fn sniff(data: &[u8]) -> Option<&'static str> {
    let text = data.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(data);
    let first = text.iter().position(|b| !b.is_ascii_whitespace());
    if data.starts_with(b"%PDF-") {
        Some("application/pdf")
    } else if data.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if data.starts_with(b"\xFF\xD8\xFF") {
        Some("image/jpeg")
    } else if data.len() > 12 && &data[..4] == b"RIFF" && &data[8..12] == b"WEBP" {
        Some("image/webp")
    } else if first.is_some_and(|i| text[i..].starts_with(b"<?xml") || text[i..].starts_with(b"<"))
    {
        // A KSeF invoice. Served only as a download, never rendered.
        Some("application/xml")
    } else {
        None
    }
}

/// A file name safe to store and to send back in Content-Disposition.
fn clean_file_name(raw: Option<&str>) -> String {
    let base = raw
        .unwrap_or_default()
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or_default();
    let name: String = base
        .chars()
        .filter(|c| !c.is_control() && !matches!(c, '"' | '\\'))
        .take(200)
        .collect();
    let name = name.trim();
    if name.is_empty() || name.chars().all(|c| c == '.') {
        "invoice".to_string()
    } else {
        name.to_string()
    }
}

#[derive(Deserialize)]
struct UploadQuery {
    name: Option<String>,
}

/// The file is the raw request body (no multipart); its name comes in the
/// query string.
async fn upload_file(
    State(state): State<AppState>,
    Owner(owner): Owner,
    Path(id): Path<Uuid>,
    Query(query): Query<UploadQuery>,
    body: Bytes,
) -> AppResult<(StatusCode, Json<DeliveryDetail>)> {
    if body.is_empty() {
        return Err(AppError::Validation("file_empty"));
    }
    let content_type = sniff(&body).ok_or(AppError::Validation("unsupported_file_type"))?;
    let name = clean_file_name(query.name.as_deref());

    let mut tx = state.db.begin().await?;
    lock_delivery(&mut tx, id).await?;
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM delivery_files WHERE delivery_id = $1")
            .bind(id)
            .fetch_one(&mut *tx)
            .await?;
    if count >= MAX_FILES_PER_DELIVERY {
        return Err(AppError::Conflict("too_many_files"));
    }
    sqlx::query(
        "INSERT INTO delivery_files (delivery_id, file_name, content_type, size_bytes, data, created_by)
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(id)
    .bind(&name)
    .bind(content_type)
    .bind(body.len() as i32)
    .bind(body.as_ref())
    .bind(owner.id)
    .execute(&mut *tx)
    .await?;
    touch(&mut tx, id).await?;
    tx.commit().await?;
    tracing::info!(delivery = %id, bytes = body.len(), content_type, "delivery file attached");
    Ok((StatusCode::CREATED, Json(load_detail(&state, id).await?)))
}

/// RFC 5987 `filename*` value: UTF-8, percent-encoded.
fn encode_rfc5987(name: &str) -> String {
    let mut out = String::with_capacity(name.len() * 3);
    for b in name.bytes() {
        if b.is_ascii_alphanumeric() || b"!#$&+-.^_`|~".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

async fn download_file(
    State(state): State<AppState>,
    _owner: Owner,
    Path((id, file_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Response> {
    let (name, content_type, data): (String, String, Vec<u8>) = sqlx::query_as(
        "SELECT file_name, content_type, data FROM delivery_files WHERE id = $2 AND delivery_id = $1",
    )
    .bind(id)
    .bind(file_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;
    // PDFs and pictures open in the browser; anything else (XML) is only
    // ever downloaded, so it cannot run as a page on our origin.
    let inline = content_type == "application/pdf" || content_type.starts_with("image/");
    let disposition = if inline { "inline" } else { "attachment" };
    let ascii: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_graphic() || c == ' ' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let disposition = format!(
        "{disposition}; filename=\"{ascii}\"; filename*=UTF-8''{}",
        encode_rfc5987(&name)
    );
    let mut res = data.into_response();
    let h = res.headers_mut();
    h.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(&content_type).map_err(|e| AppError::Internal(e.into()))?,
    );
    h.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&disposition).map_err(|e| AppError::Internal(e.into()))?,
    );
    h.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-store"),
    );
    // A downloaded file that a browser still decides to render gets no
    // scripts and no origin. Not for PDFs: Chrome refuses to show a PDF
    // in a sandboxed document, and its viewer runs isolated anyway.
    if !inline {
        h.insert(
            header::CONTENT_SECURITY_POLICY,
            HeaderValue::from_static("sandbox; default-src 'none'"),
        );
    }
    Ok(res)
}

async fn delete_file(
    State(state): State<AppState>,
    _owner: Owner,
    Path((id, file_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<DeliveryDetail>> {
    let deleted = sqlx::query("DELETE FROM delivery_files WHERE id = $2 AND delivery_id = $1")
        .bind(id)
        .bind(file_id)
        .execute(&state.db)
        .await?;
    if deleted.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    tracing::info!(delivery = %id, file = %file_id, "delivery file deleted");
    Ok(Json(load_detail(&state, id).await?))
}

// ---------- detail ----------

#[derive(Serialize, sqlx::FromRow)]
struct DeliveryRow {
    id: Uuid,
    number: i64,
    status: String,
    supplier_id: Option<Uuid>,
    supplier_name: Option<String>,
    supplier_tax_id: Option<String>,
    invoice_number: Option<String>,
    invoice_date: Date,
    invoice_total_net_minor: Option<i64>,
    note: Option<String>,
    #[serde(with = "time::serde::rfc3339::option")]
    posted_at: Option<OffsetDateTime>,
    posted_by_name: Option<String>,
}

#[derive(Serialize, sqlx::FromRow)]
struct ItemRow {
    id: Uuid,
    part_id: Option<Uuid>,
    description: Option<String>,
    part_number: Option<String>,
    quantity_milli: i64,
    unit_cost_minor: i64,
    vat_percent: i16,
    part_name: Option<String>,
    part_brand: Option<String>,
    part_part_number: Option<String>,
    part_condition: Option<String>,
    part_unit: Option<String>,
    /// The part's current shelf price.
    part_sell_price_minor: Option<i64>,
    /// The last known cost before this delivery, to show a price change.
    previous_cost_minor: Option<i64>,
}

#[derive(Serialize)]
struct Item {
    #[serde(flatten)]
    row: ItemRow,
    total_net_minor: i64,
    /// Sale price from the business markup; absent without a markup or a part.
    #[serde(skip_serializing_if = "Option::is_none")]
    suggested_price_minor: Option<i64>,
}

#[derive(Serialize, sqlx::FromRow)]
struct FileRow {
    id: Uuid,
    file_name: String,
    content_type: String,
    size_bytes: i32,
    #[serde(with = "time::serde::rfc3339")]
    created_at: OffsetDateTime,
}

#[derive(Serialize)]
struct Totals {
    net_minor: i64,
    vat_minor: i64,
    gross_minor: i64,
}

#[derive(Serialize)]
pub struct DeliveryDetail {
    #[serde(flatten)]
    delivery: DeliveryRow,
    items: Vec<Item>,
    files: Vec<FileRow>,
    totals: Totals,
    currency: String,
    markup_percent: Option<i16>,
}

async fn detail(
    State(state): State<AppState>,
    _owner: Owner,
    Path(id): Path<Uuid>,
) -> AppResult<Json<DeliveryDetail>> {
    Ok(Json(load_detail(&state, id).await?))
}

/// Gross sale price: net cost plus markup plus VAT, rounded up to a whole
/// currency unit (a shelf price of 41.00, not 40.59).
pub fn suggested_price(unit_cost_minor: i64, vat_percent: i16, markup_percent: i16) -> i64 {
    let raw =
        unit_cost_minor as i128 * (100 + markup_percent as i128) * (100 + vat_percent as i128);
    let minor = (raw + 9_999) / 10_000;
    let whole = (minor + 99) / 100 * 100;
    whole.min(validate::MAX_PRICE_MINOR as i128) as i64
}

async fn load_detail(state: &AppState, id: Uuid) -> AppResult<DeliveryDetail> {
    let delivery = sqlx::query_as::<_, DeliveryRow>(
        "SELECT d.id, d.number, d.status, d.supplier_id, s.name AS supplier_name,
                s.tax_id AS supplier_tax_id, d.invoice_number, d.invoice_date,
                d.invoice_total_net_minor, d.note, d.posted_at, u.display_name AS posted_by_name
           FROM deliveries d
           LEFT JOIN suppliers s ON s.id = d.supplier_id
           LEFT JOIN users u ON u.id = d.posted_by
          WHERE d.id = $1",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;

    let rows = sqlx::query_as::<_, ItemRow>(
        "SELECT i.id, i.part_id, i.description, i.part_number, i.quantity_milli,
                i.unit_cost_minor, i.vat_percent,
                p.name AS part_name, p.brand AS part_brand, p.part_number AS part_part_number,
                p.condition AS part_condition, p.unit AS part_unit,
                p.sell_price_minor AS part_sell_price_minor,
                (SELECT m.unit_cost_minor FROM stock_movements m
                  WHERE m.part_id = i.part_id AND m.unit_cost_minor IS NOT NULL
                    AND NOT EXISTS (SELECT 1 FROM delivery_items o
                                     WHERE o.id = m.delivery_item_id AND o.delivery_id = i.delivery_id)
                  ORDER BY m.created_at DESC LIMIT 1) AS previous_cost_minor
           FROM delivery_items i
           LEFT JOIN parts p ON p.id = i.part_id
          WHERE i.delivery_id = $1
          ORDER BY i.created_at, i.id",
    )
    .bind(id)
    .fetch_all(&state.db)
    .await?;

    let files = sqlx::query_as::<_, FileRow>(
        "SELECT id, file_name, content_type, size_bytes, created_at
           FROM delivery_files WHERE delivery_id = $1 ORDER BY created_at, id",
    )
    .bind(id)
    .fetch_all(&state.db)
    .await?;

    let business = settings::load(&state.db).await?;
    let markup = business.markup_percent.flatten();
    let mut totals = Totals {
        net_minor: 0,
        vat_minor: 0,
        gross_minor: 0,
    };
    let items = rows
        .into_iter()
        .map(|row| {
            let net = validate::line_total(row.quantity_milli, row.unit_cost_minor);
            let gross = (net as i128 * (100 + row.vat_percent as i128) + 50) / 100;
            totals.net_minor += net;
            totals.gross_minor += gross as i64;
            let suggested_price_minor = markup
                .filter(|_| row.part_id.is_some())
                .map(|m| suggested_price(row.unit_cost_minor, row.vat_percent, m));
            Item {
                row,
                total_net_minor: net,
                suggested_price_minor,
            }
        })
        .collect();
    totals.vat_minor = totals.gross_minor - totals.net_minor;

    Ok(DeliveryDetail {
        delivery,
        items,
        files,
        totals,
        currency: business.currency,
        markup_percent: markup,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suggests_whole_prices() {
        // 100.00 net, 30 % markup, 23 % VAT = 159.90, shown as 160.00.
        assert_eq!(suggested_price(10_000, 23, 30), 16_000);
        // Exactly whole stays as it is.
        assert_eq!(suggested_price(10_000, 0, 0), 10_000);
        assert_eq!(suggested_price(1, 23, 0), 100);
        assert_eq!(suggested_price(0, 23, 50), 0);
    }

    #[test]
    fn sniffs_files() {
        assert_eq!(sniff(b"%PDF-1.7 ..."), Some("application/pdf"));
        assert_eq!(
            sniff(b"\xEF\xBB\xBF  <?xml version=\"1.0\"?><Faktura/>"),
            Some("application/xml")
        );
        assert_eq!(sniff(b"\xFF\xD8\xFF\xE0...."), Some("image/jpeg"));
        assert_eq!(sniff(b"MZ\x90\x00"), None);
        assert_eq!(sniff(b"   "), None);
    }

    #[test]
    fn cleans_file_names() {
        assert_eq!(
            clean_file_name(Some("C:\\scans\\FV 12/2026.pdf")),
            "2026.pdf"
        );
        assert_eq!(clean_file_name(Some("../../etc/passwd")), "passwd");
        assert_eq!(clean_file_name(Some("a\"b\nc.pdf")), "abc.pdf");
        assert_eq!(clean_file_name(Some("..")), "invoice");
        assert_eq!(clean_file_name(None), "invoice");
        assert_eq!(encode_rfc5987("faktura ł.pdf"), "faktura%20%C5%82.pdf");
    }
}
