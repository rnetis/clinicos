use rand::{Rng, RngCore};
use serde_json::json;
use sqlx::SqlitePool;

use crate::auth::hash_password;
use crate::config::Config;
use crate::db;
use crate::util;

pub async fn bootstrap(pool: &SqlitePool, cfg: &Config) -> Result<(), sqlx::Error> {
    reference(pool, cfg).await?;
    ensure_admin(pool, cfg).await?;
    if cfg.demo_seed {
        demo(pool, cfg).await?;
    }
    Ok(())
}

async fn reference(pool: &SqlitePool, cfg: &Config) -> Result<(), sqlx::Error> {
    for (key, value) in [
        ("clinic_name_fa", cfg.clinic_name_fa.as_str()),
        ("clinic_name_en", cfg.clinic_name_en.as_str()),
        ("clinic_phone", "021-88776655"),
        ("clinic_address_fa", "تهران، خیابان ولیعصر، پلاک ۱۲۳۴"),
        ("clinic_address_en", "No. 1234, Valiasr St, Tehran"),
        ("calendar", "jalali"),
        ("slot_minutes", "30"),
        ("work_start", "08:00"),
        ("work_end", "20:00"),
        ("discount_limit_pct", "10"),
    ] {
        sqlx::query("INSERT OR IGNORE INTO settings (key, value) VALUES (?, ?)")
            .bind(key)
            .bind(value)
            .execute(pool)
            .await?;
    }

    for (id, fa, en) in [
        ("dept-int", "داخلی", "Internal Medicine"),
        ("dept-card", "قلب و عروق", "Cardiology"),
        ("dept-peds", "اطفال", "Pediatrics"),
        ("dept-gyn", "زنان و زایمان", "Obstetrics & Gynecology"),
        ("dept-lab", "آزمایشگاه", "Laboratory"),
    ] {
        sqlx::query("INSERT OR IGNORE INTO departments (id, name_fa, name_en) VALUES (?,?,?)")
            .bind(id)
            .bind(fa)
            .bind(en)
            .execute(pool)
            .await?;
    }

    for (id, fa, en, cover) in [
        ("ins-ss", "تأمین اجتماعی", "Social Security", 70i64),
        ("ins-sal", "بیمه سلامت", "Salamat Health", 80),
        ("ins-af", "بیمه نیروهای مسلح", "Armed Forces", 60),
        ("ins-none", "بدون بیمه (خودپرداخت)", "Self-pay", 0),
    ] {
        sqlx::query("INSERT OR IGNORE INTO insurers (id, name_fa, name_en, cover) VALUES (?,?,?,?)")
            .bind(id)
            .bind(fa)
            .bind(en)
            .bind(cover)
            .execute(pool)
            .await?;
    }

    let services: [(&str, &str, &str, &str, &str, i64, i64); 8] = [
        ("svc-1", "VIS-SP", "ویزیت پزشک متخصص", "Specialist visit", "consult", 550_000, 30),
        ("svc-2", "VIS-PD", "ویزیت اطفال", "Pediatric visit", "consult", 450_000, 20),
        ("svc-3", "VIS-FU", "ویزیت پیگیری", "Follow-up visit", "consult", 300_000, 15),
        ("svc-4", "INJ-IM", "تزریق عضلانی", "IM injection", "procedure", 120_000, 10),
        ("svc-5", "ECG-12", "نوار قلب ۱۲ لایه", "12-lead ECG", "diagnostic", 350_000, 15),
        ("svc-6", "DRS-01", "پانسمان", "Dressing", "procedure", 180_000, 15),
        ("svc-7", "LAB-GEN", "خدمات آزمایشگاهی", "Laboratory services", "diagnostic", 600_000, 10),
        ("svc-8", "VAC-01", "واکسیناسیون", "Vaccination", "procedure", 250_000, 10),
    ];
    for (id, code, fa, en, cat, price, dur) in services {
        sqlx::query(
            "INSERT OR IGNORE INTO services (id, code, name_fa, name_en, category, price, duration)
             VALUES (?,?,?,?,?,?,?)",
        )
        .bind(id)
        .bind(code)
        .bind(fa)
        .bind(en)
        .bind(cat)
        .bind(price)
        .bind(dur)
        .execute(pool)
        .await?;
    }

    let tests: [(&str, &str, &str, &str, &str, &str, &str, i64, &str); 10] = [
        ("lt-cbc", "CBC", "شمارش کامل سلول خون", "Complete Blood Count (CBC)", "—", "—", "—", 250_000, "هماتولوژی/Hematology"),
        ("lt-fbs", "FBS", "قند خون ناشتا", "Fasting Blood Sugar", "mg/dL", "70-100", "70-100", 120_000, "بیوشیمی/Biochemistry"),
        ("lt-hba1c", "HbA1c", "هموگلوبین قندی", "Glycated Hemoglobin (HbA1c)", "%", "4.0-5.6", "4.0-5.6", 380_000, "بیوشیمی/Biochemistry"),
        ("lt-lipid", "LIPID", "پروفایل لیپیدی", "Lipid Panel (LDL)", "mg/dL", "LDL < 130", "LDL < 130", 450_000, "بیوشیمی/Biochemistry"),
        ("lt-tsh", "TSH", "هورمون محرک تیروئید", "Thyroid Stimulating Hormone", "mIU/L", "0.4-4.0", "0.4-4.0", 320_000, "هورمون/Hormones"),
        ("lt-k", "K", "پتاسیم خون", "Serum Potassium", "mmol/L", "3.5-5.1", "3.5-5.1", 110_000, "الکترولیت/Electrolytes"),
        ("lt-cr", "Cr", "کراتینین", "Serum Creatinine", "mg/dL", "0.7-1.3", "0.6-1.1", 110_000, "بیوشیمی/Biochemistry"),
        ("lt-ua", "UA", "آنالیز ادرار", "Urinalysis", "—", "—", "—", 180_000, "میکروسکوپی/Microscopy"),
        ("lt-vitd", "VitD", "ویتامین D", "Vitamin D (25-OH)", "ng/mL", "30-100", "30-100", 520_000, "بیوشیمی/Biochemistry"),
        ("lt-esr", "ESR", "سرعت رسوب گلبول", "Erythrocyte Sedimentation Rate", "mm/h", "< 20", "< 30", 90_000, "هماتولوژی/Hematology"),
    ];
    for (id, code, fa, en, unit, refm, reff, price, cat) in tests {
        sqlx::query(
            "INSERT OR IGNORE INTO lab_tests (id, code, name_fa, name_en, unit, ref_m, ref_f, price, cat)
             VALUES (?,?,?,?,?,?,?,?,?)",
        )
        .bind(id)
        .bind(code)
        .bind(fa)
        .bind(en)
        .bind(unit)
        .bind(refm)
        .bind(reff)
        .bind(price)
        .bind(cat)
        .execute(pool)
        .await?;
    }

    let meds: [(&str, &str, &str, &str, &str, &str, &str, &str, Option<&str>); 12] = [
        ("m1", "استامینوفن ۵۰۰", "Acetaminophen 500", "پاراستامول", "Paracetamol", "tablet", "PO", "TDS", None),
        ("m2", "آموکسی‌سیلین ۵۰۰", "Amoxicillin 500", "آموکسی‌سیلین", "Amoxicillin", "capsule", "PO", "TDS", Some("penicillin")),
        ("m3", "متفرمین ۵۰۰", "Metformin 500", "هیدروکلراید متفرمین", "Metformin HCl", "tablet", "PO", "BD", None),
        ("m4", "آتورواستاتین ۲۰", "Atorvastatin 20", "آتورواستاتین", "Atorvastatin", "tablet", "PO", "HS", None),
        ("m5", "لوزارتان ۵۰", "Losartan 50", "پتاسیم لوزارتان", "Losartan K", "tablet", "PO", "OD", None),
        ("m6", "اومپرازول ۲۰", "Omeprazole 20", "اومپرازول", "Omeprazole", "capsule", "PO", "OD", None),
        ("m7", "بروفن ۴۰۰", "Ibuprofen 400", "ایبوپروفن", "Ibuprofen", "tablet", "PO", "TDS", Some("nsaid")),
        ("m8", "آسپرین ۸۱", "Aspirin 81", "استیل‌سالیسیلیک اسید", "Acetylsalicylic acid", "tablet", "PO", "OD", Some("nsaid")),
        ("m9", "اسپری سالبوتامول", "Salbutamol inhaler", "سالبوتامول", "Salbutamol", "inhaler", "INH", "PRN", None),
        ("m10", "متوپرولول ۵۰", "Metoprolol 50", "تارتارات متوپرولول", "Metoprolol tartrate", "tablet", "PO", "OD", None),
        ("m11", "لووتیروکسین ۵۰", "Levothyroxine 50", "لووتیروکسین سدیم", "Levothyroxine Na", "tablet", "PO", "OD", None),
        ("m12", "سیپروفلوکساسین ۵۰۰", "Ciprofloxacin 500", "سیپروفلوکساسین", "Ciprofloxacin", "tablet", "PO", "BD", None),
    ];
    for (id, fa, en, gfa, gen, form, route, freq, tag) in meds {
        sqlx::query(
            "INSERT OR IGNORE INTO meds (id, name_fa, name_en, generic_fa, generic_en, forms, route, freq, allergen_tag)
             VALUES (?,?,?,?,?,?,?,?,?)",
        )
        .bind(id)
        .bind(fa)
        .bind(en)
        .bind(gfa)
        .bind(gen)
        .bind(format!("[\"{form}\"]"))
        .bind(route)
        .bind(freq)
        .bind(tag)
        .execute(pool)
        .await?;
    }

    let icd: [(&str, &str, &str); 22] = [
        ("E11.9", "دیابت نوع ۲ بدون عارضه", "Type 2 diabetes mellitus without complications"),
        ("I10", "فشار خون بالینی (اسنشیال)", "Essential (primary) hypertension"),
        ("I25.10", "بیماری عروق کرونر بدون آنژین", "Atherosclerotic heart disease without angina"),
        ("J06.9", "عفونت حاد دستگاه تنفس فوقانی", "Acute upper respiratory infection"),
        ("J02.9", "فارنژیت حاد نامشخص", "Acute pharyngitis, unspecified"),
        ("J45.909", "آسم، بدون کنترل کافی", "Asthma, unspecified, uncontrolled"),
        ("J18.9", "پنومونی، ارگانیسم نامشخص", "Pneumonia, unspecified organism"),
        ("K29.70", "گاستریت بدون خونریزی", "Gastritis without bleeding"),
        ("K21.9", "بیماری ریفلاکس معده-مری", "GERD without esophagitis"),
        ("E78.5", "هیپرلیپیدمی نامشخص", "Hyperlipidemia, unspecified"),
        ("E03.9", "هیپوتیروئیدی", "Hypothyroidism"),
        ("D64.9", "آنمی نامشخص", "Anemia, unspecified"),
        ("N39.0", "عفونت مجاری ادراری", "Urinary tract infection"),
        ("M54.5", "کمردرد", "Low back pain"),
        ("M17.9", "استئوآرتریت زانو", "Osteoarthritis of knee"),
        ("R51.9", "سردرد", "Headache"),
        ("A09", "گاستروانتریت عفونی", "Infectious gastroenteritis"),
        ("E55.9", "کمبود ویتامین D", "Vitamin D deficiency"),
        ("R07.9", "درد قفسه سینه", "Chest pain, unspecified"),
        ("Z00.0", "معاینه سلامت عمومی", "General health examination"),
        ("O09.529", "مراقبت از بارداری سالمند", "Supervision of elderly multigravida"),
        ("L23", "درماتیت تماسی آلرژیک", "Allergic contact dermatitis"),
    ];
    for (code, fa, en) in icd {
        sqlx::query("INSERT OR IGNORE INTO icd10 (code, title_fa, title_en) VALUES (?,?,?)")
            .bind(code)
            .bind(fa)
            .bind(en)
            .execute(pool)
            .await?;
    }
    Ok(())
}

