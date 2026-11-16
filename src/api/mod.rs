pub mod admin;
pub mod appointments;
pub mod billing;
pub mod dashboard;
pub mod encounters;
pub mod labs;
pub mod notifications;
pub mod patients;
pub mod queue;
pub mod reports;

use serde_json::{json, Value};

/// Bilingual text value used by free-form clinical documents.
pub fn txt(fa: impl Into<String>, en: impl Into<String>) -> Value {
    json!({ "fa": fa.into(), "en": en.into() })
}

pub fn paging(total: i64, per: i64, offset: i64) -> Value {
    let page = if per > 0 { offset / per + 1 } else { 1 };
    json!({
        "total": total,
        "page": page,
        "perPage": per,
        "pages": if per > 0 { (total + per - 1) / per } else { 1 },
    })
}

/// Normalises a search term the same way for every list endpoint.
pub fn like_term(q: &str) -> String {
    let folded: String = q
        .chars()
        .map(|c| match c {
            '\u{06F0}'..='\u{06F9}' => char::from_u32(c as u32 - 0x06F0 + 48).unwrap_or(c),
            '\u{0660}'..='\u{0669}' => char::from_u32(c as u32 - 0x0660 + 48).unwrap_or(c),
            other => other,
        })
        .collect();
    format!("%{}%", folded.trim().to_lowercase())
}
