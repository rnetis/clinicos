use axum::extract::{Query, State};
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::{Row, SqlitePool};

use super::txt;
use crate::auth::AuthUser;
use crate::error::ApiResult;
use crate::rbac;
use crate::state::AppState;
use crate::util;

pub fn routes() -> Router<AppState> {
    Router::new().route("/api/dashboard", get(summary))
}

#[derive(Debug, Deserialize)]
pub struct QueryParams {
    pub date: Option<String>,
}

#[derive(Clone, Copy)]
struct Scope<'a> {
    doctor: Option<&'a str>,
}

pub async fn summary(
    State(state): State<AppState>,
    user: AuthUser,
    Query(q): Query<QueryParams>,
) -> ApiResult<Json<Value>> {
    let date = q.date.unwrap_or_else(|| util::today(&state.cfg.tz));
    util::validate_date(&date)?;
    let show_money = rbac::can(&user.role, rbac::INVOICE_VIEW);
    let scope = Scope {
        doctor: if user.role == "doctor" { user.doctor_id.as_deref() } else { None },
    };
    let patient_scope = scope.doctor;
    let pool = &state.pool;

    let count = |sql: String, args: Vec<String>| {
        let pool = pool.clone();
        async move {
            let mut query = sqlx::query_scalar::<_, i64>(&sql);
            for a in &args {
                query = query.bind(a);
            }
            query.fetch_one(&pool).await.unwrap_or(0)
        }
    };

    let appt_filter = match scope.doctor {
        Some(_) => " AND doctor_id = ?",
        None => "",
    };
    let appts_total = count(
        format!("SELECT COUNT(*) FROM appointments WHERE date = ? AND status <> 'cancelled'{appt_filter}"),
        args(&[&date], patient_scope),
    )
    .await;
    let appts_done = count(
        format!("SELECT COUNT(*) FROM appointments WHERE date = ? AND status = 'completed'{appt_filter}"),
        args(&[&date], patient_scope),
    )
    .await;
    let waiting = count(
        format!("SELECT COUNT(*) FROM queue WHERE date = ? AND status IN ('waiting','called'){appt_filter}"),
        args(&[&date], patient_scope),
    )
    .await;
    let in_progress = count(
        format!("SELECT COUNT(*) FROM queue WHERE date = ? AND status = 'in_progress'{appt_filter}"),
        args(&[&date], patient_scope),
    )
    .await;
    let drafts = count(
        format!("SELECT COUNT(*) FROM encounters WHERE status = 'draft'{appt_filter}"),
        args(&[], patient_scope),
    )
    .await;
    let labs_pending = count(
        format!("SELECT COUNT(*) FROM lab_orders WHERE status IN ('ordered','collected','in_progress'){appt_filter}"),
        args(&[], patient_scope),
    )
    .await;
    let critical = count(
        format!(
            "SELECT COUNT(*) FROM lab_results r JOIN lab_orders l ON l.id = r.order_id
             WHERE r.flag = 'critical' AND r.reviewed_at IS NULL{}{}",
            if scope.doctor.is_some() { " AND l.doctor_id = ?" } else { "" },
            ""
        ),
        args(&[], scope.doctor),
    )
    .await;
    let overdue = count(
        format!(
            "SELECT COUNT(*) FROM follow_ups WHERE status = 'pending' AND due < ?{}{}",
            if scope.doctor.is_some() { " AND doctor_id = ?" } else { "" },
            ""
        ),
        args(&[&date], scope.doctor),
    )
    .await;
    let new_patients = count(
        "SELECT COUNT(*) FROM patients WHERE substr(created_at, 1, 10) = ? AND merged_into IS NULL".to_string(),
        vec![date.clone()],
    )
    .await;

    let revenue_today: i64 = if show_money {
        sqlx::query_scalar(
            "SELECT COALESCE(SUM(CASE WHEN kind = 'refund' THEN -amount ELSE amount END), 0)
             FROM payments WHERE substr(at, 1, 10) = ?",
        )
        .bind(&date)
        .fetch_one(pool)
        .await
        .unwrap_or(0)
    } else {
        0
    };
    let outstanding: i64 = if show_money {
        sqlx::query_scalar("SELECT COALESCE(SUM(outstanding), 0) FROM invoice_balance WHERE status NOT IN ('paid','refunded','cancelled')")
            .fetch_one(pool)
            .await
            .unwrap_or(0)
    } else {
        0
    };

    let appointments = sqlx::query(
        "SELECT a.id, a.time, a.duration, a.status, a.type, a.patient_id, a.reason_fa, a.reason_en,
                (p.first_name_fa || ' ' || p.last_name_fa) AS name_fa,
                (p.first_name_en || ' ' || p.last_name_en) AS name_en, p.code AS patient_code,
                d.name_fa AS doctor_fa, d.name_en AS doctor_en, d.color AS doctor_color,
                (SELECT q.number FROM queue q WHERE q.appointment_id = a.id AND q.status <> 'cancelled' LIMIT 1) AS queue_number
         FROM appointments a JOIN patients p ON p.id = a.patient_id JOIN doctors d ON d.id = a.doctor_id
         WHERE a.date = ? AND a.status <> 'cancelled'
           AND (? IS NULL OR a.doctor_id = ?)
         ORDER BY a.time LIMIT 60",
    )
    .bind(&date)
    .bind(scope.doctor)
    .bind(scope.doctor)
    .fetch_all(pool)
    .await?;

    let queue = sqlx::query(
        "SELECT q.id, q.number, q.status, q.priority, q.patient_id, q.checked_in_at, q.started_at,
                (p.first_name_fa || ' ' || p.last_name_fa) AS name_fa,
                (p.first_name_en || ' ' || p.last_name_en) AS name_en, p.code AS patient_code,
                d.name_fa AS doctor_fa, d.name_en AS doctor_en
         FROM queue q JOIN patients p ON p.id = q.patient_id JOIN doctors d ON d.id = q.doctor_id
         WHERE q.date = ? AND q.status IN ('waiting','called','in_progress')
           AND (? IS NULL OR q.doctor_id = ?)
         ORDER BY q.priority DESC, q.number LIMIT 20",
    )
    .bind(&date)
    .bind(scope.doctor)
    .bind(scope.doctor)
    .fetch_all(pool)
    .await?;

    let critical_results = sqlx::query(
        "SELECT r.value, r.flag, t.code, t.name_fa, t.name_en, l.id AS order_id,
                (p.first_name_fa || ' ' || p.last_name_fa) AS name_fa,
                (p.first_name_en || ' ' || p.last_name_en) AS name_en, p.id AS patient_id
         FROM lab_results r JOIN lab_orders l ON l.id = r.order_id
         JOIN lab_tests t ON t.id = r.test_id JOIN patients p ON p.id = l.patient_id
         WHERE r.flag = 'critical' AND r.reviewed_at IS NULL
           AND (? IS NULL OR l.doctor_id = ?)
         ORDER BY r.at DESC LIMIT 8",
    )
    .bind(scope.doctor)
    .bind(scope.doctor)
    .fetch_all(pool)
    .await?;

    let overdue_list = sqlx::query(
        "SELECT f.id, f.due, f.reason_fa, f.reason_en, f.priority, f.patient_id,
                (p.first_name_fa || ' ' || p.last_name_fa) AS name_fa,
                (p.first_name_en || ' ' || p.last_name_en) AS name_en, p.code AS patient_code
         FROM follow_ups f JOIN patients p ON p.id = f.patient_id
         WHERE f.status = 'pending' AND f.due < ?
           AND (? IS NULL OR f.doctor_id = ?)
         ORDER BY f.due LIMIT 8",
    )
    .bind(&date)
    .bind(scope.doctor)
    .bind(scope.doctor)
    .fetch_all(pool)
    .await?;

    let mut payload = json!({
        "date": date,
        "role": user.role,
        "kpis": {
            "appointments": appts_total,
            "completed": appts_done,
            "waiting": waiting,
            "inProgress": in_progress,
            "drafts": drafts,
            "labsPending": labs_pending,
            "criticalResults": critical,
            "overdueFollowUps": overdue,
            "newPatients": new_patients,
            "revenueToday": if show_money { Some(revenue_today) } else { None },
            "outstanding": if show_money { Some(outstanding) } else { None },
        },
        "appointments": appointments.iter().map(|r| json!({
            "id": r.get::<String, _>("id"),
            "time": r.get::<String, _>("time"),
            "duration": r.get::<i64, _>("duration"),
            "status": r.get::<String, _>("status"),
            "type": r.get::<String, _>("type"),
            "patientId": r.get::<String, _>("patient_id"),
            "patientName": txt(r.get::<String, _>("name_fa"), r.get::<String, _>("name_en")),
            "patientCode": r.get::<String, _>("patient_code"),
            "reason": txt(r.get::<String, _>("reason_fa"), r.get::<String, _>("reason_en")),
            "doctorName": txt(r.get::<String, _>("doctor_fa"), r.get::<String, _>("doctor_en")),
            "doctorColor": r.get::<String, _>("doctor_color"),
            "queueNumber": r.try_get::<Option<i64>, _>("queue_number").ok().flatten(),
        })).collect::<Vec<_>>(),
        "queue": queue.iter().map(|r| json!({
            "id": r.get::<String, _>("id"),
            "number": r.get::<i64, _>("number"),
            "status": r.get::<String, _>("status"),
            "priority": r.get::<i64, _>("priority") == 1,
            "patientId": r.get::<String, _>("patient_id"),
            "patientName": txt(r.get::<String, _>("name_fa"), r.get::<String, _>("name_en")),
            "patientCode": r.get::<String, _>("patient_code"),
            "doctorName": txt(r.get::<String, _>("doctor_fa"), r.get::<String, _>("doctor_en")),
            "checkedInAt": r.get::<String, _>("checked_in_at"),
            "startedAt": r.try_get::<Option<String>, _>("started_at").ok().flatten(),
        })).collect::<Vec<_>>(),
        "criticalResults": critical_results.iter().map(|r| json!({
            "orderId": r.get::<String, _>("order_id"),
            "patientId": r.get::<String, _>("patient_id"),
            "patientName": txt(r.get::<String, _>("name_fa"), r.get::<String, _>("name_en")),
            "test": txt(r.get::<String, _>("name_fa"), r.get::<String, _>("name_en")),
            "code": r.get::<String, _>("code"),
            "value": r.get::<String, _>("value"),
        })).collect::<Vec<_>>(),
        "overdueFollowUps": overdue_list.iter().map(|r| json!({
            "id": r.get::<String, _>("id"),
            "due": r.get::<String, _>("due"),
            "priority": r.get::<String, _>("priority"),
            "patientId": r.get::<String, _>("patient_id"),
            "patientName": txt(r.get::<String, _>("name_fa"), r.get::<String, _>("name_en")),
            "patientCode": r.get::<String, _>("patient_code"),
            "reason": txt(r.get::<String, _>("reason_fa"), r.get::<String, _>("reason_en")),
        })).collect::<Vec<_>>(),
    });

    if show_money {
        let trend = revenue_trend(pool, &date).await?;
        payload["revenueTrend"] = trend;
    }
    Ok(Json(payload))
}

fn args(extra: &[&String], doctor: Option<&str>) -> Vec<String> {
    let mut out: Vec<String> = extra.iter().map(|s| (*s).clone()).collect();
    if let Some(d) = doctor {
        out.push(d.to_string());
    }
    out
}

async fn revenue_trend(pool: &SqlitePool, today: &str) -> ApiResult<Value> {
    let from = util::add_days(today, -6)?;
    let rows = sqlx::query(
        "SELECT substr(at, 1, 10) AS day,
                COALESCE(SUM(CASE WHEN kind = 'refund' THEN -amount ELSE amount END), 0) AS amount
         FROM payments WHERE substr(at, 1, 10) BETWEEN ? AND ?
         GROUP BY day ORDER BY day",
    )
    .bind(&from)
    .bind(today)
    .fetch_all(pool)
    .await?;
    let mut out = Vec::new();
    for i in 0..7 {
        let day = util::add_days(&from, i)?;
        let amount = rows
            .iter()
            .find(|r| r.get::<String, _>("day") == day)
            .map(|r| r.get::<i64, _>("amount"))
            .unwrap_or(0);
        out.push(json!({ "date": day, "amount": amount }));
    }
    Ok(json!(out))
}
