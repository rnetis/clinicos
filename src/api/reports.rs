use axum::extract::{Query, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::{Row, SqlitePool};

use crate::auth::AuthUser;
use crate::error::{ApiResult, AppError};
use crate::rbac;
use crate::state::AppState;
use crate::util;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/reports/summary", get(summary))
        .route("/api/reports/export", get(export))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RangeQuery {
    pub from: Option<String>,
    pub to: Option<String>,
}

fn range(q: &RangeQuery, tz: &chrono_tz::Tz) -> ApiResult<(String, String)> {
    let to = q.to.clone().unwrap_or_else(|| util::today(tz));
    let from = q.from.clone().unwrap_or_else(|| util::add_days(&to, -29).unwrap_or_else(|_| to.clone()));
    util::validate_date(&from)?;
    util::validate_date(&to)?;
    if to < from {
        return Err(AppError::bad("invalid date range"));
    }
    Ok((from, to))
}

pub async fn summary(
    State(state): State<AppState>,
    user: AuthUser,
    Query(q): Query<RangeQuery>,
) -> ApiResult<Json<Value>> {
    rbac::require(&user, rbac::PATIENT_SEARCH)?;
    let (from, to) = range(&q, &state.cfg.tz)?;
    let pool = &state.pool;
    let financial = rbac::can(&user.role, rbac::INVOICE_VIEW) || rbac::can(&user.role, rbac::REPORTS_FINANCIAL);
    let own_doctor = if user.role == "doctor" { user.doctor_id.clone() } else { None };

    let visits: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM encounters WHERE date BETWEEN ? AND ? AND status IN ('signed','amended')
           AND (? IS NULL OR doctor_id = ?)",
    )
    .bind(&from)
    .bind(&to)
    .bind(own_doctor.as_deref())
    .bind(own_doctor.as_deref())
    .fetch_one(pool)
    .await?;
    let unique_patients: i64 = sqlx::query_scalar(
        "SELECT COUNT(DISTINCT patient_id) FROM encounters WHERE date BETWEEN ? AND ? AND status IN ('signed','amended')
           AND (? IS NULL OR doctor_id = ?)",
    )
    .bind(&from)
    .bind(&to)
    .bind(own_doctor.as_deref())
    .bind(own_doctor.as_deref())
    .fetch_one(pool)
    .await?;
    let new_patients: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM patients WHERE substr(created_at, 1, 10) BETWEEN ? AND ? AND merged_into IS NULL",
    )
    .bind(&from)
    .bind(&to)
    .fetch_one(pool)
    .await?;
    let appointments: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM appointments WHERE date BETWEEN ? AND ? AND status <> 'cancelled'
           AND (? IS NULL OR doctor_id = ?)",
    )
    .bind(&from)
    .bind(&to)
    .bind(own_doctor.as_deref())
    .bind(own_doctor.as_deref())
    .fetch_one(pool)
    .await?;
    let no_shows: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM appointments WHERE date BETWEEN ? AND ? AND status = 'no_show'
           AND (? IS NULL OR doctor_id = ?)",
    )
    .bind(&from)
    .bind(&to)
    .bind(own_doctor.as_deref())
    .bind(own_doctor.as_deref())
    .fetch_one(pool)
    .await?;
    let revenue: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(CASE WHEN kind = 'refund' THEN -amount ELSE amount END), 0)
         FROM payments WHERE substr(at, 1, 10) BETWEEN ? AND ?",
    )
    .bind(&from)
    .bind(&to)
    .fetch_one(pool)
    .await?;
    let billed: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(due), 0) FROM invoice_balance
         WHERE date BETWEEN ? AND ? AND status <> 'cancelled'",
    )
    .bind(&from)
    .bind(&to)
    .fetch_one(pool)
    .await?;
    let outstanding: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(outstanding), 0) FROM invoice_balance
         WHERE status NOT IN ('paid','refunded','cancelled')",
    )
    .fetch_one(pool)
    .await?;
    let invoice_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM invoices WHERE date BETWEEN ? AND ? AND status <> 'cancelled'",
    )
    .bind(&from)
    .bind(&to)
    .fetch_one(pool)
    .await?;

    let by_day = sqlx::query(
        "SELECT e.date AS day, COUNT(*) AS visits
         FROM encounters e WHERE e.date BETWEEN ? AND ? AND e.status IN ('signed','amended')
           AND (? IS NULL OR e.doctor_id = ?)
         GROUP BY e.date ORDER BY e.date",
    )
    .bind(&from)
    .bind(&to)
    .bind(own_doctor.as_deref())
    .bind(own_doctor.as_deref())
    .fetch_all(pool)
    .await?;

    let by_doctor = sqlx::query(
        "SELECT d.id, d.name_fa, d.name_en,
                (SELECT COUNT(*) FROM encounters e WHERE e.doctor_id = d.id AND e.date BETWEEN ? AND ?
                   AND e.status IN ('signed','amended')) AS visits,
                (SELECT COALESCE(SUM(CASE WHEN p.kind = 'refund' THEN -p.amount ELSE p.amount END), 0)
                   FROM payments p JOIN invoices i ON i.id = p.invoice_id
                   WHERE i.doctor_id = d.id AND substr(p.at, 1, 10) BETWEEN ? AND ?) AS revenue
         FROM doctors d WHERE d.active = 1 ORDER BY visits DESC",
    )
    .bind(&from)
    .bind(&to)
    .bind(&from)
    .bind(&to)
    .fetch_all(pool)
    .await?;

    let by_service = sqlx::query(
        "SELECT COALESCE(s.category, 'other') AS category,
                COUNT(*) AS count, COALESCE(SUM(it.qty * it.price), 0) AS amount
         FROM invoice_items it JOIN invoices i ON i.id = it.invoice_id
         LEFT JOIN services s ON s.id = it.service_id
         WHERE i.date BETWEEN ? AND ? AND i.status <> 'cancelled'
         GROUP BY category ORDER BY amount DESC",
    )
    .bind(&from)
    .bind(&to)
    .fetch_all(pool)
    .await?;

    let by_method = sqlx::query(
        "SELECT method, COUNT(*) AS count,
                COALESCE(SUM(CASE WHEN kind = 'refund' THEN -amount ELSE amount END), 0) AS amount
         FROM payments WHERE substr(at, 1, 10) BETWEEN ? AND ?
         GROUP BY method ORDER BY amount DESC",
    )
    .bind(&from)
    .bind(&to)
    .fetch_all(pool)
    .await?;

    let diagnoses = top_diagnoses(pool, &from, &to, own_doctor.as_deref()).await?;

    let lab_turnaround = sqlx::query(
        "SELECT
           SUM(CASE WHEN status = 'completed' THEN 1 ELSE 0 END) AS completed,
           SUM(CASE WHEN status = 'reviewed' THEN 1 ELSE 0 END) AS reviewed,
           SUM(CASE WHEN priority IN ('urgent','stat') THEN 1 ELSE 0 END) AS urgent
         FROM lab_orders WHERE date BETWEEN ? AND ?",
    )
    .bind(&from)
    .bind(&to)
    .fetch_one(pool)
    .await?;

    Ok(Json(json!({
        "from": from,
        "to": to,
        "totals": {
            "visits": visits,
            "uniquePatients": unique_patients,
            "newPatients": new_patients,
            "appointments": appointments,
            "noShows": no_shows,
            "noShowRate": if appointments > 0 { (no_shows as f64 / appointments as f64 * 100.0).round() } else { 0.0 },
            "revenue": if financial { Some(revenue) } else { None },
            "billed": if financial { Some(billed) } else { None },
            "outstanding": if financial { Some(outstanding) } else { None },
            "averageInvoice": if financial && invoice_count > 0 { Some(billed / invoice_count) } else { None },
        },
        "visitsByDay": by_day.iter().map(|r| json!({
            "date": r.get::<String, _>("day"),
            "visits": r.get::<i64, _>("visits"),
        })).collect::<Vec<_>>(),
        "byDoctor": by_doctor.iter().map(|r| json!({
            "id": r.get::<String, _>("id"),
            "name": super::txt(r.get::<String, _>("name_fa"), r.get::<String, _>("name_en")),
            "visits": r.get::<i64, _>("visits"),
            "revenue": if financial { Some(r.get::<i64, _>("revenue")) } else { None },
        })).collect::<Vec<_>>(),
        "byServiceCategory": by_service.iter().map(|r| json!({
            "category": r.get::<String, _>("category"),
            "count": r.get::<i64, _>("count"),
            "amount": if financial { Some(r.get::<i64, _>("amount")) } else { None },
        })).collect::<Vec<_>>(),
        "byPaymentMethod": by_method.iter().map(|r| json!({
            "method": r.get::<String, _>("method"),
            "count": r.get::<i64, _>("count"),
            "amount": if financial { Some(r.get::<i64, _>("amount")) } else { None },
        })).collect::<Vec<_>>(),
        "topDiagnoses": diagnoses,
        "labs": {
            "completed": lab_turnaround.try_get::<Option<i64>, _>("completed").ok().flatten().unwrap_or(0),
            "reviewed": lab_turnaround.try_get::<Option<i64>, _>("reviewed").ok().flatten().unwrap_or(0),
            "urgent": lab_turnaround.try_get::<Option<i64>, _>("urgent").ok().flatten().unwrap_or(0),
        },
    })))
}

