use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::Row;

use super::{paging, txt};
use crate::auth::{AuthUser, ClientIp};
use crate::db::audit;
use crate::error::{ApiResult, AppError};
use crate::rbac;
use crate::state::AppState;
use crate::util;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/queue", get(list).post(check_in))
        .route("/api/queue/{id}/status", post(change_status))
        .route("/api/queue/{id}/vitals", post(add_vitals))
        .route("/api/patients/{id}/vitals", post(add_patient_vitals))
        .route("/api/patients/{id}/nurse-notes", post(add_note))
        .route("/api/patients/{id}/procedures", post(add_procedure))
        .route("/api/patients/{id}/nursing", get(nursing_history))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListQuery {
    pub date: Option<String>,
    pub doctor_id: Option<String>,
    pub status: Option<String>,
}

pub async fn list(
    State(state): State<AppState>,
    user: AuthUser,
    Query(q): Query<ListQuery>,
) -> ApiResult<Json<Value>> {
    rbac::require(&user, rbac::QUEUE_VIEW)?;
    let date = q.date.unwrap_or_else(|| util::today(&state.cfg.tz));
    util::validate_date(&date)?;

    let mut sql = String::from(
        "SELECT q.*,
                (p.first_name_fa || ' ' || p.last_name_fa) AS name_fa,
                (p.first_name_en || ' ' || p.last_name_en) AS name_en,
                p.code AS patient_code, p.dob, p.gender, p.phone, p.blood_group,
                d.name_fa AS doctor_fa, d.name_en AS doctor_en, d.color AS doctor_color,
                a.time AS appt_time, a.status AS appt_status,
                (SELECT COUNT(*) FROM allergies al WHERE al.patient_id = p.id) AS allergy_count
         FROM queue q
         JOIN patients p ON p.id = q.patient_id
         JOIN doctors d ON d.id = q.doctor_id
         LEFT JOIN appointments a ON a.id = q.appointment_id
         WHERE q.date = ?",
    );
    if q.doctor_id.as_deref().filter(|s| !s.is_empty()).is_some() {
        sql.push_str(" AND q.doctor_id = ?");
    }
    if q.status.as_deref().filter(|s| !s.is_empty()).is_some() {
        sql.push_str(" AND q.status = ?");
    }
    sql.push_str(" ORDER BY q.priority DESC, q.number ASC");

    let mut query = sqlx::query(&sql).bind(&date);
    if let Some(doc) = q.doctor_id.as_deref().filter(|s| !s.is_empty()) {
        query = query.bind(doc);
    }
    if let Some(status) = q.status.as_deref().filter(|s| !s.is_empty()) {
        query = query.bind(status);
    }
    let rows = query.fetch_all(&state.pool).await?;

    let items: Vec<Value> = rows
        .iter()
        .map(|r| {
            json!({
                "id": r.get::<String, _>("id"),
                "patientId": r.get::<String, _>("patient_id"),
                "patientCode": r.get::<String, _>("patient_code"),
                "doctorId": r.get::<String, _>("doctor_id"),
                "appointmentId": r.try_get::<Option<String>, _>("appointment_id").ok().flatten(),
                "number": r.get::<i64, _>("number"),
                "date": r.get::<String, _>("date"),
                "priority": r.get::<i64, _>("priority") == 1,
                "status": r.get::<String, _>("status"),
                "checkedInAt": r.get::<String, _>("checked_in_at"),
                "calledAt": r.try_get::<Option<String>, _>("called_at").ok().flatten(),
                "startedAt": r.try_get::<Option<String>, _>("started_at").ok().flatten(),
                "doneAt": r.try_get::<Option<String>, _>("done_at").ok().flatten(),
                "patientName": txt(r.get::<String, _>("name_fa"), r.get::<String, _>("name_en")),
                "dob": r.get::<String, _>("dob"),
                "gender": r.get::<String, _>("gender"),
                "phone": r.get::<String, _>("phone"),
                "bloodGroup": r.get::<String, _>("blood_group"),
                "allergyCount": r.get::<i64, _>("allergy_count"),
                "doctorName": txt(r.get::<String, _>("doctor_fa"), r.get::<String, _>("doctor_en")),
                "doctorColor": r.get::<String, _>("doctor_color"),
                "appointmentTime": r.try_get::<Option<String>, _>("appt_time").ok().flatten(),
                "appointmentStatus": r.try_get::<Option<String>, _>("appt_status").ok().flatten(),
            })
        })
        .collect();
    Ok(Json(json!({ "items": items, "meta": paging(items.len() as i64, items.len().max(1) as i64, 0) })))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckInInput {
    #[serde(default)]
    pub appointment_id: Option<String>,
    #[serde(default)]
    pub patient_id: Option<String>,
    #[serde(default)]
    pub doctor_id: Option<String>,
    #[serde(default)]
    pub priority: bool,
}

pub async fn check_in(
    State(state): State<AppState>,
    ip: ClientIp,
    user: AuthUser,
    Json(body): Json<CheckInInput>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    rbac::require(&user, rbac::QUEUE_MANAGE)?;
    let date = util::today(&state.cfg.tz);

    let (patient_id, doctor_id, appointment_id) = if let Some(appt_id) = body.appointment_id.clone().filter(|s| !s.is_empty()) {
        let row = sqlx::query(
            "SELECT patient_id, doctor_id, date, status FROM appointments WHERE id = ?",
        )
        .bind(&appt_id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| AppError::not_found("appointment"))?;
        let appt_date: String = row.get("date");
        if appt_date != date {
            return Err(AppError::bad("only today's appointments can be checked in"));
        }
        let status: String = row.get("status");
        if matches!(status.as_str(), "cancelled" | "no_show" | "completed") {
            return Err(AppError::conflict("appointment is closed"));
        }
        let existing: Option<String> = sqlx::query_scalar(
            "SELECT id FROM queue WHERE appointment_id = ? AND status NOT IN ('done','cancelled')",
        )
        .bind(&appt_id)
        .fetch_optional(&state.pool)
        .await?;
        if let Some(existing) = existing {
            return Err(AppError::Conflict(format!(
                "patient is already in the queue (entry {existing})"
            )));
        }
        (
            row.get::<String, _>("patient_id"),
            row.get::<String, _>("doctor_id"),
            Some(appt_id),
        )
    } else {
        let patient_id = body
            .patient_id
            .clone()
            .filter(|s| !s.is_empty())
            .ok_or_else(|| AppError::bad("patientId or appointmentId is required"))?;
        let doctor_id = body
            .doctor_id
            .clone()
            .filter(|s| !s.is_empty())
            .ok_or_else(|| AppError::bad("doctorId is required for a walk-in"))?;
        (patient_id, doctor_id, None)
    };

    let number: i64 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(number), 0) + 1 FROM queue WHERE doctor_id = ? AND date = ?",
    )
    .bind(&doctor_id)
    .bind(&date)
    .fetch_one(&state.pool)
    .await?;

    let id = uuid::Uuid::new_v4().to_string();
    let now = util::now();
    sqlx::query(
        "INSERT INTO queue (id, patient_id, appointment_id, doctor_id, number, date, priority,
            status, checked_in_at, created_by)
         VALUES (?,?,?,?,?,?,?, 'waiting', ?, ?)",
    )
    .bind(&id)
    .bind(&patient_id)
    .bind(&appointment_id)
    .bind(&doctor_id)
    .bind(number)
    .bind(&date)
    .bind(body.priority as i32)
    .bind(&now)
    .bind(&user.id)
    .execute(&state.pool)
    .await?;

    if let Some(appt_id) = &appointment_id {
        let _ = sqlx::query(
            "UPDATE appointments SET status = 'checked_in', status_changed_at = ? WHERE id = ? AND status IN ('requested','confirmed','arrived')",
        )
        .bind(&now)
        .bind(appt_id)
        .execute(&state.pool)
        .await;
    }

    audit(
        &state.pool,
        Some(&user.id),
        &user.display_name(),
        &user.role,
        "checkin",
        "queue",
        &id,
        &format!("queue #{number}"),
        "success",
        &ip.0,
    )
    .await;

    // Let the assigned doctor know a patient is waiting.
    let doctor_user: Option<String> =
        sqlx::query_scalar("SELECT user_id FROM doctors WHERE id = ?")
            .bind(&doctor_id)
            .fetch_optional(&state.pool)
            .await?
            .flatten();
    let name: String = sqlx::query_scalar(
        "SELECT first_name_fa || ' ' || last_name_fa FROM patients WHERE id = ?",
    )
    .bind(&patient_id)
    .fetch_one(&state.pool)
    .await?;
    crate::db::notify(
        &state.pool,
        if doctor_user.is_none() { None } else { None },
        doctor_user.as_deref(),
        "queue",
        false,
        &format!("{name} در صف انتظار شماست (نوبت {number})"),
        &format!("{name} is waiting in your queue (#{number})"),
        "#/doctor/waiting",
    )
    .await;

    Ok((StatusCode::CREATED, Json(json!({ "id": id, "number": number }))))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusInput {
    pub status: String,
}

