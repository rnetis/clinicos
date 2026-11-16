use chrono::{Datelike, NaiveDate, NaiveTime, Timelike, Utc};
use chrono_tz::Tz;
use serde_json::Value;

use crate::error::{ApiResult, AppError};

/// Clinic-local "today" (never the server's UTC date).
pub fn today(tz: &Tz) -> String {
    Utc::now().with_timezone(tz).date_naive().to_string()
}

pub fn now() -> String {
    Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

pub fn validate_date(s: &str) -> ApiResult<()> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .map(|_| ())
        .map_err(|_| AppError::bad("invalid date, expected YYYY-MM-DD"))
}

pub fn validate_time(s: &str) -> ApiResult<()> {
    NaiveTime::parse_from_str(s, "%H:%M")
        .map(|_| ())
        .map_err(|_| AppError::bad("invalid time, expected HH:MM"))
}

pub fn time_minutes(s: &str) -> ApiResult<i32> {
    let t = NaiveTime::parse_from_str(s, "%H:%M")
        .map_err(|_| AppError::bad("invalid time, expected HH:MM"))?;
    Ok(t.hour() as i32 * 60 + t.minute() as i32)
}

pub fn minutes_time(m: i32) -> String {
    format!("{:02}:{:02}", m / 60, m % 60)
}

pub fn overlaps(a_start: &str, a_dur: i32, b_start: &str, b_dur: i32) -> ApiResult<bool> {
    let a = time_minutes(a_start)?;
    let b = time_minutes(b_start)?;
    Ok(a < b + b_dur && b < a + a_dur)
}

pub fn dow(date: &str) -> ApiResult<u32> {
    // 0 = Sunday .. 6 = Saturday, matching the clinic schedule model.
    let d = NaiveDate::parse_from_str(date, "%Y-%m-%d")
        .map_err(|_| AppError::bad("invalid date"))?;
    Ok(d.weekday().num_days_from_sunday())
}

pub fn add_days(date: &str, n: i64) -> ApiResult<String> {
    let d = NaiveDate::parse_from_str(date, "%Y-%m-%d")
        .map_err(|_| AppError::bad("invalid date"))?;
    Ok(d
        .checked_add_signed(chrono::Duration::days(n))
        .ok_or_else(|| AppError::bad("date out of range"))?
        .to_string())
}

/// Iranian national ID checksum (10 digits).
pub fn valid_national_id(raw: &str) -> bool {
    let s: String = raw.chars().filter(|c| c.is_ascii_digit()).collect();
    if s.len() != 10 {
        return false;
    }
    if s.chars().all(|c| c == s.chars().next().unwrap()) {
        return false;
    }
    let check: i32 = s[9..10].parse().unwrap_or(-1);
    let mut sum = 0;
    for (i, ch) in s[..9].chars().enumerate() {
        let d = ch.to_digit(10).unwrap_or(0) as i32;
        sum += d * (10 - i as i32);
    }
    let r = sum % 11;
    (r < 2 && check == r) || (r >= 2 && check == 11 - r)
}

/// Normalise Iranian mobile/landline numbers to a canonical form.
pub fn normalize_phone(raw: &str) -> String {
    let latin: String = raw
        .chars()
        .map(|c| match c {
            '\u{06F0}'..='\u{06F9}' => char::from_u32(c as u32 - 0x06F0 + 48).unwrap_or(c),
            '\u{0660}'..='\u{0669}' => char::from_u32(c as u32 - 0x0660 + 48).unwrap_or(c),
            other => other,
        })
        .collect();
    let digits: String = latin.chars().filter(|c| c.is_ascii_digit()).collect();
    if let Some(rest) = digits.strip_prefix("0098") {
        format!("0{rest}")
    } else if let Some(rest) = digits.strip_prefix("+98") {
        format!("0{rest}")
    } else if digits.len() == 12 && digits.starts_with("98") {
        format!("0{}", &digits[2..])
    } else if digits.len() == 10 && digits.starts_with('9') {
        format!("0{digits}")
    } else {
        digits
    }
}

pub fn validate_phone(phone: &str) -> ApiResult<()> {
    if phone.is_empty() {
        return Ok(());
    }
    let digits = phone.chars().filter(|c| c.is_ascii_digit()).count();
    if digits < 8 || digits > 15 {
        return Err(AppError::bad("invalid phone number"));
    }
    Ok(())
}

pub fn validate_email(email: &str) -> ApiResult<()> {
    if email.is_empty() {
        return Ok(());
    }
    let ok = email.contains('@')
        && email.split('@').nth(1).map(|d| d.contains('.')).unwrap_or(false)
        && !email.contains(char::is_whitespace)
        && email.len() <= 254;
    if ok {
        Ok(())
    } else {
        Err(AppError::bad("invalid email address"))
    }
}