async fn top_diagnoses(
    pool: &SqlitePool,
    from: &str,
    to: &str,
    doctor: Option<&str>,
) -> ApiResult<Value> {
    let rows = sqlx::query(
        "SELECT v.data FROM encounter_versions v JOIN encounters e ON e.id = v.encounter_id
         WHERE e.date BETWEEN ? AND ? AND e.status IN ('signed','amended')
           AND v.version = (SELECT MAX(x.version) FROM encounter_versions x WHERE x.encounter_id = e.id)
           AND (? IS NULL OR e.doctor_id = ?)",
    )
    .bind(from)
    .bind(to)
    .bind(doctor)
    .bind(doctor)
    .fetch_all(pool)
    .await?;

    let mut counts: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
    for row in rows {
        let raw: String = row.get("data");
        if let Ok(value) = serde_json::from_str::<Value>(&raw) {
            if let Some(list) = value.get("diagnoses").and_then(|d| d.as_array()) {
                for dx in list {
                    if let Some(code) = dx.get("code").and_then(|c| c.as_str()) {
                        *counts.entry(code.to_string()).or_insert(0) += 1;
                    }
                }
            }
        }
    }
    let mut entries: Vec<(String, i64)> = counts.into_iter().collect();
    entries.sort_by(|a, b| b.1.cmp(&a.1));
    entries.truncate(10);

    let mut out = Vec::new();
    for (code, count) in entries {
        let title = sqlx::query("SELECT title_fa, title_en FROM icd10 WHERE code = ?")
            .bind(&code)
            .fetch_optional(pool)
            .await?;
        out.push(json!({
            "code": code,
            "count": count,
            "title": title.map(|t| super::txt(t.get::<String, _>("title_fa"), t.get::<String, _>("title_en"))),
        }));
    }
    Ok(json!(out))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportQuery {
    #[serde(rename = "type")]
    pub kind: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
}

pub async fn export(
    State(state): State<AppState>,
    user: AuthUser,
    Query(q): Query<ExportQuery>,
) -> ApiResult<Response> {
    let (from, to) = range(
        &RangeQuery { from: q.from.clone(), to: q.to.clone() },
        &state.cfg.tz,
    )?;
    let kind = q.kind.clone().unwrap_or_else(|| "invoices".into());
    if matches!(kind.as_str(), "invoices" | "payments") {
        rbac::require(&user, rbac::INVOICE_VIEW)?;
    } else {
        rbac::require(&user, rbac::PATIENT_SEARCH)?;
    }

    let (filename, mut rows): (String, Vec<Vec<String>>) = match kind.as_str() {
        "patients" => {
            let data = sqlx::query(
                "SELECT code, first_name_fa, last_name_fa, national_id, phone, dob, gender, status, created_at
                 FROM patients WHERE merged_into IS NULL ORDER BY created_at DESC",
            )
            .fetch_all(&state.pool)
            .await?;
            let mut rows = vec![vec![
                "code".into(), "first_fa".into(), "last_fa".into(), "national_id".into(),
                "phone".into(), "dob".into(), "gender".into(), "status".into(), "created_at".into(),
            ]];
            rows.extend(data.iter().map(|r| {
                vec![
                    r.get::<String, _>("code"),
                    r.get::<String, _>("first_name_fa"),
                    r.get::<String, _>("last_name_fa"),
                    r.get::<String, _>("national_id"),
                    r.get::<String, _>("phone"),
                    r.get::<String, _>("dob"),
                    r.get::<String, _>("gender"),
                    r.get::<String, _>("status"),
                    r.get::<String, _>("created_at"),
                ]
            }));
            (format!("patients-{to}.csv"), rows)
        }
        "appointments" => {
            let data = sqlx::query(
                "SELECT a.date, a.time, a.status, a.type, p.code AS patient_code,
                        (p.first_name_fa || ' ' || p.last_name_fa) AS patient,
                        d.name_fa AS doctor
                 FROM appointments a JOIN patients p ON p.id = a.patient_id
                 JOIN doctors d ON d.id = a.doctor_id
                 WHERE a.date BETWEEN ? AND ? ORDER BY a.date, a.time",
            )
            .bind(&from)
            .bind(&to)
            .fetch_all(&state.pool)
            .await?;
            let mut rows = vec![vec![
                "date".into(), "time".into(), "status".into(), "type".into(),
                "patient_code".into(), "patient".into(), "doctor".into(),
            ]];
            rows.extend(data.iter().map(|r| {
                vec![
                    r.get::<String, _>("date"),
                    r.get::<String, _>("time"),
                    r.get::<String, _>("status"),
                    r.get::<String, _>("type"),
                    r.get::<String, _>("patient_code"),
                    r.get::<String, _>("patient"),
                    r.get::<String, _>("doctor"),
                ]
            }));
            (format!("appointments-{from}_{to}.csv"), rows)
        }
        "payments" => {
            let data = sqlx::query(
                "SELECT p.at, i.no, p.kind, p.method, p.amount, p.ref,
                        (pt.first_name_fa || ' ' || pt.last_name_fa) AS patient
                 FROM payments p JOIN invoices i ON i.id = p.invoice_id
                 JOIN patients pt ON pt.id = i.patient_id
                 WHERE substr(p.at, 1, 10) BETWEEN ? AND ? ORDER BY p.at",
            )
            .bind(&from)
            .bind(&to)
            .fetch_all(&state.pool)
            .await?;
            let mut rows = vec![vec![
                "at".into(), "invoice".into(), "kind".into(), "method".into(), "amount".into(),
                "reference".into(), "patient".into(),
            ]];
            rows.extend(data.iter().map(|r| {
                vec![
                    r.get::<String, _>("at"),
                    r.get::<String, _>("no"),
                    r.get::<String, _>("kind"),
                    r.get::<String, _>("method"),
                    r.get::<i64, _>("amount").to_string(),
                    r.get::<String, _>("ref"),
                    r.get::<String, _>("patient"),
                ]
            }));
            (format!("payments-{from}_{to}.csv"), rows)
        }
        _ => {
            let data = sqlx::query(
                "SELECT b.no, b.date, b.status, b.subtotal, b.discount, b.insurance, b.due, b.paid, b.outstanding,
                        (p.first_name_fa || ' ' || p.last_name_fa) AS patient
                 FROM invoice_balance b JOIN patients p ON p.id = b.patient_id
                 WHERE b.date BETWEEN ? AND ? ORDER BY b.date, b.no",
            )
            .bind(&from)
            .bind(&to)
            .fetch_all(&state.pool)
            .await?;
            let mut rows = vec![vec![
                "invoice".into(), "date".into(), "status".into(), "subtotal".into(), "discount".into(),
                "insurance".into(), "due".into(), "paid".into(), "outstanding".into(), "patient".into(),
            ]];
            rows.extend(data.iter().map(|r| {
                vec![
                    r.get::<String, _>("no"),
                    r.get::<String, _>("date"),
                    r.get::<String, _>("status"),
                    r.get::<i64, _>("subtotal").to_string(),
                    r.get::<i64, _>("discount").to_string(),
                    r.get::<i64, _>("insurance").to_string(),
                    r.get::<i64, _>("due").to_string(),
                    r.get::<i64, _>("paid").to_string(),
                    r.get::<i64, _>("outstanding").to_string(),
                    r.get::<String, _>("patient"),
                ]
            }));
            (format!("invoices-{from}_{to}.csv"), rows)
        }
    };

    let mut csv = String::from("\u{feff}");
    for row in rows.drain(..) {
        csv.push_str(
            &row.iter()
                .map(|cell| util::csv_escape(cell))
                .collect::<Vec<_>>()
                .join(","),
        );
        csv.push('\n');
    }

    crate::db::audit(
        &state.pool,
        Some(&user.id),
        &user.display_name(),
        &user.role,
        "export",
        "report",
        &kind,
        &format!("{from}..{to}"),
        "success",
        "",
    )
    .await;

    let mut headers = axum::http::HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/csv; charset=utf-8"),
    );
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&format!("attachment; filename=\"{filename}\""))
            .map_err(|_| AppError::bad("invalid filename"))?,
    );
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    Ok((StatusCode::OK, headers, csv).into_response())
}
