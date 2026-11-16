use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::{QueryBuilder, Row, Sqlite, SqlitePool};

use super::{paging, txt};
use crate::auth::{AuthUser, ClientIp};
use crate::db::{self, audit};
use crate::error::{ApiResult, AppError};
use crate::rbac;
use crate::state::AppState;
use crate::util;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/labs", get(list).post(order))
        .route("/api/labs/{id}", get(detail))
        .route("/api/labs/{id}/results", post(enter_results))
        .route("/api/labs/{id}/review", post(review))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListQuery {
    pub status: Option<String>,
    pub patient_id: Option<String>,
    pub doctor_id: Option<String>,
    pub priority: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

pub async fn list(
    State(state): State<AppState>,
    user: AuthUser,
    Query(q): Query<ListQuery>,
) -> ApiResult<Json<Value>> {
    rbac::require(&user, rbac::PATIENT_SEARCH)?;
    let (per, offset) = util::paginate(q.page, q.per_page);

    fn filters(qb: &mut QueryBuilder<'_, Sqlite>, q: &ListQuery) {
        qb.push(" WHERE 1 = 1");
        if let Some(status) = q.status.as_deref().filter(|s| !s.is_empty()) {
            qb.push(" AND l.status = ").push_bind(status.to_string());
        }
        if let Some(pid) = q.patient_id.as_deref().filter(|s| !s.is_empty()) {
            qb.push(" AND l.patient_id = ").push_bind(pid.to_string());
        }
        if let Some(doc) = q.doctor_id.as_deref().filter(|s| !s.is_empty()) {
            qb.push(" AND l.doctor_id = ").push_bind(doc.to_string());
        }
        if let Some(priority) = q.priority.as_deref().filter(|s| !s.is_empty()) {
            qb.push(" AND l.priority = ").push_bind(priority.to_string());
        }
        if let Some(from) = q.from.as_deref().filter(|s| !s.is_empty()) {
            qb.push(" AND l.date >= ").push_bind(from.to_string());
        }
        if let Some(to) = q.to.as_deref().filter(|s| !s.is_empty()) {
            qb.push(" AND l.date <= ").push_bind(to.to_string());
        }
    }

    let mut count_qb = QueryBuilder::<Sqlite>::new("SELECT COUNT(*) FROM lab_orders l");
    filters(&mut count_qb, &q);
    let total: i64 = count_qb.build_query_scalar().fetch_one(&state.pool).await?;

    let mut qb = QueryBuilder::<Sqlite>::new(
        "SELECT l.*, (p.first_name_fa || ' ' || p.last_name_fa) AS name_fa,
                (p.first_name_en || ' ' || p.last_name_en) AS name_en, p.code AS patient_code,
                p.dob, p.gender, d.name_fa AS doctor_fa, d.name_en AS doctor_en,
                (SELECT COUNT(*) FROM lab_order_tests t WHERE t.order_id = l.id) AS test_count,
                (SELECT COUNT(*) FROM lab_results r WHERE r.order_id = l.id) AS result_count,
                (SELECT COUNT(*) FROM lab_results r WHERE r.order_id = l.id AND r.flag = 'critical') AS critical_count
         FROM lab_orders l
         JOIN patients p ON p.id = l.patient_id
         JOIN doctors d ON d.id = l.doctor_id",
    );
    filters(&mut qb, &q);
    qb.push(" ORDER BY CASE l.priority WHEN 'stat' THEN 0 WHEN 'urgent' THEN 1 ELSE 2 END, l.date DESC, l.created_at DESC LIMIT ")
        .push_bind(per)
        .push(" OFFSET ")
        .push_bind(offset);
    let rows = qb.build().fetch_all(&state.pool).await?;

    let items: Vec<Value> = rows
        .iter()
        .map(|r| {
            json!({
                "id": r.get::<String, _>("id"),
                "patientId": r.get::<String, _>("patient_id"),
                "encounterId": r.try_get::<Option<String>, _>("encounter_id").ok().flatten(),
                "doctorId": r.get::<String, _>("doctor_id"),
                "date": r.get::<String, _>("date"),
                "status": r.get::<String, _>("status"),
                "priority": r.get::<String, _>("priority"),
                "patientName": txt(r.get::<String, _>("name_fa"), r.get::<String, _>("name_en")),
                "patientCode": r.get::<String, _>("patient_code"),
                "dob": r.get::<String, _>("dob"),
                "gender": r.get::<String, _>("gender"),
                "doctorName": txt(r.get::<String, _>("doctor_fa"), r.get::<String, _>("doctor_en")),
                "testCount": r.get::<i64, _>("test_count"),
                "resultCount": r.get::<i64, _>("result_count"),
                "criticalCount": r.get::<i64, _>("critical_count"),
            })
        })
        .collect();
    Ok(Json(json!({ "items": items, "meta": paging(total, per, offset) })))
}

async fn order_payload(pool: &SqlitePool, id: &str) -> ApiResult<Value> {
    let row = sqlx::query(
        "SELECT l.*, (p.first_name_fa || ' ' || p.last_name_fa) AS name_fa,
                (p.first_name_en || ' ' || p.last_name_en) AS name_en, p.code AS patient_code,
                p.dob, p.gender, p.phone, d.name_fa AS doctor_fa, d.name_en AS doctor_en
         FROM lab_orders l JOIN patients p ON p.id = l.patient_id
         JOIN doctors d ON d.id = l.doctor_id WHERE l.id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::not_found("lab order"))?;

    let tests = sqlx::query(
        "SELECT t.id, t.code, t.name_fa, t.name_en, t.unit, t.ref_m, t.ref_f,
                (SELECT r.value FROM lab_results r WHERE r.order_id = ot.order_id AND r.test_id = t.id ORDER BY r.at DESC LIMIT 1) AS value,
                (SELECT r.flag FROM lab_results r WHERE r.order_id = ot.order_id AND r.test_id = t.id ORDER BY r.at DESC LIMIT 1) AS flag,
                (SELECT r.at FROM lab_results r WHERE r.order_id = ot.order_id AND r.test_id = t.id ORDER BY r.at DESC LIMIT 1) AS resulted_at,
                (SELECT r.reviewed_at FROM lab_results r WHERE r.order_id = ot.order_id AND r.test_id = t.id ORDER BY r.at DESC LIMIT 1) AS reviewed_at
         FROM lab_order_tests ot JOIN lab_tests t ON t.id = ot.test_id
         WHERE ot.order_id = ? ORDER BY t.code",
    )
    .bind(id)
    .fetch_all(pool)
    .await?;

    Ok(json!({
        "id": id,
        "patientId": row.get::<String, _>("patient_id"),
        "encounterId": row.try_get::<Option<String>, _>("encounter_id").ok().flatten(),
        "doctorId": row.get::<String, _>("doctor_id"),
        "date": row.get::<String, _>("date"),
        "status": row.get::<String, _>("status"),
        "priority": row.get::<String, _>("priority"),
        "createdAt": row.get::<String, _>("created_at"),
        "patient": {
            "id": row.get::<String, _>("patient_id"),
            "code": row.get::<String, _>("patient_code"),
            "firstNameFa": row.get::<String, _>("name_fa"),
            "firstNameEn": row.get::<String, _>("name_en"),
            "dob": row.get::<String, _>("dob"),
            "gender": row.get::<String, _>("gender"),
            "phone": row.get::<String, _>("phone"),
        },
        "doctorName": txt(row.get::<String, _>("doctor_fa"), row.get::<String, _>("doctor_en")),
        "tests": tests.iter().map(|t| json!({
            "id": t.get::<String, _>("id"),
            "code": t.get::<String, _>("code"),
            "name": txt(t.get::<String, _>("name_fa"), t.get::<String, _>("name_en")),
            "unit": t.get::<String, _>("unit"),
            "refM": t.get::<String, _>("ref_m"),
            "refF": t.get::<String, _>("ref_f"),
            "value": t.try_get::<Option<String>, _>("value").ok().flatten(),
            "flag": t.try_get::<Option<String>, _>("flag").ok().flatten(),
            "resultedAt": t.try_get::<Option<String>, _>("resulted_at").ok().flatten(),
            "reviewedAt": t.try_get::<Option<String>, _>("reviewed_at").ok().flatten(),
        })).collect::<Vec<_>>(),
    }))
}

pub async fn detail(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    rbac::require(&user, rbac::PATIENT_SEARCH)?;
    Ok(Json(order_payload(&state.pool, &id).await?))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderInput {
    pub patient_id: String,
    #[serde(default)]
    pub doctor_id: Option<String>,
    #[serde(default)]
    pub encounter_id: Option<String>,
    pub test_ids: Vec<String>,
    #[serde(default)]
    pub priority: String,
}

pub async fn order(
    State(state): State<AppState>,
    ip: ClientIp,
    user: AuthUser,
    Json(body): Json<OrderInput>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    rbac::require(&user, rbac::LAB_ORDER)?;
    super::patients::patient_brief(&state.pool, &body.patient_id).await?;
    if body.test_ids.is_empty() {
        return Err(AppError::bad("select at least one test"));
    }
    let doctor_id = body
        .doctor_id
        .clone()
        .filter(|s| !s.is_empty())
        .or_else(|| user.doctor_id.clone())
        .ok_or_else(|| AppError::bad("doctorId is required"))?;
    let priority = util::one_of(
        if body.priority.is_empty() { "routine" } else { &body.priority },
        &["routine", "urgent", "stat"],
        "priority",
    )?;

    let id = uuid::Uuid::new_v4().to_string();
    let now = util::now();
    let date = util::today(&state.cfg.tz);
    let mut tx = state.pool.begin().await?;
    sqlx::query(
        "INSERT INTO lab_orders (id, patient_id, encounter_id, doctor_id, date, status, priority, created_at)
         VALUES (?,?,?,?,?, 'ordered', ?, ?)",
    )
    .bind(&id)
    .bind(&body.patient_id)
    .bind(&body.encounter_id)
    .bind(&doctor_id)
    .bind(&date)
    .bind(&priority)
    .bind(&now)
    .execute(&mut *tx)
    .await?;
    for test_id in &body.test_ids {
        let exists: Option<String> = sqlx::query_scalar("SELECT id FROM lab_tests WHERE id = ?")
            .bind(test_id)
            .fetch_optional(&mut *tx)
            .await?;
        if exists.is_none() {
            return Err(AppError::bad("unknown lab test"));
        }
        sqlx::query("INSERT OR IGNORE INTO lab_order_tests (order_id, test_id) VALUES (?, ?)")
            .bind(&id)
            .bind(test_id)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;

    db::notify(
        &state.pool,
        Some("lab"),
        None,
        "lab",
        priority != "routine",
        &format!("درخواست آزمایش {} جدید", priority),
        &format!("New {priority} laboratory order"),
        "#/lab",
    )
    .await;
    audit(&state.pool, Some(&user.id), &user.display_name(), &user.role, "lab_order", "lab_order", &id, &format!("{} tests", body.test_ids.len()), "success", &ip.0).await;
    Ok((StatusCode::CREATED, Json(json!({ "id": id }))))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResultInput {
    pub test_id: String,
    pub value: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResultsInput {
    pub results: Vec<ResultInput>,
}

pub async fn enter_results(
    State(state): State<AppState>,
    ip: ClientIp,
    user: AuthUser,
    Path(id): Path<String>,
    Json(body): Json<ResultsInput>,
) -> ApiResult<Json<Value>> {
    rbac::require(&user, rbac::LAB_RESULT)?;
    if body.results.is_empty() {
        return Err(AppError::bad("no results supplied"));
    }
    let order = sqlx::query("SELECT l.patient_id, l.doctor_id, l.status FROM lab_orders l WHERE l.id = ?")
        .bind(&id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| AppError::not_found("lab order"))?;
    let status: String = order.get("status");
    if matches!(status.as_str(), "reviewed" | "cancelled") {
        return Err(AppError::conflict("order is closed"));
    }
    let patient_id: String = order.get("patient_id");
    let doctor_id: String = order.get("doctor_id");
    let gender: String = sqlx::query_scalar("SELECT gender FROM patients WHERE id = ?")
        .bind(&patient_id)
        .fetch_one(&state.pool)
        .await
        .unwrap_or_default();

    let now = util::now();
    let mut critical: Vec<String> = Vec::new();
    let mut tx = state.pool.begin().await?;
    for item in &body.results {
        let test = sqlx::query("SELECT code, name_fa, name_en, unit, ref_m, ref_f FROM lab_tests WHERE id = ?")
            .bind(&item.test_id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(|| AppError::bad("unknown lab test"))?;
        let value = util::clean(&item.value, 60)?;
        if value.is_empty() {
            continue;
        }
        let ref_text: String = if gender == "M" {
            test.get("ref_m")
        } else {
            test.get("ref_f")
        };
        let flag = util::lab_flag(&value, &ref_text);
        if flag == "critical" {
            critical.push(format!("{} {}", test.get::<String, _>("code"), value));
        }
        // One current result per test; a re-run supersedes the earlier value.
        sqlx::query("DELETE FROM lab_results WHERE order_id = ? AND test_id = ?")
            .bind(&id)
            .bind(&item.test_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            "INSERT INTO lab_results (id, order_id, test_id, value, unit, ref, flag, at, entered_by)
             VALUES (?,?,?,?,?,?,?,?,?)",
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(&id)
        .bind(&item.test_id)
        .bind(&value)
        .bind(test.get::<String, _>("unit"))
        .bind(&ref_text)
        .bind(flag)
        .bind(&now)
        .bind(&user.id)
        .execute(&mut *tx)
        .await?;
    }

    let requested: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM lab_order_tests WHERE order_id = ?")
        .bind(&id)
        .fetch_one(&mut *tx)
        .await?;
    let resulted: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM lab_results WHERE order_id = ?")
        .bind(&id)
        .fetch_one(&mut *tx)
        .await?;
    let new_status = if resulted >= requested && requested > 0 { "completed" } else { "in_progress" };
    sqlx::query("UPDATE lab_orders SET status = ? WHERE id = ?")
        .bind(new_status)
        .bind(&id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;

    if !critical.is_empty() {
        let name: String = sqlx::query_scalar("SELECT first_name_fa || ' ' || last_name_fa FROM patients WHERE id = ?")
            .bind(&patient_id)
            .fetch_one(&state.pool)
            .await
            .unwrap_or_default();
        let doctor_user: Option<String> = sqlx::query_scalar("SELECT user_id FROM doctors WHERE id = ?")
            .bind(&doctor_id)
            .fetch_optional(&state.pool)
            .await?
            .flatten();
        db::notify(
            &state.pool,
            None,
            doctor_user.as_deref(),
            "lab",
            true,
            &format!("⚠ نتیجه بحرانی: {} برای {} — نیاز به بررسی فوری", critical.join(", "), name),
            &format!("⚠ Critical result: {} for {} — immediate review required", critical.join(", "), name),
            "#/doctor/labs",
        )
        .await;
    }

    audit(&state.pool, Some(&user.id), &user.display_name(), &user.role, "lab_result", "lab_order", &id, &format!("{} results", body.results.len()), "success", &ip.0).await;
    Ok(Json(json!({ "ok": true, "status": new_status, "critical": critical })))
}

pub async fn review(
    State(state): State<AppState>,
    ip: ClientIp,
    user: AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    rbac::require(&user, rbac::LAB_ACK)?;
    let row = sqlx::query("SELECT doctor_id, status FROM lab_orders WHERE id = ?")
        .bind(&id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| AppError::not_found("lab order"))?;
    if user.role != "admin" && user.doctor_id.as_deref() != Some(row.get::<String, _>("doctor_id").as_str()) {
        return Err(AppError::Forbidden);
    }
    let resulted: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM lab_results WHERE order_id = ?")
        .bind(&id)
        .fetch_one(&state.pool)
        .await?;
    if resulted == 0 {
        return Err(AppError::conflict("order has no results to review"));
    }
    let now = util::now();
    sqlx::query("UPDATE lab_orders SET status = 'reviewed' WHERE id = ?")
        .bind(&id)
        .execute(&state.pool)
        .await?;
    sqlx::query("UPDATE lab_results SET reviewed_by = ?, reviewed_at = ? WHERE order_id = ?")
        .bind(&user.id)
        .bind(&now)
        .bind(&id)
        .execute(&state.pool)
        .await?;
    audit(&state.pool, Some(&user.id), &user.display_name(), &user.role, "lab_review", "lab_order", &id, "", "success", &ip.0).await;
    Ok(Json(json!({ "ok": true })))
}