pub async fn change_status(
    State(state): State<AppState>,
    ip: ClientIp,
    user: AuthUser,
    Path(id): Path<String>,
    Json(body): Json<StatusInput>,
) -> ApiResult<Json<Value>> {
    rbac::require(&user, rbac::QUEUE_MANAGE)?;
    let row = sqlx::query("SELECT status, appointment_id FROM queue WHERE id = ?")
        .bind(&id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| AppError::not_found("queue entry"))?;
    let current: String = row.get("status");
    let target = util::one_of(&body.status, &["waiting", "called", "in_progress", "done", "cancelled"], "status")?;
    let allowed: &[&str] = match current.as_str() {
        "waiting" => &["called", "in_progress", "cancelled"],
        "called" => &["in_progress", "waiting", "cancelled"],
        "in_progress" => &["done", "cancelled"],
        _ => &[],
    };
    if !allowed.contains(&target.as_str()) {
        return Err(AppError::conflict(format!(
            "cannot change queue status from {current} to {target}"
        )));
    }
    let now = util::now();
    let column = match target.as_str() {
        "called" => "called_at",
        "in_progress" => "started_at",
        "done" => "done_at",
        _ => "",
    };
    let sql = if column.is_empty() {
        "UPDATE queue SET status = ? WHERE id = ?".to_string()
    } else {
        format!("UPDATE queue SET status = ?, {column} = COALESCE({column}, ?) WHERE id = ?")
    };
    let mut q = sqlx::query(&sql).bind(&target);
    if !column.is_empty() {
        q = q.bind(&now);
    }
    q.bind(&id).execute(&state.pool).await?;

    let appointment_id: Option<String> = row.try_get("appointment_id").ok().flatten();
    if let Some(appt_id) = appointment_id {
        let appt_status = match target.as_str() {
            "called" | "waiting" => Some("waiting"),
            "in_progress" => Some("in_progress"),
            "done" => Some("completed"),
            "cancelled" => Some("cancelled"),
            _ => None,
        };
        if let Some(status) = appt_status {
            let _ = sqlx::query(
                "UPDATE appointments SET status = ?, status_changed_at = ? WHERE id = ?",
            )
            .bind(status)
            .bind(&now)
            .bind(&appt_id)
            .execute(&state.pool)
            .await;
        }
    }

    audit(
        &state.pool,
        Some(&user.id),
        &user.display_name(),
        &user.role,
        "queue_status",
        "queue",
        &id,
        &format!("{current} -> {target}"),
        "success",
        &ip.0,
    )
    .await;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Debug, Deserialize)]