fn random_password() -> String {
    let mut bytes = [0u8; 6];
    rand::thread_rng().fill_bytes(&mut bytes);
    let hex = hex::encode(bytes);
    let digits = rand::thread_rng().gen_range(1000..9999);
    format!("Clin-{hex}{digits}")
}

async fn ensure_admin(pool: &SqlitePool, cfg: &Config) -> Result<(), sqlx::Error> {
    let users: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(pool)
        .await?;
    if users > 0 {
        return Ok(());
    }
    let generated = cfg.admin_password.is_none();
    let password = cfg.admin_password.clone().unwrap_or_else(random_password);
    let id = uuid::Uuid::new_v4().to_string();
    let hash = hash_password(&password).map_err(|e| sqlx::Error::Protocol(e.to_string()))?;
    sqlx::query(
        "INSERT INTO users (id, username, password_hash, role, name_fa, name_en, active, created_at)
         VALUES (?,?,?, 'admin', ?, ?, 1, ?)",
    )
    .bind(&id)
    .bind(&cfg.admin_username)
    .bind(hash)
    .bind("مدیر سیستم")
    .bind("System Administrator")
    .bind(util::now())
    .execute(pool)
    .await?;

    if generated {
        println!("\n============================================================");
        println!(" First-run administrator created");
        println!("   username: {}", cfg.admin_username);
        println!("   password: {password}");
        println!(" Store this password now; it will not be shown again.");
        println!("============================================================\n");
    } else {
        tracing::info!(username = %cfg.admin_username, "administrator account created from environment");
    }
    Ok(())
}

