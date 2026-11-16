use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::{QueryBuilder, Row, Sqlite, SqlitePool};

use super::{like_term, paging, txt};
use crate::auth::{AuthUser, ClientIp};
use crate::db::audit;
use crate::error::{ApiResult, AppError};
use crate::rbac;
use crate::state::AppState;
use crate::util;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/appointments", get(list).post(create))
        .route("/api/appointments/slots", get(slots))
        .route("/api/appointments/{id}", get(detail).patch(update))
        .route("/api/appointments/{id}/status", post(change_status))
        .route("/api/calendar", get(calendar))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListQuery {
    pub date: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub doctor_id: Option<String>,
    pub status: Option<String>,
    pub patient_id: Option<String>,
    pub q: Option<String>,
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct AppointmentRow {
    pub id: String,
    pub patient_id: String,
    pub doctor_id: String,
    pub dept_id: Option<String>,
    pub date: String,
    pub time: String,
    pub duration: i64,
    #[serde(rename = "type")]
    pub appt_type: String,
    pub status: String,
    pub reason_fa: String,
    pub reason_en: String,
    pub source: String,
    pub notes: String,
    pub created_at: String,
    pub status_changed_at: String,
    pub patient_code: Option<String>,
    pub patient_name_fa: Option<String>,
    pub patient_name_en: Option<String>,
    pub patient_phone: Option<String>,
    pub patient_dob: Option<String>,
    pub doctor_name_fa: Option<String>,
    pub doctor_name_en: Option<String>,
    pub doctor_color: Option<String>,
    pub queue_id: Option<String>,
    pub queue_number: Option<i64>,
}

const SELECT_APPT: &str = "SELECT a.*, a.type AS appt_type,
    p.code AS patient_code, p.first_name_fa AS p_first_fa, p.first_name_en AS p_first_en,
    p.last_name_fa AS p_last_fa, p.last_name_en AS p_last_en, p.phone AS patient_phone, p.dob AS patient_dob,
    (p.first_name_fa || ' ' || p.last_name_fa) AS patient_name_fa,
    (p.first_name_en || ' ' || p.last_name_en) AS patient_name_en,
    d.name_fa AS doctor_name_fa, d.name_en AS doctor_name_en, d.color AS doctor_color,
    (SELECT q.id FROM queue q WHERE q.appointment_id = a.id AND q.status <> 'cancelled' ORDER BY q.checked_in_at DESC LIMIT 1) AS queue_id,
    (SELECT q.number FROM queue q WHERE q.appointment_id = a.id AND q.status <> 'cancelled' ORDER BY q.checked_in_at DESC LIMIT 1) AS queue_number
  FROM appointments a
  JOIN patients p ON p.id = a.patient_id
  JOIN doctors d ON d.id = a.doctor_id";

fn filters(qb: &mut QueryBuilder<'_, Sqlite>, q: &ListQuery) {
    qb.push(" WHERE 1 = 1");
    if let Some(date) = q.date.as_deref().filter(|s| !s.is_empty()) {
        qb.push(" AND a.date = ").push_bind(date.to_string());
    }
    if let Some(from) = q.from.as_deref().filter(|s| !s.is_empty()) {
        qb.push(" AND a.date >= ").push_bind(from.to_string());
    }
    if let Some(to) = q.to.as_deref().filter(|s| !s.is_empty()) {
        qb.push(" AND a.date <= ").push_bind(to.to_string());
    }
    if let Some(doc) = q.doctor_id.as_deref().filter(|s| !s.is_empty()) {
        qb.push(" AND a.doctor_id = ").push_bind(doc.to_string());
    }
    if let Some(status) = q.status.as_deref().filter(|s| !s.is_empty()) {
        qb.push(" AND a.status = ").push_bind(status.to_string());
    }
    if let Some(pid) = q.patient_id.as_deref().filter(|s| !s.is_empty()) {
        qb.push(" AND a.patient_id = ").push_bind(pid.to_string());
    }
    if let Some(term) = q.q.as_deref().filter(|s| !s.trim().is_empty()) {
        let like = like_term(term);
        qb.push(" AND (lower(p.first_name_fa || ' ' || p.last_name_fa) LIKE ")
            .push_bind(like.clone())
            .push(" OR lower(p.first_name_en || ' ' || p.last_name_en) LIKE ")
            .push_bind(like.clone())
            .push(" OR p.code LIKE ")
            .push_bind(like.clone())
            .push(" OR p.phone LIKE ")
            .push_bind(like)
            .push(")");
    }
}

pub async fn list(
    State(state): State<AppState>,
    user: AuthUser,
    Query(q): Query<ListQuery>,
) -> ApiResult<Json<Value>> {
    rbac::require(&user, rbac::PATIENT_SEARCH)?;
    let (per, offset) = util::paginate(q.page, q.per_page.or(Some(200)));

    let mut count_qb = QueryBuilder::<Sqlite>::new(
        "SELECT COUNT(*) FROM appointments a JOIN patients p ON p.id = a.patient_id",
    );
    filters(&mut count_qb, &q);
    let total: i64 = count_qb.build_query_scalar().fetch_one(&state.pool).await?;

    let mut qb = QueryBuilder::<Sqlite>::new(SELECT_APPT);
    filters(&mut qb, &q);
    qb.push(" ORDER BY a.date, a.time LIMIT ")
        .push_bind(if total > 500 { 500 } else { per })
        .push(" OFFSET ")
        .push_bind(offset);
    let rows: Vec<AppointmentRow> = qb.build_query_as().fetch_all(&state.pool).await?;
    Ok(Json(json!({ "items": rows, "meta": paging(total, per, offset) })))
}

pub async fn detail(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    rbac::require(&user, rbac::PATIENT_SEARCH)?;
    let row: AppointmentRow = sqlx::query_as(&format!("{SELECT_APPT} WHERE a.id = ?"))
        .bind(&id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| AppError::not_found("appointment"))?;
    Ok(Json(json!(row)))
}

pub struct DoctorSchedule {
    pub work_start: String,
    pub work_end: String,
    pub slot_min: i64,
    pub days_off: Vec<u32>,
    pub active: bool,
}

pub async fn doctor_schedule(pool: &SqlitePool, doctor_id: &str) -> ApiResult<DoctorSchedule> {
    let row = sqlx::query(
        "SELECT work_start, work_end, slot_min, days_off, active FROM doctors WHERE id = ?",
    )
    .bind(doctor_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::not_found("doctor"))?;
    let days_raw: String = row.get("days_off");
    Ok(DoctorSchedule {
        work_start: row.get("work_start"),
        work_end: row.get("work_end"),
        slot_min: row.get::<i64, _>("slot_min").max(5),
        days_off: serde_json::from_str(&days_raw).unwrap_or_default(),
        active: row.get::<i64, _>("active") == 1,
    })
}

/// Returns the conflicting appointment (if any) inside the caller's transaction/pool.
pub async fn find_conflict(
    pool: &SqlitePool,
    doctor_id: &str,
    date: &str,
    time: &str,
    duration: i64,
    exclude_id: Option<&str>,
) -> ApiResult<Option<Value>> {
    let rows = sqlx::query(
        "SELECT a.id, a.time, a.duration, a.patient_id,
                (p.first_name_fa || ' ' || p.last_name_fa) AS name_fa,
                (p.first_name_en || ' ' || p.last_name_en) AS name_en
         FROM appointments a JOIN patients p ON p.id = a.patient_id
         WHERE a.doctor_id = ? AND a.date = ?
           AND a.status NOT IN ('cancelled','no_show','rescheduled')
           AND (? IS NULL OR a.id <> ?)",
    )
    .bind(doctor_id)
    .bind(date)
    .bind(exclude_id)
    .bind(exclude_id)
    .fetch_all(pool)
    .await?;

    for row in rows {
        let other_time: String = row.get("time");
        let other_dur: i64 = row.get("duration");
        if util::overlaps(time, duration as i32, &other_time, other_dur as i32)? {
            return Ok(Some(json!({
                "id": row.get::<String, _>("id"),
                "time": other_time,
                "duration": other_dur,
                "patientName": txt(
                    row.try_get::<Option<String>, _>("name_fa").ok().flatten().unwrap_or_default(),
                    row.try_get::<Option<String>, _>("name_en").ok().flatten().unwrap_or_default(),
                ),
            })));
        }
    }
    Ok(None)
}

async fn validate_slot(
    pool: &SqlitePool,
    doctor_id: &str,
    date: &str,
    time: &str,
    duration: i64,
    exclude_id: Option<&str>,
) -> ApiResult<()> {
    util::validate_date(date)?;
    util::validate_time(time)?;
    if !(5..=240).contains(&duration) {
        return Err(AppError::bad("duration must be between 5 and 240 minutes"));
    }
    let schedule = doctor_schedule(pool, doctor_id).await?;
    if !schedule.active {
        return Err(AppError::bad("doctor is not accepting appointments"));
    }
    let dow = util::dow(date)?;
    if schedule.days_off.contains(&dow) {
        return Err(AppError::conflict("doctor is not scheduled to work on this day"));
    }
    let start = util::time_minutes(time)?;
    let work_start = util::time_minutes(&schedule.work_start)?;
    let work_end = util::time_minutes(&schedule.work_end)?;
    if start < work_start || start + duration as i32 > work_end {
        return Err(AppError::conflict(format!(
            "outside working hours ({}–{})",
            schedule.work_start, schedule.work_end
        )));
    }
    if let Some(conflict) = find_conflict(pool, doctor_id, date, time, duration, exclude_id).await? {
        return Err(AppError::Conflict(format!(
            "time slot conflicts with appointment at {} ({})",
            conflict["time"].as_str().unwrap_or(""),
            conflict["patientName"]["en"].as_str().unwrap_or("")
        )));
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppointmentInput {
    pub patient_id: String,
    pub doctor_id: String,
    pub date: String,
    pub time: String,
    #[serde(default)]
    pub duration: Option<i64>,
    #[serde(default, rename = "type")]
    pub appt_type: String,
    #[serde(default)]
    pub reason_fa: String,
    #[serde(default)]
    pub reason_en: String,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub notes: String,
}

async fn patient_ok(pool: &SqlitePool, id: &str) -> ApiResult<()> {
    let row = sqlx::query("SELECT merged_into FROM patients WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::not_found("patient"))?;
    if row.get::<Option<String>, _>("merged_into").is_some() {
        return Err(AppError::conflict("patient record was merged; use the surviving record"));
    }
    Ok(())
}

pub async fn create(
    State(state): State<AppState>,
    ip: ClientIp,
    user: AuthUser,
    Json(body): Json<AppointmentInput>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    rbac::require(&user, rbac::APPT_MANAGE)?;
    patient_ok(&state.pool, &body.patient_id).await?;

    let duration = body.duration.unwrap_or(30);
    let appt_type = util::one_of(
        if body.appt_type.is_empty() { "consultation" } else { &body.appt_type },
        &["consultation", "first_visit", "follow_up", "emergency", "procedure", "telehealth"],
        "appointment type",
    )?;
    let source = util::one_of(
        if body.source.is_empty() { "walk_in" } else { &body.source },
        &["walk_in", "phone", "portal", "referral", "emergency"],
        "source",
    )?;
    validate_slot(&state.pool, &body.doctor_id, &body.date, &body.time, duration, None).await?;

    let id = uuid::Uuid::new_v4().to_string();
    let now = util::now();
    let dept: Option<String> =
        sqlx::query_scalar("SELECT dept_id FROM doctors WHERE id = ?")
            .bind(&body.doctor_id)
            .fetch_one(&state.pool)
            .await?;

    sqlx::query(
        "INSERT INTO appointments (id, patient_id, doctor_id, dept_id, date, time, duration, type,
            status, reason_fa, reason_en, source, notes, created_by, created_at, status_changed_at)
         VALUES (?,?,?,?,?,?,?,?, 'requested', ?,?,?,?,?,?,?)",
    )
    .bind(&id)
    .bind(&body.patient_id)
    .bind(&body.doctor_id)
    .bind(&dept)
    .bind(&body.date)
    .bind(&body.time)
    .bind(duration)
    .bind(&appt_type)
    .bind(util::clean(&body.reason_fa, 300)?)
    .bind(util::clean(&body.reason_en, 300)?)
    .bind(&source)
    .bind(util::clean(&body.notes, 1000)?)
    .bind(&user.id)
    .bind(&now)
    .bind(&now)
    .execute(&state.pool)
    .await?;

    audit(
        &state.pool,
        Some(&user.id),
        &user.display_name(),
        &user.role,
        "appt_create",
        "appointment",
        &id,
        &format!("{} {} {}", body.date, body.time, appt_type),
        "success",
        &ip.0,
    )
    .await;

    Ok((StatusCode::CREATED, Json(json!({ "id": id }))))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppointmentPatch {
    #[serde(default)]
    pub date: Option<String>,
    #[serde(default)]
    pub time: Option<String>,
    #[serde(default)]
    pub duration: Option<i64>,
    #[serde(default)]
    pub doctor_id: Option<String>,
    #[serde(default)]
    pub reason_fa: Option<String>,
    #[serde(default)]
    pub reason_en: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
}

pub async fn update(
    State(state): State<AppState>,
    ip: ClientIp,
    user: AuthUser,
    Path(id): Path<String>,
    Json(body): Json<AppointmentPatch>,
) -> ApiResult<Json<Value>> {
    rbac::require(&user, rbac::APPT_MANAGE)?;
    let row = sqlx::query("SELECT doctor_id, date, time, duration, status FROM appointments WHERE id = ?")
        .bind(&id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| AppError::not_found("appointment"))?;
    let status: String = row.get("status");
    if matches!(status.as_str(), "completed" | "cancelled" | "no_show") {
        return Err(AppError::conflict("a closed appointment cannot be edited"));
    }
    let doctor_id = body.doctor_id.clone().unwrap_or_else(|| row.get("doctor_id"));
    let date = body.date.clone().unwrap_or_else(|| row.get("date"));
    let time = body.time.clone().unwrap_or_else(|| row.get("time"));
    let duration = body.duration.unwrap_or_else(|| row.get("duration"));
    validate_slot(&state.pool, &doctor_id, &date, &time, duration, Some(&id)).await?;

    let rescheduled = date != row.get::<String, _>("date") || time != row.get::<String, _>("time");
    let now = util::now();
    sqlx::query(
        "UPDATE appointments SET doctor_id = ?, date = ?, time = ?, duration = ?,
            reason_fa = COALESCE(?, reason_fa), reason_en = COALESCE(?, reason_en),
            notes = COALESCE(?, notes), status = ?, status_changed_at = ?
         WHERE id = ?",
    )
    .bind(&doctor_id)
    .bind(&date)
    .bind(&time)
    .bind(duration)
    .bind(body.reason_fa.as_deref().map(|s| s.trim().to_string()))
    .bind(body.reason_en.as_deref().map(|s| s.trim().to_string()))
    .bind(body.notes.as_deref().map(|s| s.trim().to_string()))
    .bind(if rescheduled && status == "confirmed" { "requested" } else { status.as_str() })
    .bind(&now)
    .bind(&id)
    .execute(&state.pool)
    .await?;

    audit(
        &state.pool,
        Some(&user.id),
        &user.display_name(),
        &user.role,
        if rescheduled { "appt_reschedule" } else { "appt_update" },
        "appointment",
        &id,
        &format!("{date} {time}"),
        "success",
        &ip.0,
    )
    .await;
    Ok(Json(json!({ "ok": true })))
}

pub fn transition_allowed(from: &str, to: &str) -> bool {
    let allowed: &[&str] = match from {
        "requested" => &["confirmed", "cancelled", "rescheduled"],
        "confirmed" => &["arrived", "checked_in", "cancelled", "no_show", "rescheduled"],
        "arrived" => &["checked_in", "waiting", "in_progress", "cancelled", "no_show"],
        "checked_in" => &["waiting", "in_progress", "cancelled", "no_show"],
        "waiting" => &["in_progress", "cancelled", "no_show"],
        "in_progress" => &["completed", "cancelled"],
        "rescheduled" => &["requested", "confirmed", "cancelled"],
        _ => &[],
    };
    allowed.contains(&to)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusInput {
    pub status: String,
    #[serde(default)]
    pub note: String,
}

pub async fn change_status(
    State(state): State<AppState>,
    ip: ClientIp,
    user: AuthUser,
    Path(id): Path<String>,
    Json(body): Json<StatusInput>,
) -> ApiResult<Json<Value>> {
    rbac::require(&user, rbac::APPT_MANAGE)?;
    let row = sqlx::query("SELECT status, date, time, patient_id, doctor_id FROM appointments WHERE id = ?")
        .bind(&id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| AppError::not_found("appointment"))?;
    let current: String = row.get("status");
    let target = util::one_of(
        &body.status,
        &[
            "requested", "confirmed", "arrived", "checked_in", "waiting", "in_progress",
            "completed", "cancelled", "no_show", "rescheduled",
        ],
        "status",
    )?;
    if current == target {
        return Ok(Json(json!({ "ok": true })));
    }
    if !transition_allowed(&current, &target) {
        return Err(AppError::conflict(format!(
            "cannot change status from {current} to {target}"
        )));
    }

    sqlx::query("UPDATE appointments SET status = ?, status_changed_at = ?, notes = CASE WHEN ? = '' THEN notes ELSE ? END WHERE id = ?")
        .bind(&target)
        .bind(util::now())
        .bind(body.note.trim())
        .bind(body.note.trim())
        .bind(&id)
        .execute(&state.pool)
        .await?;

    // Completing an appointment closes its open queue entry too.
    if target == "completed" {
        let _ = sqlx::query(
            "UPDATE queue SET status = 'done', done_at = COALESCE(done_at, ?)
             WHERE appointment_id = ? AND status NOT IN ('done','cancelled')",
        )
        .bind(util::now())
        .bind(&id)
        .execute(&state.pool)
        .await;
    }
    if matches!(target.as_str(), "cancelled" | "no_show") {
        let _ = sqlx::query(
            "UPDATE queue SET status = 'cancelled' WHERE appointment_id = ? AND status NOT IN ('done','cancelled')",
        )
        .bind(&id)
        .execute(&state.pool)
        .await;
    }

    audit(
        &state.pool,
        Some(&user.id),
        &user.display_name(),
        &user.role,
        "appt_status",
        "appointment",
        &id,
        &format!("{current} -> {target} {}", body.note.trim()),
        "success",
        &ip.0,
    )
    .await;
    Ok(Json(json!({ "ok": true, "status": target })))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SlotsQuery {
    pub doctor_id: String,
    pub date: String,
    #[serde(default)]
    pub exclude_id: Option<String>,
}

pub async fn slots(
    State(state): State<AppState>,
    user: AuthUser,
    Query(q): Query<SlotsQuery>,
) -> ApiResult<Json<Value>> {
    rbac::require(&user, rbac::PATIENT_SEARCH)?;
    util::validate_date(&q.date)?;
    let schedule = doctor_schedule(&state.pool, &q.doctor_id).await?;
    let dow = util::dow(&q.date)?;
    if schedule.days_off.contains(&dow) || !schedule.active {
        return Ok(Json(json!({ "slots": [], "closed": true })));
    }
    let booked = sqlx::query(
        "SELECT time, duration FROM appointments
         WHERE doctor_id = ? AND date = ? AND status NOT IN ('cancelled','no_show','rescheduled')
           AND (? IS NULL OR id <> ?)",
    )
    .bind(&q.doctor_id)
    .bind(&q.date)
    .bind(q.exclude_id.as_deref())
    .bind(q.exclude_id.as_deref())
    .fetch_all(&state.pool)
    .await?;

    let start = util::time_minutes(&schedule.work_start)?;
    let end = util::time_minutes(&schedule.work_end)?;
    let step = schedule.slot_min as i32;
    let mut out = Vec::new();
    let mut m = start;
    while m + step <= end {
        let time = util::minutes_time(m);
        let taken = booked.iter().any(|r| {
            let bt: String = r.get("time");
            let bd: i64 = r.get("duration");
            util::overlaps(&time, step, &bt, bd as i32).unwrap_or(false)
        });
        out.push(json!({ "time": time, "available": !taken }));
        m += step;
    }
    Ok(Json(json!({ "slots": out, "closed": false })))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CalendarQuery {
    pub from: Option<String>,
    pub to: Option<String>,
    pub doctor_id: Option<String>,
}

pub async fn calendar(
    State(state): State<AppState>,
    user: AuthUser,
    Query(q): Query<CalendarQuery>,
) -> ApiResult<Json<Value>> {
    rbac::require(&user, rbac::PATIENT_SEARCH)?;
    let from = q.from.unwrap_or_else(|| util::today(&state.cfg.tz));
    let to = q.to.unwrap_or_else(|| util::add_days(&from, 41).unwrap_or_else(|_| from.clone()));
    util::validate_date(&from)?;
    util::validate_date(&to)?;
    if to < from {
        return Err(AppError::bad("invalid date range"));
    }

    let mut qb = QueryBuilder::<Sqlite>::new(
        "SELECT a.date, a.status, COUNT(*) AS n FROM appointments a WHERE a.date BETWEEN ",
    );
    qb.push_bind(from.clone())
        .push(" AND ")
        .push_bind(to.clone());
    if let Some(doc) = q.doctor_id.as_deref().filter(|s| !s.is_empty()) {
        qb.push(" AND a.doctor_id = ").push_bind(doc.to_string());
    }
    qb.push(" GROUP BY a.date, a.status");
    let rows = qb.build().fetch_all(&state.pool).await?;

    let mut days = serde_json::Map::new();
    for r in rows {
        let date: String = r.get("date");
        let status: String = r.get("status");
        let n: i64 = r.get("n");
        let entry = days
            .entry(date)
            .or_insert_with(|| json!({ "date": "", "total": 0, "byStatus": {} }));
        entry["date"] = json!(r.get::<String, _>("date"));
        entry["total"] = json!(entry["total"].as_i64().unwrap_or(0) + n);
        entry["byStatus"][&status] = json!(n);
    }
    Ok(Json(json!({
        "from": from,
        "to": to,
        "days": days.values().cloned().collect::<Vec<_>>(),
    })))
}
