//! The organization: its legal details (for printed work orders and, later,
//! telling purchase invoices from issued ones in KSeF) and business-wide
//! settings: currency, distance unit, default VAT and parts markup. These
//! are business decisions, independent of the UI language.

use axum::{extract::State, routing::get, Json, Router};
use serde::{Deserialize, Serialize};

use crate::{
    auth::{CurrentUser, Owner},
    error::{AppError, AppResult},
    routes::deserialize_some,
    state::AppState,
    validate,
};

pub fn routes() -> Router<AppState> {
    Router::new().route("/", get(get_settings).patch(update))
}

#[derive(Serialize, sqlx::FromRow)]
pub struct Organization {
    pub company_name: Option<String>,
    pub tax_id: Option<String>,
    pub regon: Option<String>,
    pub legal_address: Option<String>,
    pub service_address: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub contact_person: Option<String>,
    pub bank_account: Option<String>,
}

#[derive(Serialize)]
pub struct Settings {
    pub currency: String,
    pub distance_unit: String,
    pub default_vat_percent: i16,
    /// Default markup on the net purchase price for suggested sale prices;
    /// `null`: no suggestions. Owners only (absent for mechanics).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub markup_percent: Option<Option<i16>>,
    pub organization: Organization,
}

pub async fn load(db: &sqlx::PgPool) -> AppResult<Settings> {
    #[derive(sqlx::FromRow)]
    struct Row {
        currency: String,
        distance_unit: String,
        default_vat_percent: i16,
        markup_percent: Option<i16>,
        #[sqlx(flatten)]
        organization: Organization,
    }
    let r = sqlx::query_as::<_, Row>(
        "SELECT currency, distance_unit, default_vat_percent, markup_percent,
                company_name, tax_id, regon, legal_address, service_address, phone, email,
                contact_person, bank_account
           FROM business_settings",
    )
    .fetch_one(db)
    .await?;
    Ok(Settings {
        currency: r.currency,
        distance_unit: r.distance_unit,
        default_vat_percent: r.default_vat_percent,
        markup_percent: Some(r.markup_percent),
        organization: r.organization,
    })
}

async fn for_user(db: &sqlx::PgPool, user: &CurrentUser) -> AppResult<Settings> {
    let mut s = load(db).await?;
    if !user.is_owner() {
        s.markup_percent = None;
    }
    Ok(s)
}

async fn get_settings(
    State(state): State<AppState>,
    user: CurrentUser,
) -> AppResult<Json<Settings>> {
    Ok(Json(for_user(&state.db, &user).await?))
}

/// Every field optional: absent means unchanged; `null` (or an empty
/// string) clears the optional ones.
#[derive(Deserialize)]
struct UpdateSettings {
    currency: Option<String>,
    distance_unit: Option<String>,
    default_vat_percent: Option<i64>,
    #[serde(default, deserialize_with = "deserialize_some")]
    markup_percent: Option<Option<i64>>,
    #[serde(default)]
    organization: UpdateOrganization,
}

#[derive(Deserialize, Default)]
struct UpdateOrganization {
    #[serde(default, deserialize_with = "deserialize_some")]
    company_name: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_some")]
    tax_id: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_some")]
    regon: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_some")]
    legal_address: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_some")]
    service_address: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_some")]
    phone: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_some")]
    email: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_some")]
    contact_person: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_some")]
    bank_account: Option<Option<String>>,
}

type Field = Option<Option<String>>;

/// Validates a PATCH text field: `None` stays `None` (unchanged).
fn field(
    v: &Field,
    check: impl Fn(Option<&str>) -> Result<Option<String>, &'static str>,
) -> AppResult<Field> {
    v.as_ref()
        .map(|inner| check(inner.as_deref()))
        .transpose()
        .map_err(AppError::Validation)
}

async fn update(
    State(state): State<AppState>,
    Owner(owner): Owner,
    Json(req): Json<UpdateSettings>,
) -> AppResult<Json<Settings>> {
    let currency = req
        .currency
        .as_deref()
        .map(validate::currency)
        .transpose()
        .map_err(AppError::Validation)?;
    let unit = req
        .distance_unit
        .as_deref()
        .map(|u| validate::one_of(u, validate::DISTANCE_UNITS, "invalid_distance_unit"))
        .transpose()
        .map_err(AppError::Validation)?;
    let vat = req
        .default_vat_percent
        .map(validate::vat_percent)
        .transpose()
        .map_err(AppError::Validation)?;
    let markup = req
        .markup_percent
        .map(|m| m.map(validate::markup_percent).transpose())
        .transpose()
        .map_err(AppError::Validation)?;
    let o = &req.organization;
    let text = |max: usize| move |v: Option<&str>| validate::optional_text(v, max);
    let fields: [(&str, Field); 9] = [
        ("company_name", field(&o.company_name, text(160))?),
        ("tax_id", field(&o.tax_id, validate::tax_id)?),
        ("regon", field(&o.regon, validate::regon)?),
        ("legal_address", field(&o.legal_address, text(300))?),
        ("service_address", field(&o.service_address, text(300))?),
        ("phone", field(&o.phone, validate::phone)?),
        ("email", field(&o.email, validate::email)?),
        ("contact_person", field(&o.contact_person, text(120))?),
        (
            "bank_account",
            field(&o.bank_account, validate::bank_account)?,
        ),
    ];

    let mut tx = state.db.begin().await?;
    sqlx::query(
        "UPDATE business_settings SET currency = COALESCE($1, currency),
                                      distance_unit = COALESCE($2, distance_unit),
                                      default_vat_percent = COALESCE($3, default_vat_percent),
                                      markup_percent = CASE WHEN $4 THEN $5 ELSE markup_percent END,
                                      updated_at = now()",
    )
    .bind(currency)
    .bind(unit)
    .bind(vat)
    .bind(markup.is_some())
    .bind(markup.flatten())
    .execute(&mut *tx)
    .await?;
    for (column, value) in fields {
        if let Some(value) = value {
            // `column` comes from the fixed list above, never from the request.
            sqlx::query(&format!(
                "UPDATE business_settings SET {column} = $1, updated_at = now()"
            ))
            .bind(value)
            .execute(&mut *tx)
            .await?;
        }
    }
    tx.commit().await?;
    tracing::info!("organization settings changed");

    Ok(Json(for_user(&state.db, &owner).await?))
}