async fn demo(pool: &SqlitePool, cfg: &Config) -> Result<(), sqlx::Error> {
    let patients: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM patients")
        .fetch_one(pool)
        .await?;
    if patients > 0 {
        return Ok(());
    }
    let t = util::today(&cfg.tz);
    let d = |n: i64| util::add_days(&t, n).unwrap_or_else(|_| t.clone());
    let now = util::now();
    let password = hash_password(&cfg.demo_password)
        .map_err(|e| sqlx::Error::Protocol(e.to_string()))?;

    // ---- doctors ----
    let doctors: [(&str, &str, &str, &str, &str, &str, &str, i64, i64, &str, &str); 4] = [
        ("doc-1", "دکتر آریا مرادی", "Dr. Aria Moradi", "قلب و عروق", "Cardiology", "C-12457", "dept-card", 800_000, 30, "[5]", "#b91c1c"),
        ("doc-2", "دکتر سارا احمدی", "Dr. Sara Ahmadi", "پزشکی داخلی", "Internal Medicine", "I-33210", "dept-int", 550_000, 30, "[5]", "#0e7490"),
        ("doc-3", "دکتر رضا کریمی", "Dr. Reza Karimi", "اطفال", "Pediatrics", "P-89002", "dept-peds", 450_000, 20, "[4,5]", "#15803d"),
        ("doc-4", "دکتر مریم حسینی", "Dr. Maryam Hosseini", "زنان و زایمان", "Obstetrics & Gynecology", "G-55120", "dept-gyn", 600_000, 30, "[5]", "#6d28d9"),
    ];
    for (id, fa, en, sfa, sen, license, dept, fee, slot, off, color) in doctors {
        sqlx::query(
            "INSERT OR IGNORE INTO doctors (id, name_fa, name_en, specialty_fa, specialty_en, license,
                dept_id, work_start, work_end, slot_min, fee, days_off, color, active)
             VALUES (?,?,?,?,?,?,?, '09:00', '17:00', ?,?,?,?,1)",
        )
        .bind(id)
        .bind(fa)
        .bind(en)
        .bind(sfa)
        .bind(sen)
        .bind(license)
        .bind(dept)
        .bind(slot)
        .bind(fee)
        .bind(off)
        .bind(color)
        .execute(pool)
        .await?;
    }

    // ---- staff accounts ----
    let staff: [(&str, &str, &str, &str, &str, Option<&str>); 6] = [
        ("u-rec", "reception", "receptionist", "لاله صادقی", "Laleh Sadeghi", None),
        ("u-doc1", "dr.moradi", "doctor", "دکتر آریا مرادی", "Dr. Aria Moradi", Some("doc-1")),
        ("u-doc2", "dr.ahmadi", "doctor", "دکتر سارا احمدی", "Dr. Sara Ahmadi", Some("doc-2")),
        ("u-nurse", "nurse", "nurse", "شیوا رحمانی", "Shiva Rahmani", None),
        ("u-lab", "lab", "lab", "بهنام قاسمی", "Behnam Ghasemi", None),
        ("u-fin", "finance", "accountant", "امیر تهرانی", "Amir Tehrani", None),
    ];
    for (id, username, role, fa, en, doctor_id) in staff {
        sqlx::query(
            "INSERT OR IGNORE INTO users (id, username, password_hash, role, name_fa, name_en, doctor_id, active, created_at)
             VALUES (?,?,?,?,?,?,?,1,?)",
        )
        .bind(id)
        .bind(username)
        .bind(&password)
        .bind(role)
        .bind(fa)
        .bind(en)
        .bind(doctor_id)
        .bind(d(-300))
        .execute(pool)
        .await?;
        if let Some(doc) = doctor_id {
            sqlx::query("UPDATE doctors SET user_id = ? WHERE id = ? AND user_id IS NULL")
                .bind(id)
                .bind(doc)
                .execute(pool)
                .await?;
        }
    }

    // ---- patients ----
    struct P<'a> {
        id: &'a str,
        code: &'a str,
        ffa: &'a str,
        fen: &'a str,
        lfa: &'a str,
        len: &'a str,
        dob: &'a str,
        gender: &'a str,
        nid: &'a str,
        phone: &'a str,
        insurer: &'a str,
        policy: &'a str,
        doctor: Option<&'a str>,
        blood: &'a str,
        category: &'a str,
        status: &'a str,
        source: &'a str,
        created: i64,
        alert: Option<(&'a str, &'a str)>,
    }
    let people = [
        P { id: "pat-1", code: "P-1001", ffa: "مریم", fen: "Maryam", lfa: "رضایی", len: "Rezaei", dob: "1985-03-12", gender: "F", nid: "0023847561", phone: "09123456789", insurer: "ins-ss", policy: "SS-77124560", doctor: Some("doc-2"), blood: "A+", category: "general", status: "active", source: "referral", created: -95, alert: Some(("کنترل فشار خون ماهانه", "Monthly BP control required")) },
        P { id: "pat-2", code: "P-1002", ffa: "علی", fen: "Ali", lfa: "محمدی", len: "Mohammadi", dob: "1990-07-30", gender: "M", nid: "0045217893", phone: "09351234567", insurer: "ins-sal", policy: "SL-33445512", doctor: Some("doc-2"), blood: "O-", category: "general", status: "active", source: "phone", created: -210, alert: None },
        P { id: "pat-3", code: "P-1003", ffa: "زهرا", fen: "Zahra", lfa: "کریمی", len: "Karimi", dob: "1996-11-05", gender: "F", nid: "0076512349", phone: "09199876543", insurer: "ins-ss", policy: "SS-99887766", doctor: Some("doc-2"), blood: "B+", category: "general", status: "active", source: "walk_in", created: -150, alert: None },
        P { id: "pat-4", code: "P-1004", ffa: "حسین", fen: "Hossein", lfa: "احمدی", len: "Ahmadi", dob: "1968-01-22", gender: "M", nid: "0001234567", phone: "09121110000", insurer: "ins-af", policy: "AF-12345098", doctor: Some("doc-1"), blood: "AB+", category: "senior", status: "active", source: "referral", created: -40, alert: Some(("سابقه درد قفسه سینه — ارزیابی قلبی فوری", "History of chest pain — urgent cardiac evaluation")) },
        P { id: "pat-5", code: "P-1005", ffa: "فاطمه", fen: "Fatemeh", lfa: "نوری", len: "Nouri", dob: "1995-09-17", gender: "F", nid: "0067812345", phone: "09101234567", insurer: "ins-sal", policy: "SL-55667788", doctor: Some("doc-2"), blood: "", category: "general", status: "active", source: "phone", created: -30, alert: None },
        P { id: "pat-6", code: "P-1006", ffa: "رضا", fen: "Reza", lfa: "موسوی", len: "Mousavi", dob: "1979-04-03", gender: "M", nid: "0024567891", phone: "09161234567", insurer: "ins-ss", policy: "SS-11223344", doctor: Some("doc-2"), blood: "", category: "general", status: "active", source: "walk_in", created: -60, alert: None },
        P { id: "pat-7", code: "P-1007", ffa: "نازنین", fen: "Nazanin", lfa: "شریفی", len: "Sharifi", dob: "2010-02-14", gender: "F", nid: "0098765432", phone: "09125554433", insurer: "ins-sal", policy: "SL-99881122", doctor: Some("doc-3"), blood: "", category: "child", status: "active", source: "walk_in", created: -25, alert: None },
        P { id: "pat-8", code: "P-1008", ffa: "محمود", fen: "Mahmoud", lfa: "اکبری", len: "Akbari", dob: "1955-12-08", gender: "M", nid: "0009876543", phone: "09124445566", insurer: "ins-none", policy: "", doctor: Some("doc-1"), blood: "", category: "senior", status: "pending_verify", source: "walk_in", created: -2, alert: None },
        P { id: "pat-9", code: "P-1009", ffa: "سکینه", fen: "Sakineh", lfa: "حسینی", len: "Hosseini", dob: "1972-06-25", gender: "F", nid: "0033467821", phone: "09123334455", insurer: "ins-ss", policy: "SS-33446677", doctor: Some("doc-4"), blood: "", category: "general", status: "active", source: "phone", created: -100, alert: None },
        P { id: "pat-10", code: "P-1010", ffa: "لیلا", fen: "Leila", lfa: "جعفری", len: "Jafari", dob: "1988-08-09", gender: "F", nid: "0077456321", phone: "09301239876", insurer: "ins-sal", policy: "SL-77665544", doctor: Some("doc-3"), blood: "", category: "general", status: "active", source: "portal", created: -80, alert: None },
    ];
    for p in &people {
        sqlx::query(
            "INSERT OR IGNORE INTO patients (id, code, first_name_fa, first_name_en, last_name_fa, last_name_en,
                dob, gender, national_id, phone, insurance_provider_id, policy_no, assigned_doctor_id,
                blood_group, category, status, source, created_at, created_by)
             VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?, 'u-rec')",
        )
        .bind(p.id)
        .bind(p.code)
        .bind(p.ffa)
        .bind(p.fen)
        .bind(p.lfa)
        .bind(p.len)
        .bind(p.dob)
        .bind(p.gender)
        .bind(p.nid)
        .bind(p.phone)
        .bind(p.insurer)
        .bind(p.policy)
        .bind(p.doctor)
        .bind(p.blood)
        .bind(p.category)
        .bind(p.status)
        .bind(p.source)
        .bind(format!("{}T09:00:00.000Z", d(p.created)))
        .execute(pool)
        .await?;
        if let Some((fa, en)) = p.alert {
            sqlx::query("INSERT INTO patient_alerts (id, patient_id, text_fa, text_en, created_at) VALUES (?,?,?,?,?)")
                .bind(uuid::Uuid::new_v4().to_string())
                .bind(p.id)
                .bind(fa)
                .bind(en)
                .bind(d(p.created))
                .execute(pool)
                .await?;
        }
    }

    async fn allergy(pool: &SqlitePool, patient: &str, tag: &str, fa: &str, en: &str, sev: &str, note: &str, at: &str) {
        let _ = sqlx::query(
            "INSERT INTO allergies (id, patient_id, tag, name_fa, name_en, severity, note, noted_at) VALUES (?,?,?,?,?,?,?,?)",
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(patient)
        .bind(tag)
        .bind(fa)
        .bind(en)
        .bind(sev)
        .bind(note)
        .bind(at)
        .execute(pool)
        .await;
    }
    allergy(pool, "pat-1", "nsaid", "بروفن (NSAID)", "Ibuprofen (NSAID)", "severe", "کهیر و تنگی نفس / Urticaria & dyspnea", &d(-95)).await;

    async fn condition(pool: &SqlitePool, patient: &str, code: &str, fa: &str, en: &str, chronic: bool, at: &str) {
        let _ = sqlx::query(
            "INSERT INTO conditions (id, patient_id, code, name_fa, name_en, chronic, noted_at) VALUES (?,?,?,?,?,?,?)",
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(patient)
        .bind(code)
        .bind(fa)
        .bind(en)
        .bind(chronic as i32)
        .bind(at)
        .execute(pool)
        .await;
    }
    condition(pool, "pat-1", "E11.9", "دیابت نوع ۲", "Type 2 Diabetes", true, &d(-95)).await;
    condition(pool, "pat-1", "I10", "فشار خون بالا", "Hypertension", true, &d(-20)).await;
    condition(pool, "pat-2", "J45.909", "آسم", "Asthma", true, &d(-210)).await;
    condition(pool, "pat-4", "I25.10", "بیماری کرونر قلب", "Coronary artery disease", true, &d(-7)).await;
    condition(pool, "pat-4", "I10", "فشار خون بالا", "Hypertension", true, &d(-7)).await;
    condition(pool, "pat-4", "E78.5", "هیپرلیپیدمی", "Hyperlipidemia", true, &d(-7)).await;
    condition(pool, "pat-6", "I10", "فشار خون بالا", "Hypertension", true, &d(-60)).await;

    // ---- appointments ----
    let appts: [(&str, &str, &str, i64, &str, &str, &str, &str, &str); 8] = [
        ("apt-1", "pat-3", "doc-2", 0, "09:00", "completed", "first_visit", "گلودرد و تب", "Sore throat & fever"),
        ("apt-2", "pat-2", "doc-1", 0, "09:10", "in_progress", "emergency", "درد قفسه سینه", "Chest pain"),
        ("apt-3", "pat-5", "doc-2", 0, "10:30", "checked_in", "follow_up", "کنترل آسم", "Asthma review"),
        ("apt-4", "pat-6", "doc-2", 0, "11:00", "confirmed", "consultation", "کنترل فشار خون", "BP review"),
        ("apt-5", "pat-4", "doc-1", 1, "09:30", "confirmed", "follow_up", "پیگیری کرونری قلب", "CAD follow-up"),
        ("apt-6", "pat-1", "doc-2", 3, "10:00", "requested", "follow_up", "کنترل فشار خون و قند", "BP & glucose review"),
        ("apt-7", "pat-7", "doc-3", -1, "09:30", "completed", "first_visit", "سرماخوردگی کودک", "Child cold symptoms"),
        ("apt-8", "pat-9", "doc-4", 2, "12:00", "confirmed", "consultation", "مشاوره زنان", "Gyn consultation"),
    ];
    for (id, pid, doc, offset, time, status, kind, rfa, ren) in appts {
        let date = d(offset);
        let dept: Option<String> = sqlx::query_scalar("SELECT dept_id FROM doctors WHERE id = ?")
            .bind(doc)
            .fetch_optional(pool)
            .await?
            .flatten();
        sqlx::query(
            "INSERT OR IGNORE INTO appointments (id, patient_id, doctor_id, dept_id, date, time, duration,
                type, status, reason_fa, reason_en, source, created_by, created_at, status_changed_at)
             VALUES (?,?,?,?,?,?,30,?,?,?,?, 'phone', 'u-rec', ?, ?)",
        )
        .bind(id)
        .bind(pid)
        .bind(doc)
        .bind(dept)
        .bind(&date)
        .bind(time)
        .bind(kind)
        .bind(status)
        .bind(rfa)
        .bind(ren)
        .bind(&now)
        .bind(&now)
        .execute(pool)
        .await?;
    }

    // ---- queue ----
    for (id, pid, doc, number, status, priority, minutes) in [
        ("q-1", "pat-3", "doc-2", 1i64, "done", 0i64, 110i64),
        ("q-2", "pat-2", "doc-1", 1, "in_progress", 1, 65),
        ("q-3", "pat-5", "doc-2", 2, "waiting", 0, 22),
    ] {
        let appt = sqlx::query_scalar::<_, String>(
            "SELECT id FROM appointments WHERE patient_id = ? AND doctor_id = ? AND date = ? LIMIT 1",
        )
        .bind(pid)
        .bind(doc)
        .bind(&t)
        .fetch_optional(pool)
        .await?;
        sqlx::query(
            "INSERT OR IGNORE INTO queue (id, patient_id, appointment_id, doctor_id, number, date,
                priority, status, checked_in_at, called_at, started_at, done_at, created_by)
             VALUES (?,?,?,?,?,?,?,?,?,?,?,?, 'u-rec')",
        )
        .bind(id)
        .bind(pid)
        .bind(appt)
        .bind(doc)
        .bind(number)
        .bind(&t)
        .bind(priority)
        .bind(status)
        .bind(ago(minutes))
        .bind(if status != "waiting" { Some(ago(minutes - 5)) } else { None })
        .bind(if matches!(status, "in_progress" | "done") { Some(ago(minutes - 8)) } else { None })
        .bind(if status == "done" { Some(ago(minutes - 46)) } else { None })
        .execute(pool)
        .await?;
    }

    // ---- signed encounters ----
    let enc5 = json!({
        "chief": {"fa":"تشنگی زیاد و تکرر ادرار","en":"Excessive thirst and polyuria"},
        "hpi": {"fa":"شروع تدریجی طی چند ماه. کاهش وزن ۴ کیلوگرم. سابقه خانوادگی دیابت.","en":"Gradual over months. 4 kg weight loss. Family history of diabetes."},
        "exam": {"fa":"طبیعی","en":"Normal"},
        "vitals": {"bp":"132/84","hr":80,"rr":16,"temp":36.7,"spo2":98,"height":163,"weight":82,"glucose":210},
        "plan": {"fa":"رژیم دیابت، کنترل قند خانگی، آموزش بیمار","en":"Diabetic diet, home glucose monitoring, patient education"},
        "diagnoses": [{"code":"E11.9","type":"primary","note":{"fa":"تشخیص جدید بر اساس FBS و HbA1c","en":"New diagnosis per FBS & HbA1c"}}],
        "prescriptions": [{"medId":"m3","dose":"500 mg","form":"tablet","route":"PO","freq":"BD","durDays":90,"qty":180,"instructions":{"fa":"همراه غذا","en":"With meals"}}],
        "labTests": ["lt-hba1c"], "labPriority": "routine",
        "followUp": {"due": d(-65), "reason":{"fa":"بررسی HbA1c","en":"HbA1c review"},"priority":"routine","testsBefore":{"fa":"HbA1c","en":"HbA1c"}},
        "referral": {"fa":"","en":""}, "notes": ""
    });
    let enc1 = json!({
        "chief": {"fa":"سردرد و سرگیجه؛ فشار خون بالا در اندازه‌گیری خانگی","en":"Headache and dizziness; high BP at home"},
        "hpi": {"fa":"فشار خون از دو ماه قبل بالای ۱۴۰/۹۰ گزارش شده. بدون درد قفسه سینه. مصرف نمک زیاد.","en":"BP >140/90 for two months. No chest pain. High salt intake."},
        "exam": {"fa":"بدون ادیم، صداهای ریوی طبیعی","en":"No edema, clear lung sounds"},
        "vitals": {"bp":"150/95","hr":84,"rr":16,"temp":36.8,"spo2":97,"height":163,"weight":78,"glucose":110},
        "plan": {"fa":"کاهش نمک، پیاده‌روی روزانه ۳۰ دقیقه","en":"Reduce salt, 30 min daily walking"},
        "diagnoses": [
            {"code":"I10","type":"primary","note":{"fa":"سنجش مجدد در دو مراجعه","en":"Confirmed on two visits"}},
            {"code":"R51.9","type":"secondary","note":{"fa":"مرتبط با فشار خون","en":"Likely BP-related"}}
        ],
        "prescriptions": [{"medId":"m5","dose":"50 mg","form":"tablet","route":"PO","freq":"OD","durDays":30,"qty":30,"instructions":{"fa":"صبح‌ها با آب","en":"Morning with water"}}],
        "labTests": ["lt-cbc","lt-lipid"], "labPriority": "routine",
        "followUp": {"due": d(-6), "reason":{"fa":"کنترل فشار خون","en":"BP control review"},"priority":"high","testsBefore":{"fa":"CBC، پروفایل لیپیدی","en":"CBC, lipid panel"}},
        "referral": {"fa":"","en":""}, "notes": ""
    });
    let enc6 = json!({
        "chief": {"fa":"درد قفسه سینه هنگام فعالیت","en":"Exertional chest pain"},
        "hpi": {"fa":"درد پس از پیاده‌روی سریع، با استراحت بهتر می‌شود.","en":"Pain after brisk walking, relieved by rest."},
        "exam": {"fa":"صداهای قلبی منظم، بدون مورمور. ریه سالم.","en":"Regular heart sounds, no murmur. Clear lungs."},
        "vitals": {"bp":"148/90","hr":78,"rr":17,"temp":36.6,"spo2":96,"height":176,"weight":90,"glucose":105},
        "plan": {"fa":"ECG و تروپونین، کنترل فشار، اصلاح رژیم","en":"ECG and troponin, BP control, diet modification"},
        "diagnoses": [
            {"code":"I25.10","type":"primary","note":{"fa":"آزمون خطر محاسبه شود","en":"Risk assessment advised"}},
            {"code":"I10","type":"secondary","note":{"fa":"","en":""}},
            {"code":"E78.5","type":"secondary","note":{"fa":"LDL بالا در آزمایش قبلی","en":"High LDL on labs"}}
        ],
        "prescriptions": [
            {"medId":"m8","dose":"81 mg","form":"tablet","route":"PO","freq":"OD","durDays":90,"qty":90,"instructions":{"fa":"بعد از شام","en":"After dinner"}},
            {"medId":"m4","dose":"20 mg","form":"tablet","route":"PO","freq":"HS","durDays":90,"qty":90,"instructions":{"fa":"","en":""}}
        ],
        "labTests": ["lt-k","lt-cr"], "labPriority": "urgent",
        "followUp": {"due": d(30), "reason":{"fa":"پیگیری کرونری","en":"CAD follow-up"},"priority":"routine","testsBefore":{"fa":"پروفایل لیپیدی","en":"Lipid panel"}},
        "referral": {"fa":"مشاوره قلب در صورت مثبت بودن تست استرس","en":"Cardiology referral if stress test positive"}, "notes": ""
    });
    let draft = json!({
        "chief": {"fa":"درد قفسه سینه فشارنده از صبح، بدون انتشار","en":"Pressure-like chest pain since morning, no radiation"},
        "hpi": {"fa":"شروع ناگهانی ۴ ساعت قبل. عرق سرد. پدرش در ۵۰ سالگی MI.","en":"Sudden onset 4h ago. Diaphoresis. Father had MI at 50."},
        "exam": {"fa":"","en":""},
        "vitals": {"bp":"145/92","hr":102,"rr":20,"temp":37,"spo2":95,"height":180,"weight":95,"glucose":130},
        "plan": {"fa":"","en":""},
        "diagnoses": [], "prescriptions": [], "labTests": ["lt-k","lt-cr"], "labPriority": "stat",
        "followUp": null, "referral": {"fa":"","en":""}, "notes": ""
    });

    for (id, pid, doc, offset, data, status, queue_id, appt_id) in [
        ("enc-5", "pat-1", "doc-2", -95i64, enc5, "signed", None, None),
        ("enc-1", "pat-1", "doc-2", -20, enc1, "signed", None, None),
        ("enc-6", "pat-4", "doc-1", -7, enc6, "signed", None, None),
        ("enc-4", "pat-2", "doc-1", 0, draft, "draft", Some("q-2"), Some("apt-2")),
    ] {
        let version = if status == "signed" { 1i64 } else { 0 };
        sqlx::query(
            "INSERT OR IGNORE INTO encounters (id, patient_id, doctor_id, appointment_id, queue_entry_id,
                date, status, version, entered_by, signed_at, signed_by, created_at, updated_at)
             VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?)",
        )
        .bind(id)
        .bind(pid)
        .bind(doc)
        .bind(appt_id)
        .bind(queue_id)
        .bind(d(offset))
        .bind(status)
        .bind(version)
        .bind(if doc == "doc-1" { "u-doc1" } else { "u-doc2" })
        .bind(if status == "signed" { Some(format!("{}T10:30:00.000Z", d(offset))) } else { None })
        .bind(if status == "signed" { Some(if doc == "doc-1" { "u-doc1" } else { "u-doc2" }) } else { None })
        .bind(&now)
        .bind(&now)
        .execute(pool)
        .await?;
        sqlx::query(
            "INSERT OR IGNORE INTO encounter_versions (id, encounter_id, version, at, by_user, reason, data)
             VALUES (?,?,?,?,?, '', ?)",
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(id)
        .bind(version)
        .bind(format!("{}T10:30:00.000Z", d(offset)))
        .bind(if doc == "doc-1" { "u-doc1" } else { "u-doc2" })
        .bind(data.to_string())
        .execute(pool)
        .await?;
    }

    // ---- lab orders with results ----
    let labs: [(&str, &str, &str, &str, i64, &str, &str, Vec<(&str, &str, &str, &str)>); 3] = [
        ("lab-1", "pat-1", "enc-5", "doc-2", -95, "reviewed", "routine", vec![("lt-hba1c", "8.9", "%", "abnormal")]),
        ("lab-2", "pat-1", "enc-1", "doc-2", -20, "in_progress", "routine", vec![("lt-cbc", "در محدوده طبیعی", "—", "normal"), ("lt-lipid", "165", "mg/dL", "abnormal")]),
        ("lab-3", "pat-4", "enc-6", "doc-1", 0, "completed", "urgent", vec![("lt-k", "6.2", "mmol/L", "critical"), ("lt-cr", "1.4", "mg/dL", "abnormal")]),
    ];
    for (id, pid, enc, doc, offset, status, priority, results) in labs {
        sqlx::query(
            "INSERT OR IGNORE INTO lab_orders (id, patient_id, encounter_id, doctor_id, date, status, priority, created_at)
             VALUES (?,?,?,?,?,?,?,?)",
        )
        .bind(id)
        .bind(pid)
        .bind(enc)
        .bind(doc)
        .bind(d(offset))
        .bind(status)
        .bind(priority)
        .bind(format!("{}T09:00:00.000Z", d(offset)))
        .execute(pool)
        .await?;
        for (test, value, unit, flag) in results {
            sqlx::query("INSERT OR IGNORE INTO lab_order_tests (order_id, test_id) VALUES (?, ?)")
                .bind(id)
                .bind(test)
                .execute(pool)
                .await?;
            let reference: String = sqlx::query_scalar("SELECT ref_m FROM lab_tests WHERE id = ?")
                .bind(test)
                .fetch_one(pool)
                .await?;
            sqlx::query(
                "INSERT INTO lab_results (id, order_id, test_id, value, unit, ref, flag, at, entered_by, reviewed_by, reviewed_at)
                 VALUES (?,?,?,?,?,?,?,?, 'u-lab', ?, ?)",
            )
            .bind(uuid::Uuid::new_v4().to_string())
            .bind(id)
            .bind(test)
            .bind(value)
            .bind(unit)
            .bind(&reference)
            .bind(flag)
            .bind(ago(45))
            .bind(if status == "reviewed" { Some("u-doc2") } else { None })
            .bind(if status == "reviewed" { Some(ago(30)) } else { None })
            .execute(pool)
            .await?;
        }
    }

    // ---- invoices ----
    struct Inv<'a> {
        id: &'a str,
        no: &'a str,
        pid: &'a str,
        enc: Option<&'a str>,
        doc: &'a str,
        offset: i64,
        status: &'a str,
        items: Vec<(&'a str, &'a str, i64)>,
        cover: i64,
        insurer: &'a str,
        payment: Option<(i64, &'a str, &'a str)>,
    }
    let invoices = [
        Inv { id: "inv-1", no: "INV-1001", pid: "pat-3", enc: None, doc: "doc-2", offset: 0, status: "paid", items: vec![("ویزیت پزشک متخصص", "Specialist visit", 550_000)], cover: 70, insurer: "ins-ss", payment: Some((165_000, "cash", "")) },
        Inv { id: "inv-2", no: "INV-1002", pid: "pat-7", enc: None, doc: "doc-3", offset: -1, status: "paid", items: vec![("ویزیت اطفال", "Pediatric visit", 450_000)], cover: 80, insurer: "ins-sal", payment: Some((90_000, "card", "POS-8871")) },
        Inv { id: "inv-3", no: "INV-1003", pid: "pat-4", enc: Some("enc-6"), doc: "doc-1", offset: -7, status: "unpaid", items: vec![("ویزیت پزشک متخصص", "Specialist visit", 550_000), ("نوار قلب ۱۲ لایه", "12-lead ECG", 350_000)], cover: 60, insurer: "ins-af", payment: None },
        Inv { id: "inv-4", no: "INV-1004", pid: "pat-1", enc: Some("enc-1"), doc: "doc-2", offset: -20, status: "partial", items: vec![("ویزیت پزشک متخصص", "Specialist visit", 550_000), ("خدمات آزمایشگاهی", "Laboratory services", 600_000)], cover: 70, insurer: "ins-ss", payment: Some((200_000, "card", "POS-5512")) },
    ];
    for inv in invoices {
        sqlx::query(
            "INSERT OR IGNORE INTO invoices (id, no, patient_id, encounter_id, doctor_id, date, status,
                discount, cover, insurer_id, created_by, created_at)
             VALUES (?,?,?,?,?,?,?,0,?,?, 'u-rec', ?)",
        )
        .bind(inv.id)
        .bind(inv.no)
        .bind(inv.pid)
        .bind(inv.enc)
        .bind(inv.doc)
        .bind(d(inv.offset))
        .bind(inv.status)
        .bind(inv.cover)
        .bind(inv.insurer)
        .bind(format!("{}T12:00:00.000Z", d(inv.offset)))
        .execute(pool)
        .await?;
        for (fa, en, price) in &inv.items {
            sqlx::query(
                "INSERT INTO invoice_items (id, invoice_id, desc_fa, desc_en, qty, price) VALUES (?,?,?,?,1,?)",
            )
            .bind(uuid::Uuid::new_v4().to_string())
            .bind(inv.id)
            .bind(fa)
            .bind(en)
            .bind(price)
            .execute(pool)
            .await?;
        }
        if let Some((amount, method, reference)) = inv.payment {
            sqlx::query(
                "INSERT INTO payments (id, invoice_id, amount, method, ref, at, by_user, kind)
                 VALUES (?,?,?,?,?,?, 'u-rec', 'payment')",
            )
            .bind(uuid::Uuid::new_v4().to_string())
            .bind(inv.id)
            .bind(amount)
            .bind(method)
            .bind(reference)
            .bind(format!("{}T12:30:00.000Z", d(inv.offset)))
            .execute(pool)
            .await?;
        }
    }

    // ---- follow-ups, vitals, notifications ----
    for (id, pid, doc, due, rfa, ren, priority, status) in [
        ("fu-1", "pat-1", "doc-2", -6i64, "کنترل فشار خون", "BP control review", "high", "pending"),
        ("fu-2", "pat-4", "doc-1", 30, "پیگیری کرونری قلب", "CAD follow-up", "routine", "scheduled"),
        ("fu-3", "pat-2", "doc-1", 7, "ارزیابی مجدد درد قفسه سینه", "Chest pain reassessment", "urgent", "pending"),
    ] {
        sqlx::query(
            "INSERT OR IGNORE INTO follow_ups (id, patient_id, doctor_id, due, reason_fa, reason_en,
                priority, status, created_at, updated_at) VALUES (?,?,?,?,?,?,?,?,?,?)",
        )
        .bind(id)
        .bind(pid)
        .bind(doc)
        .bind(d(due))
        .bind(rfa)
        .bind(ren)
        .bind(priority)
        .bind(status)
        .bind(&now)
        .bind(&now)
        .execute(pool)
        .await?;
    }

    sqlx::query(
        "INSERT INTO vitals (id, patient_id, queue_entry_id, at, by_user, data) VALUES (?,?,?,?,?,?)",
    )
    .bind(uuid::Uuid::new_v4().to_string())
    .bind("pat-5")
    .bind("q-3")
    .bind(ago(20))
    .bind("u-nurse")
    .bind(json!({"bp":"126/84","hr":88,"rr":17,"temp":37.1,"spo2":96,"height":168,"weight":60,"glucose":null,"note":""}).to_string())
    .execute(pool)
    .await?;

    for (crit, user, fa, en, link) in [
        (true, "u-doc1", "⚠ نتیجه بحرانی: پتاسیم ۶.۲ برای حسین احمدی — نیاز به تأیید فوری", "⚠ Critical result: K 6.2 for Hossein Ahmadi — immediate review required", "#/doctor/labs"),
        (false, "u-doc2", "پیگیری سررسید گذشته: مریم رضایی — کنترل فشار خون", "Overdue follow-up: Maryam Rezaei — BP control", "#/doctor/followups"),
        (false, "u-doc2", "بیمار فاطمه نوری در صف انتظار شماست (نوبت ۱۰:۳۰)", "Fatemeh Nouri is waiting in your queue (10:30)", "#/doctor/waiting"),
    ] {
        sqlx::query(
            "INSERT INTO notifications (id, at, to_user, type, crit, read, text_fa, text_en, link)
             VALUES (?,?,?,?,?,0,?,?,?)",
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(ago(30))
        .bind(user)
        .bind("system")
        .bind(crit as i32)
        .bind(fa)
        .bind(en)
        .bind(link)
        .execute(pool)
        .await?;
    }

    db::audit(
        pool,
        None,
        "system",
        "system",
        "demo_seed",
        "system",
        "",
        &format!("{} patients", people.len()),
        "success",
        "",
    )
    .await;

    if cfg.demo_seed {
        println!("Demo dataset seeded. Staff accounts use password: {}", cfg.demo_password);
    }
    Ok(())
}

fn ago(minutes: i64) -> String {
    (chrono::Utc::now() - chrono::Duration::minutes(minutes.max(0)))
        .to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

