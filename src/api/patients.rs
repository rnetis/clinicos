use axum::extract::{Path, Query, State};
use axum::{Json, Router};
use axum::routing::{delete, get, post};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::sqlite::SqliteRow;
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
        .route("/api/patients", get(list).post(create))
        .route("/api/patients/{id}", get(detail).patch(update))
        .route("/api/patients/{id}/allergies", post(add_allergy))
        .route("/api/patients/{id}/allergies/{sub}", delete(remove_allergy))
        .route("/api/patients/{id}/conditions", post(add_condition))
        .route("/api/patients/{id}/conditions/{sub}", delete(remove_condition))
        .route("/api/patients/{id}/meds", post(add_med))
        .route("/api/patients/{id}/meds/{sub}", delete(remove_med))
        .route("/api/patients/{id}/alerts", post(add_alert))
        .route("/api/patients/{id}/alerts/{sub}", delete(remove_alert))
        .route("/api/patients/{id}/merge", post(merge_patient))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListQuery {
    pub q: Option<String>,
    pub page: Option<i64>,
    pub per_page: Option<i64>,
    pub status: Option<String>,
    pub doctor_id: Option<String>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct PatientListRow {
    pub id: String,
    pub code: String,
    pub first_name_fa: String,
    pub first_name_en: String,
    pub last_name_fa: String,
    pub last_name_en: String,
    pub dob: String,
    pub gender: String,
    pub phone: String,
    pub national_id: String,
    pub policy_no: String,
    pub category: String,
    pub status: String,
    pub source: String,
    pub blood_group: String,
    pub assigned_doctor_id: Option<String>,
    pub created_at: String,
    pub doctor_name_fa: Option<String>,
    pub doctor_name_en: Option<String>,
    pub insurer_name_fa: Option<String>,
    pub insurer_name_en: Option<String>,
    pub insurer_cover: Option<i64>,
    pub allergy_count: i64,
    pub last_visit: Option<String>,
    pub outstanding: i64,
}

fn push_filters(qb: &mut QueryBuilder<'_, Sqlite>, q: &ListQuery) {
    qb.push(" WHERE p.merged_into IS NULL");
    if let Some(term) = q.q.as_deref().filter(|s| !s.trim().is_empty()) {
        let like = like_term(term);
        qb.push(
            " AND (lower(p.code) LIKE ",
        )
        .push_bind(like.clone())
        .push(" OR lower(p.first_name_fa || ' ' || p.last_name_fa) LIKE ")
        .push_bind(like.clone())
        .push(" OR lower(p.first_name_en || ' ' || p.last_name_en) LIKE ")
        .push_bind(like.clone())
        .push(" OR p.national_id LIKE ")
        .push_bind(like.clone())
        .push(" OR p.phone LIKE ")
        .push_bind(like.clone())
        .push(" OR lower(p.email) LIKE ")
        .push_bind(like.clone())
        .push(" OR lower(p.policy_no) LIKE ")
        .push_bind(like)
        .push(")");
    }
    if let Some(status) = q.status.as_deref().filter(|s| !s.is_empty()) {
        qb.push(" AND p.status = ").push_bind(status.to_string());
    }
    if let Some(doc) = q.doctor_id.as_deref().filter(|s| !s.is_empty()) {
        qb.push(" AND p.assigned_doctor_id = ").push_bind(doc.to_string());
    }
}

pub async fn list(
    State(state): State<AppState>,
    user: AuthUser,
    Query(q): Query<ListQuery>,
) -> ApiResult<Json<Value>> {
    rbac::require(&user, rbac::PATIENT_SEARCH)?;
    let (per, offset) = util::paginate(q.page, q.per_page);

    let mut count_qb = QueryBuilder::<Sqlite>::new("SELECT COUNT(*) AS n FROM patients p");
    push_filters(&mut count_qb, &q);
    let total: i64 = count_qb
        .build_query_scalar()
        .fetch_one(&state.pool)
        .await?;

    let mut qb = QueryBuilder::<Sqlite>::new(
        "SELECT p.id, p.code, p.first_name_fa, p.first_name_en, p.last_name_fa, p.last_name_en,
                p.dob, p.gender, p.phone, p.national_id, p.policy_no, p.category, p.status,
                p.source, p.blood_group, p.assigned_doctor_id, p.created_at,
                d.name_fa AS doctor_name_fa, d.name_en AS doctor_name_en,
                i.name_fa AS insurer_name_fa, i.name_en AS insurer_name_en, i.cover AS insurer_cover,
                (SELECT COUNT(*) FROM allergies a WHERE a.patient_id = p.id) AS allergy_count,
                (SELECT MAX(e.date) FROM encounters e WHERE e.patient_id = p.id) AS last_visit,
                COALESCE(pb.outstanding, 0) AS outstanding
         FROM patients p
         LEFT JOIN doctors d ON d.id = p.assigned_doctor_id
         LEFT JOIN insurers i ON i.id = p.insurance_provider_id
         LEFT JOIN patient_balance pb ON pb.patient_id = p.id",
    );
    push_filters(&mut qb, &q);
    qb.push(" ORDER BY p.created_at DESC, p.code DESC LIMIT ")
        .push_bind(per)
        .push(" OFFSET ")
        .push_bind(offset);

    let rows: Vec<PatientListRow> = qb.build_query_as().fetch_all(&state.pool).await?;
    Ok(Json(json!({ "items": rows, "meta": paging(total, per, offset) })))
}

async fn patient_exists(pool: &SqlitePool, id: &str) -> ApiResult<()> {
    let found: Option<String> = sqlx::query_scalar("SELECT id FROM patients WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    if found.is_some() {
        Ok(())
    } else {
        Err(AppError::not_found("patient"))
    }
}

#[derive(Debug, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
struct AllergyRow {
    id: String,
    tag: String,
    name_fa: String,
    name_en: String,
    severity: String,
    note: String,
    noted_at: String,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
struct ConditionRow {
    id: String,
    code: String,
    name_fa: String,
    name_en: String,
    chronic: i64,
    noted_at: String,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
struct MedRow {
    id: String,
    text_fa: String,
    text_en: String,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
struct AlertRow {
    id: String,
    text_fa: String,
    text_en: String,
    created_at: String,
}

pub async fn detail(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    rbac::require(&user, rbac::PATIENT_SEARCH)?;
    patient_exists(&state.pool, &id).await?;
    let clinical = rbac::can(&user.role, rbac::PATIENT_VIEW_CLINICAL);

    let row = sqlx::query(
        "SELECT p.*, d.name_fa AS doctor_name_fa, d.name_en AS doctor_name_en,
                d.specialty_fa AS doctor_specialty_fa, d.specialty_en AS doctor_specialty_en,
                i.name_fa AS insurer_name_fa, i.name_en AS insurer_name_en, i.cover AS insurer_cover,
                COALESCE(pb.outstanding, 0) AS outstanding
         FROM patients p
         LEFT JOIN doctors d ON d.id = p.assigned_doctor_id
         LEFT JOIN insurers i ON i.id = p.insurance_provider_id
         LEFT JOIN patient_balance pb ON pb.patient_id = p.id
         WHERE p.id = ?",
    )
    .bind(&id)
    .fetch_one(&state.pool)
    .await?;

    let get = |key: &str| -> String { row.try_get::<String, _>(key).unwrap_or_default() };
    let opt = |key: &str| -> Option<String> { row.try_get::<Option<String>, _>(key).ok().flatten() };
    let num = |key: &str| -> i64 { row.try_get::<i64, _>(key).unwrap_or(0) };

    let allergies: Vec<AllergyRow> = sqlx::query_as(
        "SELECT id, tag, name_fa, name_en, severity, note, noted_at FROM allergies
         WHERE patient_id = ? ORDER BY noted_at DESC",
    )
    .bind(&id)
    .fetch_all(&state.pool)
    .await?;
    let conditions: Vec<ConditionRow> = sqlx::query_as(
        "SELECT id, code, name_fa, name_en, chronic, noted_at FROM conditions
         WHERE patient_id = ? ORDER BY noted_at DESC",
    )
    .bind(&id)
    .fetch_all(&state.pool)
    .await?;
    let meds: Vec<MedRow> =
        sqlx::query_as("SELECT id, text_fa, text_en FROM patient_meds WHERE patient_id = ?")
            .bind(&id)
            .fetch_all(&state.pool)
            .await?;
    let alerts: Vec<AlertRow> = sqlx::query_as(
        "SELECT id, text_fa, text_en, created_at FROM patient_alerts WHERE patient_id = ? ORDER BY created_at DESC",
    )
    .bind(&id)
    .fetch_all(&state.pool)
    .await?;

    let counts = json!({
        "appointments": sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM appointments WHERE patient_id = ?").bind(&id).fetch_one(&state.pool).await?,
        "encounters": sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM encounters WHERE patient_id = ?").bind(&id).fetch_one(&state.pool).await?,
        "labs": sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM lab_orders WHERE patient_id = ?").bind(&id).fetch_one(&state.pool).await?,
        "invoices": sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM invoices WHERE patient_id = ?").bind(&id).fetch_one(&state.pool).await?,
        "followUps": sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM follow_ups WHERE patient_id = ? AND status IN ('pending','scheduled','contacted')").bind(&id).fetch_one(&state.pool).await?,
    });

    let mut payload = json!({
        "id": get("id"),
        "code": get("code"),
        "firstNameFa": get("first_name_fa"),
        "firstNameEn": get("first_name_en"),
        "lastNameFa": get("last_name_fa"),
        "lastNameEn": get("last_name_en"),
        "fatherName": get("father_name"),
        "dob": get("dob"),
        "gender": get("gender"),
        "nationalId": get("national_id"),
        "passport": get("passport"),
        "phone": get("phone"),
        "phone2": get("phone2"),
        "email": get("email"),
        "addressFa": get("address_fa"),
        "addressEn": get("address_en"),
        "cityFa": get("city_fa"),
        "cityEn": get("city_en"),
        "postal": get("postal"),
        "emergencyName": get("emergency_name"),
        "emergencyRel": get("emergency_rel"),
        "emergencyPhone": get("emergency_phone"),
        "insuranceProviderId": opt("insurance_provider_id"),
        "policyNo": get("policy_no"),
        "category": get("category"),
        "status": get("status"),
        "source": get("source"),
        "assignedDoctorId": opt("assigned_doctor_id"),
        "bloodGroup": get("blood_group"),
        "notes": get("notes"),
        "history": {
            "surgical": get("history_surgical"),
            "family": get("history_family"),
            "lifestyle": get("history_lifestyle"),
        },
        "mergedInto": opt("merged_into"),
        "createdAt": get("created_at"),
        "doctorNameFa": opt("doctor_name_fa"),
        "doctorNameEn": opt("doctor_name_en"),
        "doctorSpecialtyFa": opt("doctor_specialty_fa"),
        "doctorSpecialtyEn": opt("doctor_specialty_en"),
        "insurerNameFa": opt("insurer_name_fa"),
        "insurerNameEn": opt("insurer_name_en"),
        "insurerCover": num("insurer_cover"),
        "outstanding": num("outstanding"),
        "alerts": alerts,
        "counts": counts,
    });

    if clinical {
        payload["allergies"] = json!(allergies);
        payload["conditions"] = json!(conditions);
        payload["meds"] = json!(meds);
        payload["latestVitals"] = latest_vitals(&state.pool, &id).await?;
    } else {
        payload["allergies"] = json!([]);
        payload["conditions"] = json!([]);
        payload["meds"] = json!([]);
    }

    let appointments = sqlx::query(
        "SELECT a.id, a.date, a.time, a.duration, a.status, a.type, a.reason_fa, a.reason_en,
                d.name_fa AS doctor_fa, d.name_en AS doctor_en
         FROM appointments a LEFT JOIN doctors d ON d.id = a.doctor_id
         WHERE a.patient_id = ? ORDER BY a.date DESC, a.time DESC LIMIT 8",
    )
    .bind(&id)
    .fetch_all(&state.pool)
    .await?;
    payload["recentAppointments"] = json!(appointments
        .iter()
        .map(|r| json!({
            "id": r.get::<String, _>("id"),
            "date": r.get::<String, _>("date"),
            "time": r.get::<String, _>("time"),
            "duration": r.get::<i64, _>("duration"),
            "status": r.get::<String, _>("status"),
            "type": r.get::<String, _>("type"),
            "reason": txt(r.get::<String, _>("reason_fa"), r.get::<String, _>("reason_en")),
            "doctorName": txt(
                r.try_get::<Option<String>, _>("doctor_fa").ok().flatten().unwrap_or_default(),
                r.try_get::<Option<String>, _>("doctor_en").ok().flatten().unwrap_or_default(),
            ),
        }))
        .collect::<Vec<_>>());

    payload["invoices"] = invoices_for(&state.pool, &id).await?;
    payload["followUps"] = followups_for(&state.pool, &id).await?;

    if clinical {
        payload["encounters"] = encounters_for(&state.pool, &id).await?;
        payload["labOrders"] = labs_for(&state.pool, &id).await?;
    }

    Ok(Json(payload))
}

async fn latest_vitals(pool: &SqlitePool, patient_id: &str) -> ApiResult<Value> {
    let row = sqlx::query("SELECT data, at FROM vitals WHERE patient_id = ? ORDER BY at DESC LIMIT 1")
        .bind(patient_id)
        .fetch_optional(pool)
        .await?;
    Ok(match row {
        Some(r) => {
            let data: String = r.get("data");
            let at: String = r.get("at");
            json!({ "at": at, "data": serde_json::from_str::<Value>(&data).unwrap_or(json!({})) })
        }
        None => Value::Null,
    })
}

async fn invoices_for(pool: &SqlitePool, patient_id: &str) -> ApiResult<Value> {
    let rows = sqlx::query(
        "SELECT b.*, (SELECT COUNT(*) FROM invoice_items it WHERE it.invoice_id = b.id) AS items
         FROM invoice_balance b WHERE b.patient_id = ? ORDER BY b.date DESC, b.no DESC LIMIT 12",
    )
    .bind(patient_id)
    .fetch_all(pool)
    .await?;
    Ok(json!(rows
        .iter()
        .map(|r| json!({
            "id": r.get::<String, _>("id"),
            "no": r.get::<String, _>("no"),
            "date": r.get::<String, _>("date"),
            "status": r.get::<String, _>("status"),
            "subtotal": r.get::<i64, _>("subtotal"),
            "discount": r.get::<i64, _>("discount"),
            "insurance": r.get::<i64, _>("insurance"),
            "due": r.get::<i64, _>("due"),
            "paid": r.get::<i64, _>("paid"),
            "outstanding": r.get::<i64, _>("outstanding"),
            "items": r.get::<i64, _>("items"),
        }))
        .collect::<Vec<_>>()))
}

async fn followups_for(pool: &SqlitePool, patient_id: &str) -> ApiResult<Value> {
    let rows = sqlx::query(
        "SELECT f.id, f.due, f.reason_fa, f.reason_en, f.priority, f.status,
                f.tests_before_fa, f.tests_before_en, d.name_fa AS doctor_fa, d.name_en AS doctor_en
         FROM follow_ups f LEFT JOIN doctors d ON d.id = f.doctor_id
         WHERE f.patient_id = ? ORDER BY f.due DESC LIMIT 12",
    )
    .bind(patient_id)
    .fetch_all(pool)
    .await?;
    Ok(json!(rows
        .iter()
        .map(|r| json!({
            "id": r.get::<String, _>("id"),
            "due": r.get::<String, _>("due"),
            "reason": txt(r.get::<String, _>("reason_fa"), r.get::<String, _>("reason_en")),
            "priority": r.get::<String, _>("priority"),
            "status": r.get::<String, _>("status"),
            "testsBefore": txt(r.get::<String, _>("tests_before_fa"), r.get::<String, _>("tests_before_en")),
            "doctorName": txt(
                r.try_get::<Option<String>, _>("doctor_fa").ok().flatten().unwrap_or_default(),
                r.try_get::<Option<String>, _>("doctor_en").ok().flatten().unwrap_or_default(),
            ),
        }))
        .collect::<Vec<_>>()))
}

async fn encounters_for(pool: &SqlitePool, patient_id: &str) -> ApiResult<Value> {
    let rows = sqlx::query(
        "SELECT e.id, e.date, e.status, e.version, e.signed_at,
                d.name_fa AS doctor_fa, d.name_en AS doctor_en,
                (SELECT data FROM encounter_versions v WHERE v.encounter_id = e.id
                 ORDER BY v.version DESC LIMIT 1) AS data
         FROM encounters e LEFT JOIN doctors d ON d.id = e.doctor_id
         WHERE e.patient_id = ? ORDER BY e.date DESC, e.created_at DESC LIMIT 20",
    )
    .bind(patient_id)
    .fetch_all(pool)
    .await?;
    Ok(json!(rows
        .iter()
        .map(|r| {
            let data: Option<String> = r.try_get("data").ok();
            let data: Value = data
                .and_then(|d| serde_json::from_str(&d).ok())
                .unwrap_or(json!({}));
            json!({
                "id": r.get::<String, _>("id"),
                "date": r.get::<String, _>("date"),
                "status": r.get::<String, _>("status"),
                "version": r.get::<i64, _>("version"),
                "signedAt": r.try_get::<Option<String>, _>("signed_at").ok().flatten(),
                "doctorName": txt(
                    r.try_get::<Option<String>, _>("doctor_fa").ok().flatten().unwrap_or_default(),
                    r.try_get::<Option<String>, _>("doctor_en").ok().flatten().unwrap_or_default(),
                ),
                "chief": data.get("chief").cloned().unwrap_or(json!({"fa":"","en":""})),
                "diagnoses": data.get("diagnoses").cloned().unwrap_or(json!([])),
                "prescriptions": data.get("prescriptions").cloned().unwrap_or(json!([])),
            })
        })
        .collect::<Vec<_>>()))
}

async fn labs_for(pool: &SqlitePool, patient_id: &str) -> ApiResult<Value> {
    let rows = sqlx::query(
        "SELECT id, date, status, priority FROM lab_orders WHERE patient_id = ?
         ORDER BY date DESC LIMIT 12",
    )
    .bind(patient_id)
    .fetch_all(pool)
    .await?;
    let mut out = Vec::new();
    for r in &rows {
        let order_id: String = r.get("id");
        let results = test_results(pool, &order_id).await?;
        out.push(json!({
            "id": order_id,
            "date": r.get::<String, _>("date"),
            "status": r.get::<String, _>("status"),
            "priority": r.get::<String, _>("priority"),
            "results": results,
        }));
    }
    Ok(json!(out))
}

async fn test_results(pool: &SqlitePool, order_id: &str) -> ApiResult<Value> {
    let rows = sqlx::query(
        "SELECT r.test_id, t.code, t.name_fa, t.name_en, r.value, r.unit, r.ref, r.flag, r.at
         FROM lab_results r JOIN lab_tests t ON t.id = r.test_id
         WHERE r.order_id = ? ORDER BY t.code",
    )
    .bind(order_id)
    .fetch_all(pool)
    .await?;
    Ok(json!(rows
        .iter()
        .map(|r| json!({
            "testId": r.get::<String, _>("test_id"),
            "code": r.get::<String, _>("code"),
            "name": txt(r.get::<String, _>("name_fa"), r.get::<String, _>("name_en")),
            "value": r.get::<String, _>("value"),
            "unit": r.get::<String, _>("unit"),
            "ref": r.get::<String, _>("ref"),
            "flag": r.get::<String, _>("flag"),
            "at": r.get::<String, _>("at"),
        }))
        .collect::<Vec<_>>()))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatientInput {
    pub first_name_fa: String,
    #[serde(default)]
    pub first_name_en: String,
    pub last_name_fa: String,
    #[serde(default)]
    pub last_name_en: String,
    #[serde(default)]
    pub father_name: String,
    #[serde(default)]
    pub dob: String,
    #[serde(default)]
    pub gender: String,
    #[serde(default)]
    pub national_id: String,
    #[serde(default)]
    pub passport: String,
    #[serde(default)]
    pub phone: String,
    #[serde(default)]
    pub phone2: String,
    #[serde(default)]
    pub email: String,
    #[serde(default)]
    pub address_fa: String,
    #[serde(default)]
    pub address_en: String,
    #[serde(default)]
    pub city_fa: String,
    #[serde(default)]
    pub city_en: String,
    #[serde(default)]
    pub postal: String,
    #[serde(default)]
    pub emergency_name: String,
    #[serde(default)]
    pub emergency_rel: String,
    #[serde(default)]
    pub emergency_phone: String,
    #[serde(default)]
    pub insurance_provider_id: Option<String>,
    #[serde(default)]
    pub policy_no: String,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub assigned_doctor_id: Option<String>,
    #[serde(default)]
    pub blood_group: String,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub history_surgical: String,
    #[serde(default)]
    pub history_family: String,
    #[serde(default)]
    pub history_lifestyle: String,
    #[serde(default)]
    pub force: bool,
}

fn digits_only(s: &str) -> String {
    s.chars().filter(|c| c.is_ascii_digit()).collect()
}

impl PatientInput {
    fn validate(&self) -> ApiResult<()> {
        if util::clean(&self.first_name_fa, 80)?.is_empty() {
            return Err(AppError::bad("first name (Persian) is required"));
        }
        if util::clean(&self.last_name_fa, 80)?.is_empty() {
            return Err(AppError::bad("last name (Persian) is required"));
        }
        if !self.dob.is_empty() {
            util::validate_date(&self.dob)?;
            if self.dob > util::today(&chrono_tz::Asia::Tehran) {
                return Err(AppError::bad("date of birth cannot be in the future"));
            }
        }
        let phone = util::normalize_phone(&self.phone);
        util::validate_phone(&phone)?;
        util::validate_phone(&util::normalize_phone(&self.phone2))?;
        util::validate_phone(&util::normalize_phone(&self.emergency_phone))?;
        util::validate_email(&self.email)?;
        if !self.national_id.trim().is_empty() && digits_only(&self.national_id).len() != 10 {
            return Err(AppError::bad("national ID must be 10 digits"));
        }
        if let Some(g) = Some(self.gender.as_str()).filter(|g| !g.is_empty()) {
            util::one_of(g, &["M", "F"], "gender")?;
        }
        Ok(())
    }

    fn insurance(&self) -> ApiResult<Option<String>> {
        Ok(self.insurance_provider_id.clone().filter(|s| !s.is_empty()))
    }
}

pub async fn create(
    State(state): State<AppState>,
    ip: ClientIp,
    user: AuthUser,
    Json(body): Json<PatientInput>,
) -> ApiResult<(axum::http::StatusCode, Json<Value>)> {
    rbac::require(&user, rbac::PATIENT_REGISTER)?;
    body.validate()?;

    let duplicates = duplicates(&state.pool, &body).await?;
    let strong = duplicates
        .as_array()
        .map(|a| a.iter().any(|d| d["strong"] == json!(true)))
        .unwrap_or(false);
    if strong && !body.force {
        return Ok((
            axum::http::StatusCode::CONFLICT,
            Json(json!({
                "error": "duplicate",
                "message": "A matching patient already exists",
                "duplicates": duplicates,
            })),
        ));
    }

    let id = uuid::Uuid::new_v4().to_string();
    let now = util::now();
    let mut tx = state.pool.begin().await?;
    let code = db::next_sequence(&mut tx, "patients", "code", "P-", 1000).await?;
    let status = if body.status.is_empty() { "active" } else { &body.status };
    util::one_of(status, &["active", "inactive", "archived", "pending_verify"], "status")?;
    let category = if body.category.is_empty() { "general" } else { &body.category };
    let source = if body.source.is_empty() { "walk_in" } else { &body.source };

    sqlx::query(
        "INSERT INTO patients (
            id, code, first_name_fa, first_name_en, last_name_fa, last_name_en, father_name, dob,
            gender, national_id, passport, phone, phone2, email, address_fa, address_en, city_fa,
            city_en, postal, emergency_name, emergency_rel, emergency_phone, insurance_provider_id,
            policy_no, category, status, source, assigned_doctor_id, blood_group, notes,
            history_surgical, history_family, history_lifestyle, created_at, created_by
         ) VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)",
    )
    .bind(&id)
    .bind(&code)
    .bind(util::require_nonempty(&body.first_name_fa, "first name")?)
    .bind(util::clean(&body.first_name_en, 80)?)
    .bind(util::require_nonempty(&body.last_name_fa, "last name")?)
    .bind(util::clean(&body.last_name_en, 80)?)
    .bind(util::clean(&body.father_name, 80)?)
    .bind(util::clean(&body.dob, 10)?)
    .bind(util::clean(&body.gender, 1)?)
    .bind(digits_only(&body.national_id))
    .bind(util::clean(&body.passport, 32)?)
    .bind(util::normalize_phone(&body.phone))
    .bind(util::normalize_phone(&body.phone2))
    .bind(util::clean(&body.email, 254)?)
    .bind(util::clean(&body.address_fa, 400)?)
    .bind(util::clean(&body.address_en, 400)?)
    .bind(util::clean(&body.city_fa, 80)?)
    .bind(util::clean(&body.city_en, 80)?)
    .bind(util::clean(&body.postal, 16)?)
    .bind(util::clean(&body.emergency_name, 120)?)
    .bind(util::clean(&body.emergency_rel, 60)?)
    .bind(util::normalize_phone(&body.emergency_phone))
    .bind(body.insurance()?)
    .bind(util::clean(&body.policy_no, 64)?)
    .bind(category)
    .bind(status)
    .bind(source)
    .bind(&body.assigned_doctor_id)
    .bind(util::clean(&body.blood_group, 8)?)
    .bind(util::clean(&body.notes, 2000)?)
    .bind(util::clean(&body.history_surgical, 1000)?)
    .bind(util::clean(&body.history_family, 1000)?)
    .bind(util::clean(&body.history_lifestyle, 1000)?)
    .bind(&now)
    .bind(&user.id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;

    audit(
        &state.pool,
        Some(&user.id),
        &user.display_name(),
        &user.role,
        "patient_create",
        "patient",
        &id,
        &code,
        "success",
        &ip.0,
    )
    .await;

    Ok((
        axum::http::StatusCode::CREATED,
        Json(json!({ "id": id, "code": code })),
    ))
}

async fn duplicates(pool: &SqlitePool, body: &PatientInput) -> ApiResult<Value> {
    let nid = digits_only(&body.national_id);
    let phone = util::normalize_phone(&body.phone);
    let like = like_term(&format!("{} {}", body.first_name_fa, body.last_name_fa));
    let rows = sqlx::query(
        "SELECT p.id, p.code, p.first_name_fa, p.last_name_fa, p.first_name_en, p.last_name_en,
                p.dob, p.national_id, p.phone,
                (SELECT MAX(e.date) FROM encounters e WHERE e.patient_id = p.id) AS last_visit
         FROM patients p
         WHERE p.merged_into IS NULL
           AND ((? <> '' AND p.national_id = ?)
             OR (? <> '' AND p.phone = ?)
             OR lower(p.first_name_fa || ' ' || p.last_name_fa) LIKE ?
             OR lower(p.first_name_en || ' ' || p.last_name_en) LIKE ?)
         LIMIT 8",
    )
    .bind(&nid)
    .bind(&nid)
    .bind(&phone)
    .bind(&phone)
    .bind(&like)
    .bind(&like)
    .fetch_all(pool)
    .await?;

    let out = rows
        .iter()
        .map(|r| {
            let mut reasons: Vec<&str> = Vec::new();
            if !nid.is_empty() && r.get::<String, _>("national_id") == nid {
                reasons.push("national_id");
            }
            if !phone.is_empty() && r.get::<String, _>("phone") == phone {
                reasons.push("phone");
            }
            let name_fa = format!(
                "{} {}",
                r.get::<String, _>("first_name_fa"),
                r.get::<String, _>("last_name_fa")
            );
            if like_term(&name_fa) == like {
                reasons.push("name");
            }
            let dob: String = r.get("dob");
            if !dob.is_empty() && dob == body.dob && reasons.contains(&"name") {
                reasons.push("name+dob");
            }
            if reasons.is_empty() {
                reasons.push("name");
            }
            let strong = reasons
                .iter()
                .any(|x| matches!(*x, "national_id" | "phone" | "name+dob"));
            json!({
                "id": r.get::<String, _>("id"),
                "code": r.get::<String, _>("code"),
                "name": txt(r.get::<String, _>("first_name_fa"), r.get::<String, _>("first_name_en")),
                "lastName": txt(r.get::<String, _>("last_name_fa"), r.get::<String, _>("last_name_en")),
                "dob": dob,
                "reasons": reasons,
                "strong": strong,
                "lastVisit": r.try_get::<Option<String>, _>("last_visit").ok().flatten(),
            })
        })
        .collect::<Vec<_>>();
    Ok(json!(out))
}

pub async fn update(
    State(state): State<AppState>,
    ip: ClientIp,
    user: AuthUser,
    Path(id): Path<String>,
    Json(body): Json<PatientInput>,
) -> ApiResult<Json<Value>> {
    rbac::require(&user, rbac::PATIENT_EDIT_ADMIN)?;
    body.validate()?;
    patient_exists(&state.pool, &id).await?;

    let status = if body.status.is_empty() { "active" } else { &body.status };
    util::one_of(status, &["active", "inactive", "archived", "pending_verify"], "status")?;

    sqlx::query(
        "UPDATE patients SET
            first_name_fa = ?, first_name_en = ?, last_name_fa = ?, last_name_en = ?,
            father_name = ?, dob = ?, gender = ?, national_id = ?, passport = ?, phone = ?,
            phone2 = ?, email = ?, address_fa = ?, address_en = ?, city_fa = ?, city_en = ?,
            postal = ?, emergency_name = ?, emergency_rel = ?, emergency_phone = ?,
            insurance_provider_id = ?, policy_no = ?, category = ?, status = ?, source = ?,
            assigned_doctor_id = ?, blood_group = ?, notes = ?, history_surgical = ?,
            history_family = ?, history_lifestyle = ?
         WHERE id = ?",
    )
    .bind(util::require_nonempty(&body.first_name_fa, "first name")?)
    .bind(util::clean(&body.first_name_en, 80)?)
    .bind(util::require_nonempty(&body.last_name_fa, "last name")?)
    .bind(util::clean(&body.last_name_en, 80)?)
    .bind(util::clean(&body.father_name, 80)?)
    .bind(util::clean(&body.dob, 10)?)
    .bind(util::clean(&body.gender, 1)?)
    .bind(digits_only(&body.national_id))
    .bind(util::clean(&body.passport, 32)?)
    .bind(util::normalize_phone(&body.phone))
    .bind(util::normalize_phone(&body.phone2))
    .bind(util::clean(&body.email, 254)?)
    .bind(util::clean(&body.address_fa, 400)?)
    .bind(util::clean(&body.address_en, 400)?)
    .bind(util::clean(&body.city_fa, 80)?)
    .bind(util::clean(&body.city_en, 80)?)
    .bind(util::clean(&body.postal, 16)?)
    .bind(util::clean(&body.emergency_name, 120)?)
    .bind(util::clean(&body.emergency_rel, 60)?)
    .bind(util::normalize_phone(&body.emergency_phone))
    .bind(body.insurance()?)
    .bind(util::clean(&body.policy_no, 64)?)
    .bind(if body.category.is_empty() { "general".to_string() } else { util::clean(&body.category, 32)? })
    .bind(status)
    .bind(if body.source.is_empty() { "walk_in".to_string() } else { util::clean(&body.source, 32)? })
    .bind(&body.assigned_doctor_id)
    .bind(util::clean(&body.blood_group, 8)?)
    .bind(util::clean(&body.notes, 2000)?)
    .bind(util::clean(&body.history_surgical, 1000)?)
    .bind(util::clean(&body.history_family, 1000)?)
    .bind(util::clean(&body.history_lifestyle, 1000)?)
    .bind(&id)
    .execute(&state.pool)
    .await?;

    audit(
        &state.pool,
        Some(&user.id),
        &user.display_name(),
        &user.role,
        "patient_update",
        "patient",
        &id,
        "",
        "success",
        &ip.0,
    )
    .await;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AllergyInput {
    #[serde(default)]
    pub tag: String,
    pub name_fa: String,
    #[serde(default)]
    pub name_en: String,
    #[serde(default)]
    pub severity: String,
    #[serde(default)]
    pub note: String,
}

async fn add_allergy(
    State(state): State<AppState>,
    ip: ClientIp,
    user: AuthUser,
    Path(id): Path<String>,
    Json(body): Json<AllergyInput>,
) -> ApiResult<Json<Value>> {
    rbac::require(&user, rbac::PATIENT_VIEW_CLINICAL)?;
    rbac::require(&user, rbac::CLINICAL_WRITE)?;
    patient_exists(&state.pool, &id).await?;
    let sub = uuid::Uuid::new_v4().to_string();
    let severity = if body.severity.is_empty() { "moderate" } else { &body.severity };
    util::one_of(severity, &["mild", "moderate", "severe"], "severity")?;
    sqlx::query(
        "INSERT INTO allergies (id, patient_id, tag, name_fa, name_en, severity, note, noted_at)
         VALUES (?,?,?,?,?,?,?,?)",
    )
    .bind(&sub)
    .bind(&id)
    .bind(util::clean(&body.tag, 40)?)
    .bind(util::require_nonempty(&body.name_fa, "allergy name")?)
    .bind(util::clean(&body.name_en, 120)?)
    .bind(severity)
    .bind(util::clean(&body.note, 500)?)
    .bind(util::today(&state.cfg.tz))
    .execute(&state.pool)
    .await?;
    audit(&state.pool, Some(&user.id), &user.display_name(), &user.role, "allergy_add", "patient", &id, "", "success", &ip.0).await;
    Ok(Json(json!({ "id": sub })))
}

async fn add_condition(
    State(state): State<AppState>,
    ip: ClientIp,
    user: AuthUser,
    Path(id): Path<String>,
    Json(body): Json<ConditionInput>,
) -> ApiResult<Json<Value>> {
    rbac::require(&user, rbac::CLINICAL_WRITE)?;
    patient_exists(&state.pool, &id).await?;
    let sub = uuid::Uuid::new_v4().to_string();
    let (name_fa, name_en) = if let Some(title) = icd_title(&state.pool, &body.code).await? {
        title
    } else {
        (
            util::require_nonempty(&body.name_fa, "condition name")?,
            util::clean(&body.name_en, 160)?,
        )
    };
    sqlx::query(
        "INSERT INTO conditions (id, patient_id, code, name_fa, name_en, chronic, noted_at)
         VALUES (?,?,?,?,?,?,?)",
    )
    .bind(&sub)
    .bind(&id)
    .bind(util::clean(&body.code, 16)?)
    .bind(name_fa)
    .bind(name_en)
    .bind(body.chronic as i32)
    .bind(util::today(&state.cfg.tz))
    .execute(&state.pool)
    .await?;
    audit(&state.pool, Some(&user.id), &user.display_name(), &user.role, "condition_add", "patient", &id, &body.code, "success", &ip.0).await;
    Ok(Json(json!({ "id": sub })))
}

async fn icd_title(pool: &SqlitePool, code: &str) -> ApiResult<Option<(String, String)>> {
    if code.trim().is_empty() {
        return Ok(None);
    }
    let row = sqlx::query("SELECT title_fa, title_en FROM icd10 WHERE code = ?")
        .bind(code.trim())
        .fetch_optional(pool)
        .await?;
    Ok(row.map(|r| (r.get("title_fa"), r.get("title_en"))))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConditionInput {
    #[serde(default)]
    pub code: String,
    #[serde(default)]
    pub name_fa: String,
    #[serde(default)]
    pub name_en: String,
    #[serde(default)]
    pub chronic: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MedInput {
    pub text_fa: String,
    #[serde(default)]
    pub text_en: String,
}

async fn add_med(
    State(state): State<AppState>,
    ip: ClientIp,
    user: AuthUser,
    Path(id): Path<String>,
    Json(body): Json<MedInput>,
) -> ApiResult<Json<Value>> {
    rbac::require(&user, rbac::CLINICAL_WRITE)?;
    patient_exists(&state.pool, &id).await?;
    let sub = uuid::Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO patient_meds (id, patient_id, text_fa, text_en) VALUES (?,?,?,?)")
        .bind(&sub)
        .bind(&id)
        .bind(util::require_nonempty(&body.text_fa, "medication")?)
        .bind(util::clean(&body.text_en, 200)?)
        .execute(&state.pool)
        .await?;
    audit(&state.pool, Some(&user.id), &user.display_name(), &user.role, "med_add", "patient", &id, "", "success", &ip.0).await;
    Ok(Json(json!({ "id": sub })))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AlertInput {
    pub text_fa: String,
    #[serde(default)]
    pub text_en: String,
}

async fn add_alert(
    State(state): State<AppState>,
    ip: ClientIp,
    user: AuthUser,
    Path(id): Path<String>,
    Json(body): Json<AlertInput>,
) -> ApiResult<Json<Value>> {
    rbac::require(&user, rbac::CLINICAL_WRITE)?;
    patient_exists(&state.pool, &id).await?;
    let sub = uuid::Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO patient_alerts (id, patient_id, text_fa, text_en, created_at) VALUES (?,?,?,?,?)",
    )
    .bind(&sub)
    .bind(&id)
    .bind(util::require_nonempty(&body.text_fa, "alert")?)
    .bind(util::clean(&body.text_en, 300)?)
    .bind(util::today(&state.cfg.tz))
    .execute(&state.pool)
    .await?;
    audit(&state.pool, Some(&user.id), &user.display_name(), &user.role, "alert_add", "patient", &id, "", "success", &ip.0).await;
    Ok(Json(json!({ "id": sub })))
}

async fn remove_sub(
    state: &AppState,
    user: &AuthUser,
    ip: &ClientIp,
    patient_id: &str,
    table: &str,
    sub: &str,
    action: &str,
) -> ApiResult<Json<Value>> {
    rbac::require(user, rbac::CLINICAL_WRITE)?;
    let sql = format!("DELETE FROM {table} WHERE id = ? AND patient_id = ?");
    let res = sqlx::query(&sql)
        .bind(sub)
        .bind(patient_id)
        .execute(&state.pool)
        .await?;
    if res.rows_affected() == 0 {
        return Err(AppError::not_found("record"));
    }
    audit(&state.pool, Some(&user.id), &user.display_name(), &user.role, action, "patient", patient_id, sub, "success", &ip.0).await;
    Ok(Json(json!({ "ok": true })))
}

async fn remove_allergy(State(state): State<AppState>, ip: ClientIp, user: AuthUser, Path((id, sub)): Path<(String, String)>) -> ApiResult<Json<Value>> {
    remove_sub(&state, &user, &ip, &id, "allergies", &sub, "allergy_remove").await
}
async fn remove_condition(State(state): State<AppState>, ip: ClientIp, user: AuthUser, Path((id, sub)): Path<(String, String)>) -> ApiResult<Json<Value>> {
    remove_sub(&state, &user, &ip, &id, "conditions", &sub, "condition_remove").await
}
async fn remove_med(State(state): State<AppState>, ip: ClientIp, user: AuthUser, Path((id, sub)): Path<(String, String)>) -> ApiResult<Json<Value>> {
    remove_sub(&state, &user, &ip, &id, "patient_meds", &sub, "med_remove").await
}
async fn remove_alert(State(state): State<AppState>, ip: ClientIp, user: AuthUser, Path((id, sub)): Path<(String, String)>) -> ApiResult<Json<Value>> {
    remove_sub(&state, &user, &ip, &id, "patient_alerts", &sub, "alert_remove").await
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MergeInput {
    pub target_id: String,
    #[serde(default)]
    pub reason: String,
}

pub async fn merge_patient(
    State(state): State<AppState>,
    ip: ClientIp,
    user: AuthUser,
    Path(id): Path<String>,
    Json(body): Json<MergeInput>,
) -> ApiResult<Json<Value>> {
    rbac::require(&user, rbac::PATIENT_MERGE)?;
    if id == body.target_id {
        return Err(AppError::bad("cannot merge a patient into itself"));
    }
    patient_exists(&state.pool, &id).await?;
    patient_exists(&state.pool, &body.target_id).await?;

    let alias: Option<String> = sqlx::query_scalar("SELECT merged_into FROM patients WHERE id = ?")
        .bind(&id)
        .fetch_one(&state.pool)
        .await?;
    if alias.is_some() {
        return Err(AppError::conflict("patient is already merged"));
    }

    let mut tx = state.pool.begin().await?;
    // Clinical and financial history moves to the surviving record; the alias keeps old links resolving.
    for sql in [
        "UPDATE appointments SET patient_id = ? WHERE patient_id = ?",
        "UPDATE encounters SET patient_id = ? WHERE patient_id = ?",
        "UPDATE lab_orders SET patient_id = ? WHERE patient_id = ?",
        "UPDATE invoices SET patient_id = ? WHERE patient_id = ?",
        "UPDATE follow_ups SET patient_id = ? WHERE patient_id = ?",
        "UPDATE vitals SET patient_id = ? WHERE patient_id = ?",
        "UPDATE nurse_notes SET patient_id = ? WHERE patient_id = ?",
        "UPDATE procedures SET patient_id = ? WHERE patient_id = ?",
        "UPDATE allergies SET patient_id = ? WHERE patient_id = ?",
        "UPDATE conditions SET patient_id = ? WHERE patient_id = ?",
        "UPDATE patient_meds SET patient_id = ? WHERE patient_id = ?",
        "UPDATE patient_alerts SET patient_id = ? WHERE patient_id = ?",
        "UPDATE queue SET patient_id = ? WHERE patient_id = ?",
    ] {
        sqlx::query(sql)
            .bind(&body.target_id)
            .bind(&id)
            .execute(&mut *tx)
            .await?;
    }
    sqlx::query("UPDATE patients SET status = 'archived', merged_into = ? WHERE id = ?")
        .bind(&body.target_id)
        .bind(&id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;

    audit(
        &state.pool,
        Some(&user.id),
        &user.display_name(),
        &user.role,
        "patient_merge",
        "patient",
        &id,
        &format!("into {} {}", body.target_id, body.reason),
        "success",
        &ip.0,
    )
    .await;
    Ok(Json(json!({ "ok": true, "mergedInto": body.target_id })))
}

/// Patient brief reused by appointments, queue, encounters and labs.
pub async fn patient_brief(pool: &SqlitePool, id: &str) -> ApiResult<Value> {
    let row = sqlx::query(
        "SELECT p.id, p.code, p.first_name_fa, p.first_name_en, p.last_name_fa, p.last_name_en,
                p.dob, p.gender, p.phone, p.blood_group, p.alerts_n
         FROM (SELECT p.*, (SELECT COUNT(*) FROM patient_alerts a WHERE a.patient_id = p.id) AS alerts_n
               FROM patients p) p WHERE p.id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::not_found("patient"))?;
    Ok(brief_from_row(&row))
}

pub fn brief_from_row(row: &SqliteRow) -> Value {
    json!({
        "id": row.get::<String, _>("id"),
        "code": row.get::<String, _>("code"),
        "firstNameFa": row.get::<String, _>("first_name_fa"),
        "firstNameEn": row.get::<String, _>("first_name_en"),
        "lastNameFa": row.get::<String, _>("last_name_fa"),
        "lastNameEn": row.get::<String, _>("last_name_en"),
        "dob": row.get::<String, _>("dob"),
        "gender": row.get::<String, _>("gender"),
        "phone": row.get::<String, _>("phone"),
        "bloodGroup": row.get::<String, _>("blood_group"),
    })
}

pub async fn audit_list(
    State(state): State<AppState>,
    user: AuthUser,
    Query(q): Query<AuditQuery>,
) -> ApiResult<Json<Value>> {
    rbac::require(&user, rbac::AUDIT_VIEW)?;
    let (per, offset) = util::paginate(q.page, q.per_page);
    fn audit_filters(qb: &mut QueryBuilder<'_, Sqlite>, q: &AuditQuery) -> ApiResult<()> {
        if let Some(action) = q.action.as_deref().filter(|s| !s.is_empty()) {
            qb.push(" AND action = ").push_bind(action.to_string());
        }
        if let Some(entity) = q.entity.as_deref().filter(|s| !s.is_empty()) {
            qb.push(" AND entity = ").push_bind(entity.to_string());
        }
        if let Some(term) = q.q.as_deref().filter(|s| !s.is_empty()) {
            let like = like_term(term);
            qb.push(" AND (lower(user_name) LIKE ")
                .push_bind(like.clone())
                .push(" OR lower(details) LIKE ")
                .push_bind(like.clone())
                .push(" OR entity_id LIKE ")
                .push_bind(like)
                .push(")");
        }
        if let Some(from) = q.from.as_deref().filter(|s| !s.is_empty()) {
            util::validate_date(from)?;
            qb.push(" AND at >= ").push_bind(format!("{from}T00:00:00"));
        }
        if let Some(to) = q.to.as_deref().filter(|s| !s.is_empty()) {
            util::validate_date(to)?;
            qb.push(" AND at <= ").push_bind(format!("{to}T23:59:59"));
        }
        Ok(())
    }

    let mut count_qb = QueryBuilder::<Sqlite>::new("SELECT COUNT(*) FROM audit_log WHERE 1 = 1");
    audit_filters(&mut count_qb, &q)?;
    let total: i64 = count_qb.build_query_scalar().fetch_one(&state.pool).await?;

    let mut qb = QueryBuilder::<Sqlite>::new("SELECT * FROM audit_log WHERE 1 = 1");
    audit_filters(&mut qb, &q)?;
    qb.push(" ORDER BY at DESC LIMIT ")
        .push_bind(per)
        .push(" OFFSET ")
        .push_bind(offset);
    let rows = qb.build().fetch_all(&state.pool).await?;
    let items: Vec<Value> = rows
        .iter()
        .map(|r| {
            json!({
                "id": r.get::<String, _>("id"),
                "at": r.get::<String, _>("at"),
                "userId": r.try_get::<Option<String>, _>("user_id").ok().flatten(),
                "user": r.get::<String, _>("user_name"),
                "role": r.get::<String, _>("role"),
                "action": r.get::<String, _>("action"),
                "entity": r.get::<String, _>("entity"),
                "entityId": r.get::<String, _>("entity_id"),
                "details": r.get::<String, _>("details"),
                "outcome": r.get::<String, _>("outcome"),
                "ip": r.get::<String, _>("ip"),
            })
        })
        .collect();
    Ok(Json(json!({ "items": items, "meta": paging(total, per, offset) })))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditQuery {
    pub q: Option<String>,
    pub action: Option<String>,
    pub entity: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

pub fn routes_audit() -> Router<AppState> {
    Router::new().route("/api/audit", get(audit_list))
}

