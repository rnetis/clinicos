use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::{get, patch, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::{QueryBuilder, Row, Sqlite, SqlitePool};

use super::txt;
use crate::auth::{AuthUser, ClientIp};
use crate::db::{self, audit};
use crate::error::{ApiResult, AppError};
use crate::rbac;
use crate::state::AppState;
use crate::util;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/encounters", get(list).post(start))
        .route("/api/encounters/{id}", get(detail).put(save_draft))
        .route("/api/encounters/{id}/sign", post(sign))
        .route("/api/encounters/{id}/amend", post(amend))
        .route("/api/encounters/{id}/entered-in-error", post(entered_in_error))
        .route("/api/followups", get(list_followups))
        .route("/api/followups/{id}", patch(update_followup))
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Txt {
    #[serde(default)]
    pub fa: String,
    #[serde(default)]
    pub en: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct Dx {
    #[serde(default)]
    pub code: String,
    #[serde(default, rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub note: Txt,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct Rx {
    #[serde(default)]
    pub med_id: String,
    #[serde(default)]
    pub drug: Txt,
    #[serde(default)]
    pub dose: String,
    #[serde(default)]
    pub form: String,
    #[serde(default)]
    pub route: String,
    #[serde(default)]
    pub freq: String,
    #[serde(default)]
    pub dur_days: i64,
    #[serde(default)]
    pub qty: i64,
    #[serde(default)]
    pub instructions: Txt,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct FollowUpPlan {
    #[serde(default)]
    pub due: String,
    #[serde(default)]
    pub reason: Txt,
    #[serde(default)]
    pub priority: String,
    #[serde(default)]
    pub tests_before: Txt,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct EncounterData {
    #[serde(default)]
    pub chief: Txt,
    #[serde(default)]
    pub hpi: Txt,
    #[serde(default)]
    pub exam: Txt,
    #[serde(default)]
    pub plan: Txt,
    #[serde(default)]
    pub vitals: Value,
    #[serde(default)]
    pub diagnoses: Vec<Dx>,
    #[serde(default)]
    pub prescriptions: Vec<Rx>,
    #[serde(default)]
    pub lab_tests: Vec<String>,
    #[serde(default)]
    pub lab_priority: String,
    #[serde(default)]
    pub follow_up: Option<FollowUpPlan>,
    #[serde(default)]
    pub referral: Txt,
    #[serde(default)]
    pub notes: String,
}

impl EncounterData {
    pub fn empty() -> Self {
        Self {
            lab_priority: "routine".into(),
            ..Default::default()
        }
    }
}

const FORMS: [&str; 8] = ["tablet", "capsule", "syrup", "injection", "inhaler", "cream", "drops", "suppository"];
const ROUTES: [&str; 6] = ["PO", "IM", "IV", "INH", "TOP", "SC"];
const FREQS: [&str; 8] = ["OD", "BD", "TDS", "QID", "HS", "PRN", "STAT", "WEEKLY"];

pub async fn validate_data(pool: &SqlitePool, data: &mut EncounterData) -> ApiResult<()> {
    fn cut(t: &mut Txt, max: usize) -> ApiResult<()> {
        t.fa = util::clean(&t.fa, max)?;
        t.en = util::clean(&t.en, max)?;
        Ok(())
    }
    cut(&mut data.chief, 600)?;
    cut(&mut data.hpi, 6000)?;
    cut(&mut data.exam, 6000)?;
    cut(&mut data.plan, 6000)?;
    cut(&mut data.referral, 1000)?;
    data.notes = util::clean(&data.notes, 2000)?;

    if let Some(v) = data.vitals.as_object() {
        for key in ["bp", "hr", "rr", "temp", "spo2", "height", "weight", "glucose"] {
            if !v.contains_key(key) {
                continue;
            }
        }
        if let Some(bp) = v.get("bp").and_then(|x| x.as_str()).filter(|s| !s.is_empty()) {
            let valid = bp.matches('/').count() == 1
                && bp.split('/').all(|p| p.trim().parse::<u16>().map(|n| (30..=300).contains(&n)).unwrap_or(false));
            if !valid {
                return Err(AppError::bad("blood pressure must look like 120/80"));
            }
        }
    } else {
        data.vitals = json!({});
    }

    if data.diagnoses.len() > 12 {
        return Err(AppError::bad("too many diagnoses"));
    }
    for dx in &mut data.diagnoses {
        dx.code = util::clean(&dx.code, 16)?;
        if dx.code.is_empty() {
            return Err(AppError::bad("diagnosis code is required"));
        }
        let exists: Option<String> = sqlx::query_scalar("SELECT code FROM icd10 WHERE code = ?")
            .bind(&dx.code)
            .fetch_optional(pool)
            .await?;
        if exists.is_none() {
            return Err(AppError::bad(format!("unknown ICD-10 code: {}", dx.code)));
        }
        dx.kind = util::one_of(
            if dx.kind.is_empty() { "primary" } else { &dx.kind },
            &["primary", "secondary"],
            "diagnosis type",
        )?;
        cut(&mut dx.note, 400)?;
    }

    if data.prescriptions.len() > 20 {
        return Err(AppError::bad("too many prescriptions"));
    }
    for rx in &mut data.prescriptions {
        let row = sqlx::query("SELECT name_fa, name_en FROM meds WHERE id = ?")
            .bind(&rx.med_id)
            .fetch_optional(pool)
            .await?
            .ok_or_else(|| AppError::bad("unknown medication"))?;
        rx.drug = Txt {
            fa: row.get("name_fa"),
            en: row.get("name_en"),
        };
        rx.dose = util::require_nonempty(&rx.dose, "dose")?;
        rx.dose = util::clean(&rx.dose, 60)?;
        rx.form = util::one_of(if rx.form.is_empty() { "tablet" } else { &rx.form }, &FORMS, "form")?;
        rx.route = util::one_of(if rx.route.is_empty() { "PO" } else { &rx.route }, &ROUTES, "route")?;
        rx.freq = util::one_of(if rx.freq.is_empty() { "OD" } else { &rx.freq }, &FREQS, "frequency")?;
        if !(1..=365).contains(&rx.dur_days) {
            return Err(AppError::bad("duration must be between 1 and 365 days"));
        }
        if !(1..=3650).contains(&rx.qty) {
            return Err(AppError::bad("quantity is out of range"));
        }
        cut(&mut rx.instructions, 300)?;
    }

    if data.lab_tests.len() > 40 {
        return Err(AppError::bad("too many lab tests"));
    }
    for id in &data.lab_tests {
        let exists: Option<String> = sqlx::query_scalar("SELECT id FROM lab_tests WHERE id = ?")
            .bind(id)
            .fetch_optional(pool)
            .await?;
        if exists.is_none() {
            return Err(AppError::bad("unknown lab test"));
        }
    }
    data.lab_priority = util::one_of(
        if data.lab_priority.is_empty() { "routine" } else { &data.lab_priority },
        &["routine", "urgent", "stat"],
        "lab priority",
    )?;

    if let Some(fu) = &mut data.follow_up {
        util::validate_date(&fu.due)?;
        fu.priority = util::one_of(
            if fu.priority.is_empty() { "routine" } else { &fu.priority },
            &["routine", "high", "urgent"],
            "follow-up priority",
        )?;
        if util::clean(&fu.reason.fa, 300)?.is_empty() {
            return Err(AppError::bad("follow-up reason is required"));
        }
        cut(&mut fu.reason, 300)?;
        cut(&mut fu.tests_before, 300)?;
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListQuery {
    pub patient_id: Option<String>,
    pub doctor_id: Option<String>,
    pub status: Option<String>,
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
    rbac::require(&user, rbac::PATIENT_VIEW_CLINICAL)?;
    let (per, offset) = util::paginate(q.page, q.per_page);

    fn filters(qb: &mut QueryBuilder<'_, Sqlite>, q: &ListQuery) {
        qb.push(" WHERE 1 = 1");
        if let Some(pid) = q.patient_id.as_deref().filter(|s| !s.is_empty()) {
            qb.push(" AND e.patient_id = ").push_bind(pid.to_string());
        }
        if let Some(doc) = q.doctor_id.as_deref().filter(|s| !s.is_empty()) {
            qb.push(" AND e.doctor_id = ").push_bind(doc.to_string());
        }
        if let Some(status) = q.status.as_deref().filter(|s| !s.is_empty()) {
            qb.push(" AND e.status = ").push_bind(status.to_string());
        }
        if let Some(from) = q.from.as_deref().filter(|s| !s.is_empty()) {
            qb.push(" AND e.date >= ").push_bind(from.to_string());
        }
        if let Some(to) = q.to.as_deref().filter(|s| !s.is_empty()) {
            qb.push(" AND e.date <= ").push_bind(to.to_string());
        }
    }

    let mut count_qb = QueryBuilder::<Sqlite>::new("SELECT COUNT(*) FROM encounters e");
    filters(&mut count_qb, &q);
    let total: i64 = count_qb.build_query_scalar().fetch_one(&state.pool).await?;

    let mut qb = QueryBuilder::<Sqlite>::new(
        "SELECT e.*, (p.first_name_fa || ' ' || p.last_name_fa) AS name_fa,
                (p.first_name_en || ' ' || p.last_name_en) AS name_en, p.code AS patient_code,
                d.name_fa AS doctor_fa, d.name_en AS doctor_en,
                (SELECT data FROM encounter_versions v WHERE v.encounter_id = e.id
                 ORDER BY v.version DESC LIMIT 1) AS data
         FROM encounters e
         JOIN patients p ON p.id = e.patient_id
         JOIN doctors d ON d.id = e.doctor_id",
    );
    filters(&mut qb, &q);
    qb.push(" ORDER BY e.date DESC, e.created_at DESC LIMIT ")
        .push_bind(per)
        .push(" OFFSET ")
        .push_bind(offset);
    let rows = qb.build().fetch_all(&state.pool).await?;

    let items: Vec<Value> = rows
        .iter()
        .map(|r| {
            let data: Value = r
                .try_get::<Option<String>, _>("data")
                .ok()
                .flatten()
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or(json!({}));
            let dx = data.get("diagnoses").and_then(|d| d.as_array()).cloned().unwrap_or_default();
            let primary = dx
                .iter()
                .find(|d| d["type"] == json!("primary"))
                .or_else(|| dx.first())
                .cloned()
                .unwrap_or(Value::Null);
            json!({
                "id": r.get::<String, _>("id"),
                "patientId": r.get::<String, _>("patient_id"),
                "doctorId": r.get::<String, _>("doctor_id"),
                "date": r.get::<String, _>("date"),
                "status": r.get::<String, _>("status"),
                "version": r.get::<i64, _>("version"),
                "signedAt": r.try_get::<Option<String>, _>("signed_at").ok().flatten(),
                "patientName": txt(r.get::<String, _>("name_fa"), r.get::<String, _>("name_en")),
                "patientCode": r.get::<String, _>("patient_code"),
                "doctorName": txt(r.get::<String, _>("doctor_fa"), r.get::<String, _>("doctor_en")),
                "chief": data.get("chief").cloned().unwrap_or(json!({"fa":"","en":""})),
                "primaryDiagnosis": primary,
                "prescriptionCount": data.get("prescriptions").and_then(|p| p.as_array()).map(|p| p.len()).unwrap_or(0),
            })
        })
        .collect();
    Ok(Json(json!({ "items": items, "meta": super::paging(total, per, offset) })))
}

async fn encounter_row(pool: &SqlitePool, id: &str) -> ApiResult<sqlx::sqlite::SqliteRow> {
    sqlx::query("SELECT * FROM encounters WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::not_found("encounter"))
}

fn ensure_author(pool_doctor: Option<&str>, user: &AuthUser, allow_admin: bool) -> ApiResult<()> {
    if allow_admin && user.role == "admin" {
        return Ok(());
    }
    let own = user.doctor_id.as_deref();
    if own.is_some() && own == pool_doctor {
        Ok(())
    } else {
        Err(AppError::Forbidden)
    }
}

pub async fn detail(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    rbac::require(&user, rbac::PATIENT_VIEW_CLINICAL)?;
    let row = encounter_row(&state.pool, &id).await?;
    let patient_id: String = row.get("patient_id");
    let doctor_id: String = row.get("doctor_id");

    let versions = sqlx::query(
        "SELECT v.version, v.at, v.reason, v.data, u.name_fa AS by_fa, u.name_en AS by_en
         FROM encounter_versions v LEFT JOIN users u ON u.id = v.by_user
         WHERE v.encounter_id = ? ORDER BY v.version DESC",
    )
    .bind(&id)
    .fetch_all(&state.pool)
    .await?;

    let latest = versions
        .first()
        .and_then(|r| r.try_get::<String, _>("data").ok())
        .and_then(|s| serde_json::from_str::<Value>(&s).ok())
        .unwrap_or(json!({}));

    let patient = super::patients::patient_brief(&state.pool, &patient_id).await?;
    let doctor = sqlx::query("SELECT name_fa, name_en, specialty_fa, specialty_en, license FROM doctors WHERE id = ?")
        .bind(&doctor_id)
        .fetch_optional(&state.pool)
        .await?;

    Ok(Json(json!({
        "id": id,
        "patientId": patient_id,
        "doctorId": doctor_id,
        "appointmentId": row.try_get::<Option<String>, _>("appointment_id").ok().flatten(),
        "queueEntryId": row.try_get::<Option<String>, _>("queue_entry_id").ok().flatten(),
        "date": row.get::<String, _>("date"),
        "status": row.get::<String, _>("status"),
        "version": row.get::<i64, _>("version"),
        "signedAt": row.try_get::<Option<String>, _>("signed_at").ok().flatten(),
        "updatedAt": row.get::<String, _>("updated_at"),
        "patient": patient,
        "doctor": doctor.map(|d| json!({
            "name": txt(d.get::<String, _>("name_fa"), d.get::<String, _>("name_en")),
            "specialty": txt(d.get::<String, _>("specialty_fa"), d.get::<String, _>("specialty_en")),
            "license": d.get::<String, _>("license"),
        })),
        "data": latest,
        "versions": versions.iter().map(|r| json!({
            "version": r.get::<i64, _>("version"),
            "at": r.get::<String, _>("at"),
            "reason": r.get::<String, _>("reason"),
            "by": txt(
                r.try_get::<Option<String>, _>("by_fa").ok().flatten().unwrap_or_default(),
                r.try_get::<Option<String>, _>("by_en").ok().flatten().unwrap_or_default(),
            ),
            "data": r.try_get::<String, _>("data").ok()
                .and_then(|s| serde_json::from_str::<Value>(&s).ok())
                .unwrap_or(json!({})),
        })).collect::<Vec<_>>(),
    })))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartInput {
    pub patient_id: String,
    #[serde(default)]
    pub doctor_id: Option<String>,
    #[serde(default)]
    pub appointment_id: Option<String>,
    #[serde(default)]
    pub queue_entry_id: Option<String>,
}

pub async fn start(
    State(state): State<AppState>,
    ip: ClientIp,
    user: AuthUser,
    Json(body): Json<StartInput>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    rbac::require(&user, rbac::CLINICAL_WRITE)?;
    let doctor_id = match body.doctor_id.clone().filter(|s| !s.is_empty()) {
        Some(id) => id,
        None => user
            .doctor_id
            .clone()
            .ok_or_else(|| AppError::bad("doctorId is required"))?,
    };
    if user.role == "doctor" && user.doctor_id.as_deref() != Some(doctor_id.as_str()) {
        return Err(AppError::Forbidden);
    }
    super::patients::patient_brief(&state.pool, &body.patient_id).await?;
    let date = util::today(&state.cfg.tz);

    if let Some(existing) = sqlx::query(
        "SELECT id FROM encounters WHERE patient_id = ? AND doctor_id = ? AND date = ? AND status = 'draft'
         ORDER BY created_at DESC LIMIT 1",
    )
    .bind(&body.patient_id)
    .bind(&doctor_id)
    .bind(&date)
    .fetch_optional(&state.pool)
    .await?
    {
        let id: String = existing.get("id");
        return Ok((StatusCode::OK, Json(json!({ "id": id, "existing": true }))));
    }

    let id = uuid::Uuid::new_v4().to_string();
    let now = util::now();
    let mut tx = state.pool.begin().await?;
    sqlx::query(
        "INSERT INTO encounters (id, patient_id, doctor_id, appointment_id, queue_entry_id, date,
            status, version, entered_by, created_at, updated_at)
         VALUES (?,?,?,?,?,?, 'draft', 0, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&body.patient_id)
    .bind(&doctor_id)
    .bind(&body.appointment_id)
    .bind(&body.queue_entry_id)
    .bind(&date)
    .bind(&user.id)
    .bind(&now)
    .bind(&now)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "INSERT INTO encounter_versions (id, encounter_id, version, at, by_user, reason, data)
         VALUES (?,?,0,?,?, '', ?)",
    )
    .bind(uuid::Uuid::new_v4().to_string())
    .bind(&id)
    .bind(&now)
    .bind(&user.id)
    .bind(serde_json::to_string(&EncounterData::empty()).unwrap_or_else(|_| "{}".into()))
    .execute(&mut *tx)
    .await?;
    if let Some(queue_id) = body.queue_entry_id.as_deref().filter(|s| !s.is_empty()) {
        sqlx::query("UPDATE queue SET status = 'in_progress', started_at = COALESCE(started_at, ?) WHERE id = ?")
            .bind(&now)
            .bind(queue_id)
            .execute(&mut *tx)
            .await?;
    }
    if let Some(appt_id) = body.appointment_id.as_deref().filter(|s| !s.is_empty()) {
        sqlx::query("UPDATE appointments SET status = 'in_progress', status_changed_at = ? WHERE id = ? AND status NOT IN ('completed','cancelled')")
            .bind(&now)
            .bind(appt_id)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;

    audit(&state.pool, Some(&user.id), &user.display_name(), &user.role, "enc_start", "encounter", &id, &body.patient_id, "success", &ip.0).await;
    Ok((StatusCode::CREATED, Json(json!({ "id": id, "existing": false }))))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveInput {
    pub base_version: Option<i64>,
    pub data: EncounterData,
}

pub async fn save_draft(
    State(state): State<AppState>,
    ip: ClientIp,
    user: AuthUser,
    Path(id): Path<String>,
    Json(body): Json<SaveInput>,
) -> ApiResult<Json<Value>> {
    rbac::require(&user, rbac::CLINICAL_WRITE)?;
    let row = encounter_row(&state.pool, &id).await?;
    let doctor_id: String = row.get("doctor_id");
    ensure_author(Some(&doctor_id), &user, true)?;
    let status: String = row.get("status");
    if status != "draft" {
        return Err(AppError::conflict("signed records are immutable; use amend"));
    }
    let version: i64 = row.get("version");
    if let Some(base) = body.base_version {
        if base != version {
            return Err(AppError::conflict("record changed on another device; reload before saving"));
        }
    }
    let mut data = body.data;
    validate_data(&state.pool, &mut data).await?;
    let serialized = serde_json::to_string(&data).map_err(|e| AppError::bad(e.to_string()))?;
    let now = util::now();

    let mut tx = state.pool.begin().await?;
    sqlx::query("UPDATE encounter_versions SET data = ?, at = ?, by_user = ? WHERE encounter_id = ? AND version = ?")
        .bind(&serialized)
        .bind(&now)
        .bind(&user.id)
        .bind(&id)
        .bind(version)
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE encounters SET updated_at = ? WHERE id = ?")
        .bind(&now)
        .bind(&id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;

    audit(&state.pool, Some(&user.id), &user.display_name(), &user.role, "enc_save", "encounter", &id, "", "success", &ip.0).await;
    Ok(Json(json!({ "ok": true, "version": version, "updatedAt": now })))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignInput {
    #[serde(default)]
    pub data: Option<EncounterData>,
}

pub async fn sign(
    State(state): State<AppState>,
    ip: ClientIp,
    user: AuthUser,
    Path(id): Path<String>,
    Json(body): Json<SignInput>,
) -> ApiResult<Json<Value>> {
    rbac::require(&user, rbac::ENCOUNTER_SIGN)?;
    let row = encounter_row(&state.pool, &id).await?;
    let doctor_id: String = row.get("doctor_id");
    ensure_author(Some(&doctor_id), &user, true)?;
    let status: String = row.get("status");
    if status != "draft" {
        return Err(AppError::conflict("encounter is not a draft"));
    }
    let patient_id: String = row.get("patient_id");
    let date: String = row.get("date");
    let queue_entry_id: Option<String> = row.try_get("queue_entry_id").ok().flatten();
    let appointment_id: Option<String> = row.try_get("appointment_id").ok().flatten();

    let mut data = match body.data {
        Some(d) => d,
        None => {
            let raw: Option<String> = sqlx::query_scalar(
                "SELECT data FROM encounter_versions WHERE encounter_id = ? ORDER BY version DESC LIMIT 1",
            )
            .bind(&id)
            .fetch_optional(&state.pool)
            .await?;
            raw.and_then(|s| serde_json::from_str(&s).ok())
                .ok_or_else(|| AppError::bad("encounter has no content"))?
        }
    };
    validate_data(&state.pool, &mut data).await?;
    if data.chief.fa.trim().is_empty() && data.chief.en.trim().is_empty() {
        return Err(AppError::bad("chief complaint is required before signing"));
    }
    if data.diagnoses.is_empty() {
        return Err(AppError::bad("at least one diagnosis is required before signing"));
    }
    let serialized = serde_json::to_string(&data).map_err(|e| AppError::bad(e.to_string()))?;
    let now = util::now();
    let mut tx = state.pool.begin().await?;

    sqlx::query(
        "UPDATE encounters SET status = 'signed', version = 1, signed_at = ?, signed_by = ?, updated_at = ? WHERE id = ?",
    )
    .bind(&now)
    .bind(&user.id)
    .bind(&now)
    .bind(&id)
    .execute(&mut *tx)
    .await?;
    sqlx::query("DELETE FROM encounter_versions WHERE encounter_id = ? AND version = 0")
        .bind(&id)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        "INSERT INTO encounter_versions (id, encounter_id, version, at, by_user, reason, data)
         VALUES (?,?,1,?,?, '', ?)",
    )
    .bind(uuid::Uuid::new_v4().to_string())
    .bind(&id)
    .bind(&now)
    .bind(&user.id)
    .bind(&serialized)
    .execute(&mut *tx)
    .await?;

    if let Some(queue_id) = &queue_entry_id {
        sqlx::query("UPDATE queue SET status = 'done', done_at = COALESCE(done_at, ?) WHERE id = ?")
            .bind(&now)
            .bind(queue_id)
            .execute(&mut *tx)
            .await?;
    }
    if let Some(appt_id) = &appointment_id {
        sqlx::query("UPDATE appointments SET status = 'completed', status_changed_at = ? WHERE id = ? AND status NOT IN ('cancelled','no_show')")
            .bind(&now)
            .bind(appt_id)
            .execute(&mut *tx)
            .await?;
    }

    let mut lab_order_id = None;
    if !data.lab_tests.is_empty() {
        let order_id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO lab_orders (id, patient_id, encounter_id, doctor_id, date, status, priority, created_at)
             VALUES (?,?,?,?,?, 'ordered', ?, ?)",
        )
        .bind(&order_id)
        .bind(&patient_id)
        .bind(&id)
        .bind(&doctor_id)
        .bind(&date)
        .bind(&data.lab_priority)
        .bind(&now)
        .execute(&mut *tx)
        .await?;
        for test_id in &data.lab_tests {
            sqlx::query("INSERT OR IGNORE INTO lab_order_tests (order_id, test_id) VALUES (?, ?)")
                .bind(&order_id)
                .bind(test_id)
                .execute(&mut *tx)
                .await?;
        }
        lab_order_id = Some(order_id);
    }

    if let Some(fu) = &data.follow_up {
        sqlx::query(
            "INSERT INTO follow_ups (id, patient_id, doctor_id, encounter_id, due, reason_fa, reason_en,
                priority, status, tests_before_fa, tests_before_en, created_at, updated_at)
             VALUES (?,?,?,?,?,?,?,?, 'pending', ?,?,?,?)",
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(&patient_id)
        .bind(&doctor_id)
        .bind(&id)
        .bind(&fu.due)
        .bind(&fu.reason.fa)
        .bind(&fu.reason.en)
        .bind(&fu.priority)
        .bind(&fu.tests_before.fa)
        .bind(&fu.tests_before.en)
        .bind(&now)
        .bind(&now)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;

    // Notifications happen after commit so a messaging failure can never roll back a signature.
    let chief = if data.chief.fa.is_empty() { data.chief.en.clone() } else { data.chief.fa.clone() };
    if let Some(order_id) = lab_order_id {
        db::notify(
            &state.pool,
            Some("lab"),
            None,
            "lab",
            data.lab_priority != "routine",
            &format!("درخواست آزمایش جدید پس از ویزیت: {chief}"),
            &format!("New laboratory order after visit: {chief}"),
            "#/lab",
        )
        .await;
        audit(&state.pool, Some(&user.id), &user.display_name(), &user.role, "lab_order", "lab_order", &order_id, &format!("{} tests", data.lab_tests.len()), "success", &ip.0).await;
    }
    audit(
        &state.pool,
        Some(&user.id),
        &user.display_name(),
        &user.role,
        "enc_sign",
        "encounter",
        &id,
        &data.diagnoses.iter().map(|d| d.code.clone()).collect::<Vec<_>>().join(","),
        "success",
        &ip.0,
    )
    .await;

    Ok(Json(json!({ "ok": true, "status": "signed", "version": 1 })))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AmendInput {
    pub reason: String,
    pub data: EncounterData,
}

pub async fn amend(
    State(state): State<AppState>,
    ip: ClientIp,
    user: AuthUser,
    Path(id): Path<String>,
    Json(body): Json<AmendInput>,
) -> ApiResult<Json<Value>> {
    rbac::require(&user, rbac::ENCOUNTER_SIGN)?;
    let row = encounter_row(&state.pool, &id).await?;
    let doctor_id: String = row.get("doctor_id");
    ensure_author(Some(&doctor_id), &user, true)?;
    let status: String = row.get("status");
    if !matches!(status.as_str(), "signed" | "amended") {
        return Err(AppError::conflict("only signed records can be amended"));
    }
    let reason = util::require_nonempty(&body.reason, "amendment reason")?;
    if reason.chars().count() < 5 {
        return Err(AppError::bad("amendment reason must be at least 5 characters"));
    }
    let mut data = body.data;
    validate_data(&state.pool, &mut data).await?;
    let serialized = serde_json::to_string(&data).map_err(|e| AppError::bad(e.to_string()))?;
    let now = util::now();
    let next: i64 = row.get::<i64, _>("version") + 1;

    let mut tx = state.pool.begin().await?;
    sqlx::query("UPDATE encounters SET status = 'amended', version = ?, updated_at = ? WHERE id = ?")
        .bind(next)
        .bind(&now)
        .bind(&id)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        "INSERT INTO encounter_versions (id, encounter_id, version, at, by_user, reason, data)
         VALUES (?,?,?,?,?,?,?)",
    )
    .bind(uuid::Uuid::new_v4().to_string())
    .bind(&id)
    .bind(next)
    .bind(&now)
    .bind(&user.id)
    .bind(&reason)
    .bind(&serialized)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;

    audit(&state.pool, Some(&user.id), &user.display_name(), &user.role, "enc_amend", "encounter", &id, &reason, "success", &ip.0).await;
    Ok(Json(json!({ "ok": true, "version": next })))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorInput {
    pub reason: String,
}

pub async fn entered_in_error(
    State(state): State<AppState>,
    ip: ClientIp,
    user: AuthUser,
    Path(id): Path<String>,
    Json(body): Json<ErrorInput>,
) -> ApiResult<Json<Value>> {
    rbac::require(&user, rbac::ENCOUNTER_SIGN)?;
    let row = encounter_row(&state.pool, &id).await?;
    let doctor_id: String = row.get("doctor_id");
    ensure_author(Some(&doctor_id), &user, true)?;
    let status: String = row.get("status");
    if status == "entered_in_error" {
        return Ok(Json(json!({ "ok": true })));
    }
    let reason = util::require_nonempty(&body.reason, "reason")?;
    let now = util::now();
    sqlx::query("UPDATE encounters SET status = 'entered_in_error', updated_at = ? WHERE id = ?")
        .bind(&now)
        .bind(&id)
        .execute(&state.pool)
        .await?;
    audit(&state.pool, Some(&user.id), &user.display_name(), &user.role, "enc_error", "encounter", &id, &reason, "success", &ip.0).await;
    Ok(Json(json!({ "ok": true })))
}


/* ------------------------------------------------------------ follow-ups */

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FollowUpQuery {
    pub status: Option<String>,
    pub doctor_id: Option<String>,
    pub patient_id: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub overdue: Option<bool>,
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

pub async fn list_followups(
    State(state): State<AppState>,
    user: AuthUser,
    Query(q): Query<FollowUpQuery>,
) -> ApiResult<Json<Value>> {
    rbac::require(&user, rbac::FU_MANAGE)
        .or_else(|_| rbac::require(&user, rbac::FU_STATUS))?;
    let (per, offset) = util::paginate(q.page, q.per_page);
    let today = util::today(&state.cfg.tz);
    // Doctors only see their own follow-up worklist unless they explicitly ask otherwise.
    let doctor_id = match user.role.as_str() {
        "doctor" => Some(user.doctor_id.clone().unwrap_or_default()),
        _ => q.doctor_id.clone(),
    };

    fn filters(
        qb: &mut QueryBuilder<'_, Sqlite>,
        q: &FollowUpQuery,
        doctor_id: &Option<String>,
        today: &str,
    ) {
        qb.push(" WHERE 1 = 1");
        if let Some(status) = q.status.as_deref().filter(|s| !s.is_empty()) {
            qb.push(" AND f.status = ").push_bind(status.to_string());
        }
        if let Some(doc) = doctor_id.as_deref().filter(|s| !s.is_empty()) {
            qb.push(" AND f.doctor_id = ").push_bind(doc.to_string());
        }
        if let Some(pid) = q.patient_id.as_deref().filter(|s| !s.is_empty()) {
            qb.push(" AND f.patient_id = ").push_bind(pid.to_string());
        }
        if let Some(from) = q.from.as_deref().filter(|s| !s.is_empty()) {
            qb.push(" AND f.due >= ").push_bind(from.to_string());
        }
        if let Some(to) = q.to.as_deref().filter(|s| !s.is_empty()) {
            qb.push(" AND f.due <= ").push_bind(to.to_string());
        }
        if q.overdue.unwrap_or(false) {
            qb.push(" AND f.status = 'pending' AND f.due < ")
                .push_bind(today.to_string());
        }
    }

    let mut count_qb = QueryBuilder::<Sqlite>::new("SELECT COUNT(*) FROM follow_ups f");
    filters(&mut count_qb, &q, &doctor_id, &today);
    let total: i64 = count_qb.build_query_scalar().fetch_one(&state.pool).await?;

    let mut qb = QueryBuilder::<Sqlite>::new(
        "SELECT f.*, (p.first_name_fa || ' ' || p.last_name_fa) AS name_fa,
                (p.first_name_en || ' ' || p.last_name_en) AS name_en, p.code AS patient_code, p.phone,
                d.name_fa AS doctor_fa, d.name_en AS doctor_en
         FROM follow_ups f JOIN patients p ON p.id = f.patient_id
         LEFT JOIN doctors d ON d.id = f.doctor_id",
    );
    filters(&mut qb, &q, &doctor_id, &today);
    qb.push(" ORDER BY CASE f.priority WHEN 'urgent' THEN 0 WHEN 'high' THEN 1 ELSE 2 END, f.due LIMIT ")
        .push_bind(per)
        .push(" OFFSET ")
        .push_bind(offset);
    let rows = qb.build().fetch_all(&state.pool).await?;

    let items: Vec<Value> = rows
        .iter()
        .map(|r| {
            let due: String = r.get("due");
            json!({
                "id": r.get::<String, _>("id"),
                "patientId": r.get::<String, _>("patient_id"),
                "doctorId": r.try_get::<Option<String>, _>("doctor_id").ok().flatten(),
                "encounterId": r.try_get::<Option<String>, _>("encounter_id").ok().flatten(),
                "due": due,
                "overdue": r.get::<String, _>("status") == "pending" && due.as_str() < today.as_str(),
                "reason": txt(r.get::<String, _>("reason_fa"), r.get::<String, _>("reason_en")),
                "priority": r.get::<String, _>("priority"),
                "status": r.get::<String, _>("status"),
                "testsBefore": txt(r.get::<String, _>("tests_before_fa"), r.get::<String, _>("tests_before_en")),
                "patientName": txt(r.get::<String, _>("name_fa"), r.get::<String, _>("name_en")),
                "patientCode": r.get::<String, _>("patient_code"),
                "phone": r.get::<String, _>("phone"),
                "doctorName": txt(
                    r.try_get::<Option<String>, _>("doctor_fa").ok().flatten().unwrap_or_default(),
                    r.try_get::<Option<String>, _>("doctor_en").ok().flatten().unwrap_or_default(),
                ),
                "createdAt": r.get::<String, _>("created_at"),
            })
        })
        .collect();
    Ok(Json(json!({ "items": items, "meta": super::paging(total, per, offset), "today": today })))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FollowUpPatch {
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub due: Option<String>,
    #[serde(default)]
    pub priority: Option<String>,
}

pub async fn update_followup(
    State(state): State<AppState>,
    ip: ClientIp,
    user: AuthUser,
    Path(id): Path<String>,
    Json(body): Json<FollowUpPatch>,
) -> ApiResult<Json<Value>> {
    rbac::require(&user, rbac::FU_MANAGE)
        .or_else(|_| rbac::require(&user, rbac::FU_STATUS))?;
    let row = sqlx::query("SELECT doctor_id, status FROM follow_ups WHERE id = ?")
        .bind(&id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| AppError::not_found("follow-up"))?;
    if user.role == "doctor" {
        let owner: Option<String> = row.try_get("doctor_id").ok().flatten();
        if owner.as_deref() != user.doctor_id.as_deref() {
            return Err(AppError::Forbidden);
        }
    }
    if user.role == "nurse" {
        // Nurses may only report contact outcomes, not rewrite clinical plans.
        if body.due.is_some() || body.priority.is_some() {
            return Err(AppError::Forbidden);
        }
        let allowed = ["contacted", "completed", "cancelled"];
        if !body.status.as_deref().map(|s| allowed.contains(&s)).unwrap_or(false) {
            return Err(AppError::Forbidden);
        }
    }
    if let Some(status) = body.status.as_deref() {
        util::one_of(
            status,
            &["pending", "scheduled", "contacted", "completed", "cancelled"],
            "status",
        )?;
    }
    if let Some(priority) = body.priority.as_deref() {
        util::one_of(priority, &["routine", "high", "urgent"], "priority")?;
    }
    if let Some(due) = body.due.as_deref() {
        util::validate_date(due)?;
    }
    let now = util::now();
    sqlx::query(
        "UPDATE follow_ups SET status = COALESCE(?, status), due = COALESCE(?, due),
            priority = COALESCE(?, priority), updated_at = ? WHERE id = ?",
    )
    .bind(body.status.clone())
    .bind(body.due.clone())
    .bind(body.priority.clone())
    .bind(&now)
    .bind(&id)
    .execute(&state.pool)
    .await?;
    audit(
        &state.pool,
        Some(&user.id),
        &user.display_name(),
        &user.role,
        "followup_update",
        "followup",
        &id,
        &format!(
            "{} {} {}",
            body.status.clone().unwrap_or_default(),
            body.due.clone().unwrap_or_default(),
            body.priority.clone().unwrap_or_default()
        ),
        "success",
        &ip.0,
    )
    .await;
    Ok(Json(json!({ "ok": true })))
}