pub fn clean(value: &str, max: usize) -> ApiResult<String> {
    let trimmed = value.trim();
    if trimmed.chars().count() > max {
        return Err(AppError::bad(format!("value exceeds {max} characters")));
    }
    Ok(trimmed.to_string())
}

pub fn require_nonempty(value: &str, field: &str) -> ApiResult<String> {
    let v = value.trim();
    if v.is_empty() {
        return Err(AppError::bad(format!("{field} is required")));
    }
    Ok(v.to_string())
}

pub fn money(v: i64) -> ApiResult<i64> {
    if !(0..=10_000_000_000).contains(&v) {
        return Err(AppError::bad("amount out of range"));
    }
    Ok(v)
}

pub fn one_of(value: &str, allowed: &[&str], field: &str) -> ApiResult<String> {
    if allowed.contains(&value) {
        Ok(value.to_string())
    } else {
        Err(AppError::bad(format!("invalid {field}")))
    }
}

/// Parse reference ranges such as "70–100", "< 20", "LDL < 130".
/// Returns (low, high) bounds when numeric.
pub fn parse_ref(reference: &str) -> Option<(f64, Option<f64>)> {
    let cleaned = reference
        .replace('–', "-")
        .replace('—', "-")
        .replace('٫', ".")
        .replace(',', "");
    // "3.5-5.1" uses the dash as a range separator, so only keep a leading sign.
    let chars: Vec<char> = cleaned.chars().collect();
    let mut normalised = String::with_capacity(cleaned.len());
    for (i, c) in chars.iter().enumerate() {
        let between_digits = *c == '-'
            && i > 0
            && i + 1 < chars.len()
            && chars[i - 1].is_ascii_digit()
            && chars[i + 1].is_ascii_digit();
        normalised.push(if between_digits { ' ' } else { *c });
    }
    let numbers: Vec<f64> = normalised
        .split_whitespace()
        .filter_map(|token| {
            token
                .trim_matches(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-'))
                .parse::<f64>()
                .ok()
        })
        .collect();
    if cleaned.contains('<') {
        numbers.first().map(|h| (0.0, Some(*h)))
    } else if cleaned.contains('>') {
        numbers.first().map(|l| (*l, None))
    } else {
        match numbers.as_slice() {
            [a, b, ..] => Some((*a, Some(*b))),
            [a] => Some((0.0, Some(*a))),
            _ => None,
        }
    }
}

/// Clinical flag for a numeric result. Non-numeric values are treated as normal.
pub fn lab_flag(value: &str, reference: &str) -> &'static str {
    let Ok(v) = value.trim().parse::<f64>() else {
        return "normal";
    };
    let Some((low, high)) = parse_ref(reference) else {
        return "normal";
    };
    if let Some(h) = high {
        if v > h * 1.35 {
            return "critical";
        }
        if v > h {
            return "abnormal";
        }
    }
    if v < low {
        if v < low * 0.6 {
            return "critical";
        }
        return "abnormal";
    }
    "normal"
}

pub fn csv_escape(field: &str) -> String {
    format!("\"{}\"", field.replace('"', "\"\""))
}

pub fn json_text(data: &Value) -> String {
    serde_json::to_string(data).unwrap_or_else(|_| "{}".to_string())
}

pub fn paginate(page: Option<i64>, per_page: Option<i64>) -> (i64, i64) {
    let per = per_page.unwrap_or(25).clamp(1, 200);
    let p = page.unwrap_or(1).max(1);
    (per, (p - 1) * per)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_reference_ranges() {
        assert_eq!(parse_ref("3.5-5.1"), Some((3.5, Some(5.1))));
        assert_eq!(parse_ref("70–100"), Some((70.0, Some(100.0))));
        assert_eq!(parse_ref("LDL < 130"), Some((0.0, Some(130.0))));
        assert_eq!(parse_ref("> 5"), Some((5.0, None)));
        assert_eq!(parse_ref("—"), None);
    }

    #[test]
    fn flags_out_of_range_results() {
        assert_eq!(lab_flag("4.0", "3.5-5.1"), "normal");
        assert_eq!(lab_flag("5.5", "3.5-5.1"), "abnormal");
        assert_eq!(lab_flag("6.9", "3.5-5.1"), "critical");
        assert_eq!(lab_flag("2.0", "3.5-5.1"), "critical");
        assert_eq!(lab_flag("130", "LDL < 130"), "normal");
        assert_eq!(lab_flag("در محدوده طبیعی", "3.5-5.1"), "normal");
    }

    #[test]
    fn normalises_and_validates_identifiers() {
        assert_eq!(normalize_phone("+98 912 345 6789"), "09123456789");
        assert_eq!(normalize_phone("۰۹۱۲۳۴۵۶۷۸۹"), "09123456789");
        assert!(valid_national_id("0013542419"));
        assert!(!valid_national_id("1234567890"));
    }
}