pub struct VitalsInput {
    #[serde(default)]
    pub bp: Option<String>,
    #[serde(default)]
    pub hr: Option<f64>,
    #[serde(default)]
    pub rr: Option<f64>,
    #[serde(default)]
    pub temp: Option<f64>,
    #[serde(default)]
    pub spo2: Option<f64>,
    #[serde(default)]
    pub height: Option<f64>,
    #[serde(default)]
    pub weight: Option<f64>,
    #[serde(default)]
    pub glucose: Option<f64>,
    #[serde(default)]
    pub note: String,
}

fn vitals_json(v: &VitalsInput) -> ApiResult<Value> {
    if let Some(bp) = v.bp.as_deref().filter(|s| !s.is_empty()) {
        let ok = bp
            .split('/')
            .all(|part| part.trim().parse::<u16>().map(|n| (30..=300).contains(&n)).unwrap_or(false))
            && bp.matches('/').count() == 1;
        if !ok {
            return Err(AppError::bad("blood pressure must look like 120/80"));
        }
    }
    let bounds = |val: Option<f64>, lo: f64, hi: f64, label: &str| -> ApiResult<()> {
        if let Some(x) = val {
            if !(x >= lo && x <= hi) {
                return Err(AppError::bad(format!("{label} out of range")));
            }
        }
        Ok(())
    };
    bounds(v.hr, 20.0, 300.0, "heart rate")?;
    bounds(v.rr, 4.0, 80.0, "respiratory rate")?;
    bounds(v.temp, 30.0, 45.0, "temperature")?;
    bounds(v.spo2, 40.0, 100.0, "oxygen saturation")?;
    bounds(v.height, 20.0, 260.0, "height")?;
    bounds(v.weight, 0.5, 500.0, "weight")?;
    bounds(v.glucose, 10.0, 900.0, "glucose")?;
    Ok(json!({
        "bp": v.bp.clone().unwrap_or_default(),
        "hr": v.hr, "rr": v.rr, "temp": v.temp, "spo2": v.spo2,
        "height": v.height, "weight": v.weight, "glucose": v.glucose,
        "note": v.note.trim(),
    }))
}

