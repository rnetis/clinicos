use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::{QueryBuilder, Row, Sqlite, SqlitePool};

use super::{like_term, paging, txt};
use crate::auth::{AuthUser, ClientIp};
use crate::db::{self, audit};
use crate::error::{ApiResult, AppError};
use crate::rbac;
use crate::state::AppState;
use crate::util;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/invoices", get(list).post(create))
        .route("/api/invoices/{id}", get(detail))
        .route("/api/invoices/{id}/payments", post(add_payment))
        .route("/api/invoices/{id}/refund", post(refund))
        .route("/api/invoices/{id}/cancel", post(cancel))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListQuery {
    pub status: Option<String>,
    pub patient_id: Option<String>,
    pub doctor_id: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub q: Option<String>,
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

pub async fn list(
    State(state): State<AppState>,
    user: AuthUser,
    Query(q): Query<ListQuery>,
) -> ApiResult<Json<Value>> {
    rbac::require(&user, rbac::INVOICE_VIEW)?;
    let (per, offset) = util::paginate(q.page, q.per_page);

    fn filters(qb: &mut QueryBuilder<'_, Sqlite>, q: &ListQuery) {
        qb.push(" WHERE 1 = 1");
        if let Some(status) = q.status.as_deref().filter(|s| !s.is_empty()) {
            qb.push(" AND b.status = ").push_bind(status.to_string());
        }
        if let Some(pid) = q.patient_id.as_deref().filter(|s| !s.is_empty()) {
            qb.push(" AND b.patient_id = ").push_bind(pid.to_string());
        }
        if let Some(doc) = q.doctor_id.as_deref().filter(|s| !s.is_empty()) {
            qb.push(" AND b.doctor_id = ").push_bind(doc.to_string());
        }
        if let Some(from) = q.from.as_deref().filter(|s| !s.is_empty()) {
            qb.push(" AND b.date >= ").push_bind(from.to_string());
        }
        if let Some(to) = q.to.as_deref().filter(|s| !s.is_empty()) {
            qb.push(" AND b.date <= ").push_bind(to.to_string());
        }
        if let Some(term) = q.q.as_deref().filter(|s| !s.trim().is_empty()) {
            let like = like_term(term);
            qb.push(" AND (b.no LIKE ")
                .push_bind(like.clone())
                .push(" OR lower(p.first_name_fa || ' ' || p.last_name_fa) LIKE ")
                .push_bind(like.clone())
                .push(" OR lower(p.first_name_en || ' ' || p.last_name_en) LIKE ")
                .push_bind(like)
                .push(")");
        }
    }

    let base = "FROM invoice_balance b JOIN patients p ON p.id = b.patient_id";
    let mut count_qb = QueryBuilder::<Sqlite>::new(format!("SELECT COUNT(*) {base}"));
    filters(&mut count_qb, &q);
    let total: i64 = count_qb.build_query_scalar().fetch_one(&state.pool).await?;

    let mut qb = QueryBuilder::<Sqlite>::new(format!(
        "SELECT b.*, (p.first_name_fa || ' ' || p.last_name_fa) AS name_fa,
                (p.first_name_en || ' ' || p.last_name_en) AS name_en, p.code AS patient_code,
                d.name_fa AS doctor_fa, d.name_en AS doctor_en {base}
         LEFT JOIN doctors d ON d.id = b.doctor_id"
    ));
    filters(&mut qb, &q);
    qb.push(" ORDER BY b.date DESC, b.no DESC LIMIT ")
        .push_bind(per)
        .push(" OFFSET ")
        .push_bind(offset);
    let rows = qb.build().fetch_all(&state.pool).await?;

    let items: Vec<Value> = rows
        .iter()
        .map(|r| {
            json!({
                "id": r.get::<String, _>("id"),
                "no": r.get::<String, _>("no"),
                "patientId": r.get::<String, _>("patient_id"),
                "date": r.get::<String, _>("date"),
                "status": r.get::<String, _>("status"),
                "subtotal": r.get::<i64, _>("subtotal"),
                "discount": r.get::<i64, _>("discount"),
                "insurance": r.get::<i64, _>("insurance"),
                "due": r.get::<i64, _>("due"),
                "paid": r.get::<i64, _>("paid"),
                "outstanding": r.get::<i64, _>("outstanding"),
                "patientName": txt(r.get::<String, _>("name_fa"), r.get::<String, _>("name_en")),
                "patientCode": r.get::<String, _>("patient_code"),
                "doctorName": txt(
                    r.try_get::<Option<String>, _>("doctor_fa").ok().flatten().unwrap_or_default(),
                    r.try_get::<Option<String>, _>("doctor_en").ok().flatten().unwrap_or_default(),
                ),
            })
        })
        .collect();
    Ok(Json(json!({ "items": items, "meta": paging(total, per, offset) })))
}

async fn balance(pool: &SqlitePool, id: &str) -> ApiResult<sqlx::sqlite::SqliteRow> {
    sqlx::query("SELECT * FROM invoice_balance WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::not_found("invoice"))
}

pub async fn detail(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    rbac::require(&user, rbac::INVOICE_VIEW)?;
    let head = sqlx::query(
        "SELECT i.*, (p.first_name_fa || ' ' || p.last_name_fa) AS name_fa,
                (p.first_name_en || ' ' || p.last_name_en) AS name_en, p.code AS patient_code,
                p.phone, p.national_id, p.address_fa, p.address_en,
                d.name_fa AS doctor_fa, d.name_en AS doctor_en,
                ins.name_fa AS insurer_fa, ins.name_en AS insurer_en
         FROM invoices i
         JOIN patients p ON p.id = i.patient_id
         LEFT JOIN doctors d ON d.id = i.doctor_id
         LEFT JOIN insurers ins ON ins.id = i.insurer_id
         WHERE i.id = ?",
    )
    .bind(&id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::not_found("invoice"))?;

    let b = balance(&state.pool, &id).await?;
    let items = sqlx::query("SELECT * FROM invoice_items WHERE invoice_id = ? ORDER BY rowid")
        .bind(&id)
        .fetch_all(&state.pool)
        .await?;
    let payments = sqlx::query(
        "SELECT p.*, u.name_fa AS by_fa, u.name_en AS by_en FROM payments p
         LEFT JOIN users u ON u.id = p.by_user WHERE p.invoice_id = ? ORDER BY p.at",
    )
    .bind(&id)
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(json!({
        "id": id,
        "no": head.get::<String, _>("no"),
        "patientId": head.get::<String, _>("patient_id"),
        "encounterId": head.try_get::<Option<String>, _>("encounter_id").ok().flatten(),
        "doctorId": head.try_get::<Option<String>, _>("doctor_id").ok().flatten(),
        "date": head.get::<String, _>("date"),
        "status": b.get::<String, _>("status"),
        "discount": b.get::<i64, _>("discount"),
        "cover": head.get::<i64, _>("cover"),
        "insurerId": head.try_get::<Option<String>, _>("insurer_id").ok().flatten(),
        "insurerName": txt(
            head.try_get::<Option<String>, _>("insurer_fa").ok().flatten().unwrap_or_default(),
            head.try_get::<Option<String>, _>("insurer_en").ok().flatten().unwrap_or_default(),
        ),
        "createdAt": head.get::<String, _>("created_at"),
        "cancelledAt": head.try_get::<Option<String>, _>("cancelled_at").ok().flatten(),
        "cancelReason": head.get::<String, _>("cancel_reason"),
        "totals": {
            "subtotal": b.get::<i64, _>("subtotal"),
            "discount": b.get::<i64, _>("discount"),
            "insurance": b.get::<i64, _>("insurance"),
            "due": b.get::<i64, _>("due"),
            "paid": b.get::<i64, _>("paid"),
            "outstanding": b.get::<i64, _>("outstanding"),
        },
        "patient": {
            "id": head.get::<String, _>("patient_id"),
            "code": head.get::<String, _>("patient_code"),
            "name": txt(head.get::<String, _>("name_fa"), head.get::<String, _>("name_en")),
            "phone": head.get::<String, _>("phone"),
            "nationalId": head.get::<String, _>("national_id"),
            "address": txt(head.get::<String, _>("address_fa"), head.get::<String, _>("address_en")),
        },
        "doctorName": txt(
            head.try_get::<Option<String>, _>("doctor_fa").ok().flatten().unwrap_or_default(),
            head.try_get::<Option<String>, _>("doctor_en").ok().flatten().unwrap_or_default(),
        ),
        "items": items.iter().map(|r| json!({
            "id": r.get::<String, _>("id"),
            "serviceId": r.try_get::<Option<String>, _>("service_id").ok().flatten(),
            "desc": txt(r.get::<String, _>("desc_fa"), r.get::<String, _>("desc_en")),
            "qty": r.get::<i64, _>("qty"),
            "price": r.get::<i64, _>("price"),
            "total": r.get::<i64, _>("qty") * r.get::<i64, _>("price"),
        })).collect::<Vec<_>>(),
        "payments": payments.iter().map(|r| json!({
            "id": r.get::<String, _>("id"),
            "amount": r.get::<i64, _>("amount"),
            "method": r.get::<String, _>("method"),
            "kind": r.get::<String, _>("kind"),
            "ref": r.get::<String, _>("ref"),
            "at": r.get::<String, _>("at"),
            "by": txt(
                r.try_get::<Option<String>, _>("by_fa").ok().flatten().unwrap_or_default(),
                r.try_get::<Option<String>, _>("by_en").ok().flatten().unwrap_or_default(),
            ),
        })).collect::<Vec<_>>(),
    })))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemInput {
    #[serde(default)]
    pub service_id: Option<String>,
    #[serde(default)]
    pub desc_fa: String,
    #[serde(default)]
    pub desc_en: String,
    #[serde(default)]
    pub qty: Option<i64>,
    #[serde(default)]
    pub price: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InvoiceInput {
    pub patient_id: String,
    #[serde(default)]
    pub encounter_id: Option<String>,
    #[serde(default)]
    pub doctor_id: Option<String>,
    pub items: Vec<ItemInput>,
    #[serde(default)]
    pub discount: i64,
    #[serde(default)]
    pub cover: Option<i64>,
    #[serde(default)]
    pub insurer_id: Option<String>,
}

pub async fn create(
    State(state): State<AppState>,
    ip: ClientIp,
    user: AuthUser,
    Json(body): Json<InvoiceInput>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    rbac::require(&user, rbac::PAYMENT_PROCESS)?;
    super::patients::patient_brief(&state.pool, &body.patient_id).await?;
    if body.items.is_empty() {
        return Err(AppError::bad("invoice needs at least one line item"));
    }

    let mut prepared: Vec<(Option<String>, String, String, i64, i64)> = Vec::new();
    for item in &body.items {
        let qty = item.qty.unwrap_or(1);
        if !(1..=100).contains(&qty) {
            return Err(AppError::bad("quantity must be between 1 and 100"));
        }
        let (mut desc_fa, mut desc_en, mut price) =
            (item.desc_fa.trim().to_string(), item.desc_en.trim().to_string(), item.price.unwrap_or(0));
        if let Some(service_id) = item.service_id.as_deref().filter(|s| !s.is_empty()) {
            let svc = sqlx::query("SELECT name_fa, name_en, price FROM services WHERE id = ?")
                .bind(service_id)
                .fetch_optional(&state.pool)
                .await?
                .ok_or_else(|| AppError::bad("unknown service"))?;
            if desc_fa.is_empty() {
                desc_fa = svc.get("name_fa");
                desc_en = svc.get("name_en");
            }
            if price == 0 {
                price = svc.get("price");
            }
        }
        if desc_fa.is_empty() {
            return Err(AppError::bad("line item description is required"));
        }
        prepared.push((
            item.service_id.clone().filter(|s| !s.is_empty()),
            util::clean(&desc_fa, 160)?,
            util::clean(&desc_en, 160)?,
            qty,
            util::money(price)?,
        ));
    }

    let subtotal: i64 = prepared.iter().map(|p| p.3 * p.4).sum();
    if body.discount < 0 || body.discount > subtotal {
        return Err(AppError::bad("discount cannot exceed the subtotal"));
    }
    let (insurer_id, cover) = match body.insurer_id.clone().filter(|s| !s.is_empty()) {
        Some(id) => {
            let row = sqlx::query("SELECT cover FROM insurers WHERE id = ?")
                .bind(&id)
                .fetch_optional(&state.pool)
                .await?
                .ok_or_else(|| AppError::bad("unknown insurer"))?;
            (Some(id), body.cover.unwrap_or(row.get::<i64, _>("cover")))
        }
        None => (None, body.cover.unwrap_or(0)),
    };
    if !(0..=100).contains(&cover) {
        return Err(AppError::bad("insurance cover must be between 0 and 100"));
    }

    let id = uuid::Uuid::new_v4().to_string();
    let now = util::now();
    let date = util::today(&state.cfg.tz);
    let mut tx = state.pool.begin().await?;
    let no = db::next_sequence(&mut tx, "invoices", "no", "INV-", 1000).await?;
    sqlx::query(
        "INSERT INTO invoices (id, no, patient_id, encounter_id, doctor_id, date, status, discount,
            cover, insurer_id, created_by, created_at)
         VALUES (?,?,?,?,?,?, 'unpaid', ?,?,?,?,?)",
    )
    .bind(&id)
    .bind(&no)
    .bind(&body.patient_id)
    .bind(&body.encounter_id)
    .bind(&body.doctor_id)
    .bind(&date)
    .bind(body.discount)
    .bind(cover)
    .bind(&insurer_id)
    .bind(&user.id)
    .bind(&now)
    .execute(&mut *tx)
    .await?;
    for (service_id, desc_fa, desc_en, qty, price) in &prepared {
        sqlx::query(
            "INSERT INTO invoice_items (id, invoice_id, service_id, desc_fa, desc_en, qty, price)
             VALUES (?,?,?,?,?,?,?)",
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(&id)
        .bind(service_id)
        .bind(desc_fa)
        .bind(desc_en)
        .bind(qty)
        .bind(price)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;

    audit(&state.pool, Some(&user.id), &user.display_name(), &user.role, "invoice_create", "invoice", &id, &format!("{no} subtotal {subtotal}"), "success", &ip.0).await;
    Ok((StatusCode::CREATED, Json(json!({ "id": id, "no": no }))))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentInput {
    pub amount: i64,
    #[serde(default)]
    pub method: String,
    #[serde(default)]
    pub reference: String,
}

async fn recompute(pool: &SqlitePool, id: &str) -> ApiResult<String> {
    let b = balance(pool, id).await?;
    let status: String = b.get("status");
    if matches!(status.as_str(), "cancelled" | "refunded") {
        return Ok(status);
    }
    let due: i64 = b.get("due");
    let paid: i64 = b.get("paid");
    let new_status = if paid <= 0 && due > 0 {
        "unpaid"
    } else if paid >= due {
        "paid"
    } else {
        "partial"
    };
    if new_status != status {
        sqlx::query("UPDATE invoices SET status = ? WHERE id = ?")
            .bind(new_status)
            .bind(id)
            .execute(pool)
            .await?;
    }
    Ok(new_status.to_string())
}

pub async fn add_payment(
    State(state): State<AppState>,
    ip: ClientIp,
    user: AuthUser,
    Path(id): Path<String>,
    Json(body): Json<PaymentInput>,
) -> ApiResult<Json<Value>> {
    rbac::require(&user, rbac::PAYMENT_PROCESS)?;
    let b = balance(&state.pool, &id).await?;
    let status: String = b.get("status");
    if matches!(status.as_str(), "cancelled" | "refunded") {
        return Err(AppError::conflict("invoice is closed"));
    }
    let outstanding: i64 = b.get("outstanding");
    if body.amount <= 0 {
        return Err(AppError::bad("payment amount must be positive"));
    }
    if body.amount > outstanding {
        return Err(AppError::bad(format!(
            "payment exceeds the outstanding balance ({outstanding})"
        )));
    }
    let method = util::one_of(
        if body.method.is_empty() { "cash" } else { &body.method },
        &["cash", "card", "transfer", "insurance", "wallet"],
        "payment method",
    )?;

    let now = util::now();
    let payment_id = uuid::Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO payments (id, invoice_id, amount, method, ref, at, by_user, kind)
         VALUES (?,?,?,?,?,?,?, 'payment')",
    )
    .bind(&payment_id)
    .bind(&id)
    .bind(body.amount)
    .bind(&method)
    .bind(util::clean(&body.reference, 80)?)
    .bind(&now)
    .bind(&user.id)
    .execute(&state.pool)
    .await?;
    let status = recompute(&state.pool, &id).await?;

    audit(&state.pool, Some(&user.id), &user.display_name(), &user.role, "payment", "invoice", &id, &format!("{} {method}", body.amount), "success", &ip.0).await;
    Ok(Json(json!({ "ok": true, "status": status, "paymentId": payment_id })))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RefundInput {
    pub amount: i64,
    #[serde(default)]
    pub reason: String,
}

pub async fn refund(
    State(state): State<AppState>,
    ip: ClientIp,
    user: AuthUser,
    Path(id): Path<String>,
    Json(body): Json<RefundInput>,
) -> ApiResult<Json<Value>> {
    rbac::require(&user, rbac::REFUND_MANAGE)?;
    let b = balance(&state.pool, &id).await?;
    let paid: i64 = b.get("paid");
    if body.amount <= 0 {
        return Err(AppError::bad("refund amount must be positive"));
    }
    if body.amount > paid {
        return Err(AppError::bad(format!("refund exceeds paid amount ({paid})")));
    }
    let reason = util::require_nonempty(&body.reason, "refund reason")?;

    let payment_id = uuid::Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO payments (id, invoice_id, amount, method, ref, at, by_user, kind)
         VALUES (?,?,?, 'cash', ?,?,?, 'refund')",
    )
    .bind(&payment_id)
    .bind(&id)
    .bind(body.amount)
    .bind(util::clean(&reason, 200)?)
    .bind(util::now())
    .bind(&user.id)
    .execute(&state.pool)
    .await?;

    let b = balance(&state.pool, &id).await?;
    let status = if b.get::<i64, _>("paid") <= 0 && b.get::<i64, _>("outstanding") <= 0 {
        sqlx::query("UPDATE invoices SET status = 'refunded' WHERE id = ?")
            .bind(&id)
            .execute(&state.pool)
            .await?;
        "refunded".to_string()
    } else {
        recompute(&state.pool, &id).await?
    };

    audit(&state.pool, Some(&user.id), &user.display_name(), &user.role, "refund", "invoice", &id, &format!("{} {reason}", body.amount), "success", &ip.0).await;
    Ok(Json(json!({ "ok": true, "status": status })))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelInput {
    #[serde(default)]
    pub reason: String,
}

pub async fn cancel(
    State(state): State<AppState>,
    ip: ClientIp,
    user: AuthUser,
    Path(id): Path<String>,
    Json(body): Json<CancelInput>,
) -> ApiResult<Json<Value>> {
    rbac::require(&user, rbac::PAYMENT_PROCESS)?;
    let b = balance(&state.pool, &id).await?;
    if b.get::<String, _>("status") == "cancelled" {
        return Ok(Json(json!({ "ok": true })));
    }
    if b.get::<i64, _>("paid") != 0 {
        return Err(AppError::conflict("refund all payments before cancelling the invoice"));
    }
    sqlx::query("UPDATE invoices SET status = 'cancelled', cancelled_at = ?, cancel_reason = ? WHERE id = ?")
        .bind(util::now())
        .bind(util::clean(&body.reason, 300)?)
        .bind(&id)
        .execute(&state.pool)
        .await?;
    audit(&state.pool, Some(&user.id), &user.display_name(), &user.role, "invoice_cancel", "invoice", &id, &body.reason, "success", &ip.0).await;
    Ok(Json(json!({ "ok": true })))
}