async fn insert_vitals(
    state: &AppState,
    user: &AuthUser,
    ip: &ClientIp,
    patient_id: &str,
    queue_id: Option<&str>,
    body: &VitalsInput,
) -> ApiResult<Json<Value>> {
    rbac::require(user, rbac::VITALS_WRITE)?;
    let data = vitals_json(body)?;
    let id = uuid::Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO vitals (id, patient_id, queue_entry_id, at, by_user, data) VALUES (?,?,?,?,?,?)")
        .bind(&id)
        .bind(patient_id)
        .bind(queue_id)
        .bind(util::now())
        .bind(&user.id)
        .bind(serde_json::to_string(&data).unwrap_or_else(|_| "{}".into()))
        .execute(&state.pool)
        .await?;
    audit(&state.pool, Some(&user.id), &user.display_name(), &user.role, "vitals", "patient", patient_id, "", "success", &ip.0).await;
    Ok(Json(json!({ "id": id, "data": data })))
}

pub async fn add_vitals(
    State(state): State<AppState>,
    ip: ClientIp,
    user: AuthUser,
    Path(id): Path<String>,
    Json(body): Json<VitalsInput>,
) -> ApiResult<Json<Value>> {
    rbac::require(&user, rbac::VITALS_WRITE)?;
    let row = sqlx::query("SELECT patient_id FROM queue WHERE id = ?")
        .bind(&id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| AppError::not_found("queue entry"))?;
    let patient_id: String = row.get("patient_id");
    insert_vitals(&state, &user, &ip, &patient_id, Some(&id), &body).await
}

pub async fn add_patient_vitals(
    State(state): State<AppState>,
    ip: ClientIp,
    user: AuthUser,
    Path(id): Path<String>,
    Json(body): Json<VitalsInput>,
) -> ApiResult<Json<Value>> {
    insert_vitals(&state, &user, &ip, &id, None, &body).await
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteInput {
    #[serde(default)]
    pub kind: String,
    pub text_fa: String,
    #[serde(default)]
    pub text_en: String,
}

pub async fn add_note(
    State(state): State<AppState>,
    ip: ClientIp,
    user: AuthUser,
    Path(id): Path<String>,
    Json(body): Json<NoteInput>,
) -> ApiResult<Json<Value>> {
    rbac::require(&user, rbac::NURSING_NOTES)?;
    let sub = uuid::Uuid::new_v4().to_string();
    let kind = util::one_of(
        if body.kind.is_empty() { "observation" } else { &body.kind },
        &["observation", "handover", "triage", "education"],
        "note kind",
    )?;
    sqlx::query(
        "INSERT INTO nurse_notes (id, patient_id, at, by_user, kind, text_fa, text_en) VALUES (?,?,?,?,?,?,?)",
    )
    .bind(&sub)
    .bind(&id)
    .bind(util::now())
    .bind(&user.id)
    .bind(&kind)
    .bind(util::require_nonempty(&body.text_fa, "note")?)
    .bind(util::clean(&body.text_en, 1000)?)
    .execute(&state.pool)
    .await?;
    audit(&state.pool, Some(&user.id), &user.display_name(), &user.role, "nurse_note", "patient", &id, &kind, "success", &ip.0).await;
    Ok(Json(json!({ "id": sub })))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcedureInput {
    #[serde(default)]
    pub service_id: Option<String>,
    pub note_fa: String,
    #[serde(default)]
    pub note_en: String,
}

pub async fn add_procedure(
    State(state): State<AppState>,
    ip: ClientIp,
    user: AuthUser,
    Path(id): Path<String>,
    Json(body): Json<ProcedureInput>,
) -> ApiResult<Json<Value>> {
    rbac::require(&user, rbac::NURSING_NOTES)?;
    let sub = uuid::Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO procedures (id, patient_id, service_id, at, by_user, note_fa, note_en) VALUES (?,?,?,?,?,?,?)",
    )
    .bind(&sub)
    .bind(&id)
    .bind(&body.service_id)
    .bind(util::now())
    .bind(&user.id)
    .bind(util::require_nonempty(&body.note_fa, "note")?)
    .bind(util::clean(&body.note_en, 500)?)
    .execute(&state.pool)
    .await?;
    audit(&state.pool, Some(&user.id), &user.display_name(), &user.role, "procedure", "patient", &id, "", "success", &ip.0).await;
    Ok(Json(json!({ "id": sub })))
}

pub async fn nursing_history(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    rbac::require(&user, rbac::PATIENT_VIEW_CLINICAL)?;
    let vitals = sqlx::query(
        "SELECT v.id, v.at, v.data, u.name_fa AS by_fa, u.name_en AS by_en
         FROM vitals v LEFT JOIN users u ON u.id = v.by_user
         WHERE v.patient_id = ? ORDER BY v.at DESC LIMIT 40",
    )
    .bind(&id)
    .fetch_all(&state.pool)
    .await?;
    let notes = sqlx::query(
        "SELECT n.id, n.at, n.kind, n.text_fa, n.text_en, u.name_fa AS by_fa, u.name_en AS by_en
         FROM nurse_notes n LEFT JOIN users u ON u.id = n.by_user
         WHERE n.patient_id = ? ORDER BY n.at DESC LIMIT 40",
    )
    .bind(&id)
    .fetch_all(&state.pool)
    .await?;
    let procedures = sqlx::query(
        "SELECT p.id, p.at, p.note_fa, p.note_en, s.name_fa AS svc_fa, s.name_en AS svc_en,
                u.name_fa AS by_fa, u.name_en AS by_en
         FROM procedures p LEFT JOIN services s ON s.id = p.service_id
         LEFT JOIN users u ON u.id = p.by_user
         WHERE p.patient_id = ? ORDER BY p.at DESC LIMIT 40",
    )
    .bind(&id)
    .fetch_all(&state.pool)
    .await?;

    let who = |r: &sqlx::sqlite::SqliteRow| {
        txt(
            r.try_get::<Option<String>, _>("by_fa").ok().flatten().unwrap_or_default(),
            r.try_get::<Option<String>, _>("by_en").ok().flatten().unwrap_or_default(),
        )
    };
    Ok(Json(json!({
        "vitals": vitals.iter().map(|r| {
            let data: String = r.get("data");
            json!({
                "id": r.get::<String, _>("id"),
                "at": r.get::<String, _>("at"),
                "by": who(r),
                "data": serde_json::from_str::<Value>(&data).unwrap_or(json!({})),
            })
        }).collect::<Vec<_>>(),
        "notes": notes.iter().map(|r| json!({
            "id": r.get::<String, _>("id"),
            "at": r.get::<String, _>("at"),
            "kind": r.get::<String, _>("kind"),
            "text": txt(r.get::<String, _>("text_fa"), r.get::<String, _>("text_en")),
            "by": who(r),
        })).collect::<Vec<_>>(),
        "procedures": procedures.iter().map(|r| json!({
            "id": r.get::<String, _>("id"),
            "at": r.get::<String, _>("at"),
            "note": txt(r.get::<String, _>("note_fa"), r.get::<String, _>("note_en")),
            "service": txt(
                r.try_get::<Option<String>, _>("svc_fa").ok().flatten().unwrap_or_default(),
                r.try_get::<Option<String>, _>("svc_en").ok().flatten().unwrap_or_default(),
            ),
            "by": who(r),
        })).collect::<Vec<_>>(),
    })))
}
