-- ClinicOS initial schema. Dates are TEXT 'YYYY-MM-DD', times 'HH:MM', timestamps RFC3339 UTC.
-- Money is stored as INTEGER minor-unit-free IRR.

CREATE TABLE settings (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
);

CREATE TABLE departments (
  id      TEXT PRIMARY KEY,
  name_fa TEXT NOT NULL,
  name_en TEXT NOT NULL DEFAULT '',
  active  INTEGER NOT NULL DEFAULT 1
);

CREATE TABLE insurers (
  id      TEXT PRIMARY KEY,
  name_fa TEXT NOT NULL,
  name_en TEXT NOT NULL DEFAULT '',
  cover   INTEGER NOT NULL DEFAULT 0 CHECK (cover BETWEEN 0 AND 100),
  active  INTEGER NOT NULL DEFAULT 1
);

CREATE TABLE users (
  id                   TEXT PRIMARY KEY,
  username             TEXT NOT NULL UNIQUE COLLATE NOCASE,
  password_hash        TEXT NOT NULL,
  role                 TEXT NOT NULL CHECK (role IN ('admin','receptionist','doctor','nurse','lab','accountant')),
  name_fa              TEXT NOT NULL,
  name_en              TEXT NOT NULL DEFAULT '',
  doctor_id            TEXT,
  active               INTEGER NOT NULL DEFAULT 1,
  created_at           TEXT NOT NULL,
  last_login_at        TEXT,
  failed_attempts      INTEGER NOT NULL DEFAULT 0,
  locked_until         TEXT,
  must_change_password INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE doctors (
  id            TEXT PRIMARY KEY,
  name_fa       TEXT NOT NULL,
  name_en       TEXT NOT NULL DEFAULT '',
  specialty_fa  TEXT NOT NULL DEFAULT '',
  specialty_en  TEXT NOT NULL DEFAULT '',
  license       TEXT NOT NULL DEFAULT '',
  dept_id       TEXT REFERENCES departments(id) ON DELETE SET NULL,
  work_start    TEXT NOT NULL DEFAULT '09:00',
  work_end      TEXT NOT NULL DEFAULT '17:00',
  slot_min      INTEGER NOT NULL DEFAULT 30,
  fee           INTEGER NOT NULL DEFAULT 0,
  days_off      TEXT NOT NULL DEFAULT '[]',
  color         TEXT NOT NULL DEFAULT '#0e7490',
  active        INTEGER NOT NULL DEFAULT 1,
  user_id       TEXT REFERENCES users(id) ON DELETE SET NULL
);

CREATE TABLE sessions (
  token_hash TEXT PRIMARY KEY,
  user_id    TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  created_at TEXT NOT NULL,
  expires_at TEXT NOT NULL,
  ip         TEXT NOT NULL DEFAULT '',
  user_agent TEXT NOT NULL DEFAULT ''
);
CREATE INDEX idx_sessions_user ON sessions(user_id);
CREATE INDEX idx_sessions_expires ON sessions(expires_at);

CREATE TABLE patients (
  id                    TEXT PRIMARY KEY,
  code                  TEXT NOT NULL UNIQUE,
  first_name_fa         TEXT NOT NULL,
  first_name_en         TEXT NOT NULL DEFAULT '',
  last_name_fa          TEXT NOT NULL,
  last_name_en          TEXT NOT NULL DEFAULT '',
  father_name           TEXT NOT NULL DEFAULT '',
  dob                   TEXT NOT NULL DEFAULT '',
  gender                TEXT NOT NULL DEFAULT '' CHECK (gender IN ('','M','F')),
  national_id           TEXT NOT NULL DEFAULT '',
  passport              TEXT NOT NULL DEFAULT '',
  phone                 TEXT NOT NULL DEFAULT '',
  phone2                TEXT NOT NULL DEFAULT '',
  email                 TEXT NOT NULL DEFAULT '',
  address_fa            TEXT NOT NULL DEFAULT '',
  address_en            TEXT NOT NULL DEFAULT '',
  city_fa               TEXT NOT NULL DEFAULT 'تهران',
  city_en               TEXT NOT NULL DEFAULT 'Tehran',
  postal                TEXT NOT NULL DEFAULT '',
  emergency_name        TEXT NOT NULL DEFAULT '',
  emergency_rel         TEXT NOT NULL DEFAULT '',
  emergency_phone       TEXT NOT NULL DEFAULT '',
  insurance_provider_id TEXT REFERENCES insurers(id) ON DELETE SET NULL,
  policy_no             TEXT NOT NULL DEFAULT '',
  category              TEXT NOT NULL DEFAULT 'general',
  status                TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active','inactive','archived','pending_verify')),
  source                TEXT NOT NULL DEFAULT 'walk_in',
  assigned_doctor_id    TEXT REFERENCES doctors(id) ON DELETE SET NULL,
  blood_group           TEXT NOT NULL DEFAULT '',
  notes                 TEXT NOT NULL DEFAULT '',
  history_surgical      TEXT NOT NULL DEFAULT '',
  history_family        TEXT NOT NULL DEFAULT '',
  history_lifestyle     TEXT NOT NULL DEFAULT '',
  merged_into           TEXT REFERENCES patients(id) ON DELETE SET NULL,
  created_at            TEXT NOT NULL,
  created_by            TEXT REFERENCES users(id) ON DELETE SET NULL
);
CREATE INDEX idx_patients_name ON patients(last_name_fa, first_name_fa);
CREATE INDEX idx_patients_phone ON patients(phone);
CREATE INDEX idx_patients_nid ON patients(national_id);
CREATE INDEX idx_patients_doctor ON patients(assigned_doctor_id);

CREATE TABLE patient_alerts (
  id         TEXT PRIMARY KEY,
  patient_id TEXT NOT NULL REFERENCES patients(id) ON DELETE CASCADE,
  text_fa    TEXT NOT NULL,
  text_en    TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL
);

CREATE TABLE allergies (
  id         TEXT PRIMARY KEY,
  patient_id TEXT NOT NULL REFERENCES patients(id) ON DELETE CASCADE,
  tag        TEXT NOT NULL DEFAULT '',
  name_fa    TEXT NOT NULL,
  name_en    TEXT NOT NULL DEFAULT '',
  severity   TEXT NOT NULL DEFAULT 'moderate' CHECK (severity IN ('mild','moderate','severe')),
  note       TEXT NOT NULL DEFAULT '',
  noted_at   TEXT NOT NULL
);

CREATE TABLE conditions (
  id         TEXT PRIMARY KEY,
  patient_id TEXT NOT NULL REFERENCES patients(id) ON DELETE CASCADE,
  code       TEXT NOT NULL DEFAULT '',
  name_fa    TEXT NOT NULL,
  name_en    TEXT NOT NULL DEFAULT '',
  chronic    INTEGER NOT NULL DEFAULT 0,
  noted_at   TEXT NOT NULL
);

CREATE TABLE patient_meds (
  id         TEXT PRIMARY KEY,
  patient_id TEXT NOT NULL REFERENCES patients(id) ON DELETE CASCADE,
  text_fa    TEXT NOT NULL,
  text_en    TEXT NOT NULL DEFAULT ''
);

CREATE TABLE services (
  id       TEXT PRIMARY KEY,
  code     TEXT NOT NULL UNIQUE,
  name_fa  TEXT NOT NULL,
  name_en  TEXT NOT NULL DEFAULT '',
  category TEXT NOT NULL DEFAULT 'consult',
  price    INTEGER NOT NULL DEFAULT 0 CHECK (price >= 0),
  duration INTEGER NOT NULL DEFAULT 30,
  dept_id  TEXT REFERENCES departments(id) ON DELETE SET NULL,
  active   INTEGER NOT NULL DEFAULT 1
);

CREATE TABLE lab_tests (
  id      TEXT PRIMARY KEY,
  code    TEXT NOT NULL UNIQUE,
  name_fa TEXT NOT NULL,
  name_en TEXT NOT NULL DEFAULT '',
  unit    TEXT NOT NULL DEFAULT '',
  ref_m   TEXT NOT NULL DEFAULT '',
  ref_f   TEXT NOT NULL DEFAULT '',
  price   INTEGER NOT NULL DEFAULT 0,
  cat     TEXT NOT NULL DEFAULT ''
);

CREATE TABLE meds (
  id           TEXT PRIMARY KEY,
  name_fa      TEXT NOT NULL,
  name_en      TEXT NOT NULL DEFAULT '',
  generic_fa   TEXT NOT NULL DEFAULT '',
  generic_en   TEXT NOT NULL DEFAULT '',
  forms        TEXT NOT NULL DEFAULT '["tablet"]',
  route        TEXT NOT NULL DEFAULT 'PO',
  freq         TEXT NOT NULL DEFAULT 'OD',
  allergen_tag TEXT
);

CREATE TABLE icd10 (
  code     TEXT PRIMARY KEY,
  title_fa TEXT NOT NULL,
  title_en TEXT NOT NULL DEFAULT ''
);

CREATE TABLE appointments (
  id                TEXT PRIMARY KEY,
  patient_id        TEXT NOT NULL REFERENCES patients(id) ON DELETE RESTRICT,
  doctor_id         TEXT NOT NULL REFERENCES doctors(id) ON DELETE RESTRICT,
  dept_id           TEXT REFERENCES departments(id) ON DELETE SET NULL,
  date              TEXT NOT NULL,
  time              TEXT NOT NULL,
  duration          INTEGER NOT NULL DEFAULT 30,
  type              TEXT NOT NULL DEFAULT 'consultation',
  status            TEXT NOT NULL DEFAULT 'requested'
                    CHECK (status IN ('requested','confirmed','arrived','checked_in','waiting','in_progress','completed','cancelled','no_show','rescheduled')),
  reason_fa         TEXT NOT NULL DEFAULT '',
  reason_en         TEXT NOT NULL DEFAULT '',
  source            TEXT NOT NULL DEFAULT 'walk_in',
  notes             TEXT NOT NULL DEFAULT '',
  created_by        TEXT REFERENCES users(id) ON DELETE SET NULL,
  created_at        TEXT NOT NULL,
  status_changed_at TEXT NOT NULL
);
CREATE INDEX idx_appt_date ON appointments(date, time);
CREATE INDEX idx_appt_doctor_date ON appointments(doctor_id, date);
CREATE INDEX idx_appt_patient ON appointments(patient_id);

CREATE TABLE queue (
  id             TEXT PRIMARY KEY,
  patient_id     TEXT NOT NULL REFERENCES patients(id) ON DELETE RESTRICT,
  appointment_id TEXT REFERENCES appointments(id) ON DELETE SET NULL,
  doctor_id      TEXT NOT NULL REFERENCES doctors(id) ON DELETE RESTRICT,
  number         INTEGER NOT NULL,
  date           TEXT NOT NULL,
  priority       INTEGER NOT NULL DEFAULT 0,
  status         TEXT NOT NULL DEFAULT 'waiting'
                 CHECK (status IN ('waiting','called','in_progress','done','cancelled')),
  checked_in_at  TEXT NOT NULL,
  called_at      TEXT,
  started_at     TEXT,
  done_at        TEXT,
  created_by     TEXT REFERENCES users(id) ON DELETE SET NULL
);
CREATE INDEX idx_queue_date_doctor ON queue(date, doctor_id);

CREATE TABLE encounters (
  id             TEXT PRIMARY KEY,
  patient_id     TEXT NOT NULL REFERENCES patients(id) ON DELETE RESTRICT,
  doctor_id      TEXT NOT NULL REFERENCES doctors(id) ON DELETE RESTRICT,
  appointment_id TEXT REFERENCES appointments(id) ON DELETE SET NULL,
  queue_entry_id TEXT REFERENCES queue(id) ON DELETE SET NULL,
  date           TEXT NOT NULL,
  status         TEXT NOT NULL DEFAULT 'draft'
                 CHECK (status IN ('draft','signed','amended','entered_in_error')),
  version        INTEGER NOT NULL DEFAULT 0,
  entered_by     TEXT REFERENCES users(id) ON DELETE SET NULL,
  signed_at      TEXT,
  signed_by      TEXT REFERENCES users(id) ON DELETE SET NULL,
  created_at     TEXT NOT NULL,
  updated_at     TEXT NOT NULL
);
CREATE INDEX idx_enc_patient ON encounters(patient_id, date);
CREATE INDEX idx_enc_doctor ON encounters(doctor_id, status);

CREATE TABLE encounter_versions (
  id           TEXT PRIMARY KEY,
  encounter_id TEXT NOT NULL REFERENCES encounters(id) ON DELETE CASCADE,
  version      INTEGER NOT NULL,
  at           TEXT NOT NULL,
  by_user      TEXT REFERENCES users(id) ON DELETE SET NULL,
  reason       TEXT NOT NULL DEFAULT '',
  data         TEXT NOT NULL,
  UNIQUE (encounter_id, version)
);

CREATE TABLE lab_orders (
  id            TEXT PRIMARY KEY,
  patient_id    TEXT NOT NULL REFERENCES patients(id) ON DELETE RESTRICT,
  encounter_id  TEXT REFERENCES encounters(id) ON DELETE SET NULL,
  doctor_id     TEXT NOT NULL REFERENCES doctors(id) ON DELETE RESTRICT,
  date          TEXT NOT NULL,
  status        TEXT NOT NULL DEFAULT 'ordered'
                CHECK (status IN ('ordered','collected','in_progress','completed','reviewed','cancelled')),
  priority      TEXT NOT NULL DEFAULT 'routine' CHECK (priority IN ('routine','urgent','stat')),
  created_at    TEXT NOT NULL
);
CREATE INDEX idx_lab_status ON lab_orders(status, date);
CREATE INDEX idx_lab_patient ON lab_orders(patient_id);

CREATE TABLE lab_order_tests (
  order_id TEXT NOT NULL REFERENCES lab_orders(id) ON DELETE CASCADE,
  test_id  TEXT NOT NULL REFERENCES lab_tests(id) ON DELETE RESTRICT,
  PRIMARY KEY (order_id, test_id)
);

CREATE TABLE lab_results (
  id          TEXT PRIMARY KEY,
  order_id    TEXT NOT NULL REFERENCES lab_orders(id) ON DELETE CASCADE,
  test_id     TEXT NOT NULL REFERENCES lab_tests(id) ON DELETE RESTRICT,
  value       TEXT NOT NULL,
  unit        TEXT NOT NULL DEFAULT '',
  ref         TEXT NOT NULL DEFAULT '',
  flag        TEXT NOT NULL DEFAULT 'normal' CHECK (flag IN ('normal','abnormal','critical')),
  at          TEXT NOT NULL,
  entered_by  TEXT REFERENCES users(id) ON DELETE SET NULL,
  reviewed_by TEXT REFERENCES users(id) ON DELETE SET NULL,
  reviewed_at TEXT
);
CREATE INDEX idx_lab_results_order ON lab_results(order_id);

CREATE TABLE invoices (
  id            TEXT PRIMARY KEY,
  no            TEXT NOT NULL UNIQUE,
  patient_id    TEXT NOT NULL REFERENCES patients(id) ON DELETE RESTRICT,
  encounter_id  TEXT REFERENCES encounters(id) ON DELETE SET NULL,
  doctor_id     TEXT REFERENCES doctors(id) ON DELETE SET NULL,
  date          TEXT NOT NULL,
  status        TEXT NOT NULL DEFAULT 'unpaid'
                CHECK (status IN ('unpaid','partial','paid','refunded','cancelled','pending_verify')),
  discount      INTEGER NOT NULL DEFAULT 0 CHECK (discount >= 0),
  cover         INTEGER NOT NULL DEFAULT 0 CHECK (cover BETWEEN 0 AND 100),
  insurer_id    TEXT REFERENCES insurers(id) ON DELETE SET NULL,
  created_by    TEXT REFERENCES users(id) ON DELETE SET NULL,
  created_at    TEXT NOT NULL,
  cancelled_at  TEXT,
  cancel_reason TEXT NOT NULL DEFAULT ''
);
CREATE INDEX idx_inv_patient ON invoices(patient_id);
CREATE INDEX idx_inv_date ON invoices(date);

CREATE TABLE invoice_items (
  id         TEXT PRIMARY KEY,
  invoice_id TEXT NOT NULL REFERENCES invoices(id) ON DELETE CASCADE,
  service_id TEXT REFERENCES services(id) ON DELETE SET NULL,
  desc_fa    TEXT NOT NULL,
  desc_en    TEXT NOT NULL DEFAULT '',
  qty        INTEGER NOT NULL DEFAULT 1 CHECK (qty > 0),
  price      INTEGER NOT NULL DEFAULT 0 CHECK (price >= 0)
);
CREATE INDEX idx_inv_items ON invoice_items(invoice_id);

CREATE TABLE payments (
  id         TEXT PRIMARY KEY,
  invoice_id TEXT NOT NULL REFERENCES invoices(id) ON DELETE CASCADE,
  amount     INTEGER NOT NULL,
  method     TEXT NOT NULL DEFAULT 'cash' CHECK (method IN ('cash','card','transfer','insurance','wallet')),
  ref        TEXT NOT NULL DEFAULT '',
  at         TEXT NOT NULL,
  by_user    TEXT REFERENCES users(id) ON DELETE SET NULL,
  kind       TEXT NOT NULL DEFAULT 'payment' CHECK (kind IN ('payment','refund'))
);
CREATE INDEX idx_payments_invoice ON payments(invoice_id);

CREATE TABLE follow_ups (
  id              TEXT PRIMARY KEY,
  patient_id      TEXT NOT NULL REFERENCES patients(id) ON DELETE RESTRICT,
  doctor_id       TEXT REFERENCES doctors(id) ON DELETE SET NULL,
  encounter_id    TEXT REFERENCES encounters(id) ON DELETE SET NULL,
  due             TEXT NOT NULL,
  reason_fa       TEXT NOT NULL DEFAULT '',
  reason_en       TEXT NOT NULL DEFAULT '',
  priority        TEXT NOT NULL DEFAULT 'routine' CHECK (priority IN ('routine','high','urgent')),
  status          TEXT NOT NULL DEFAULT 'pending'
                  CHECK (status IN ('pending','scheduled','contacted','completed','cancelled')),
  tests_before_fa TEXT NOT NULL DEFAULT '',
  tests_before_en TEXT NOT NULL DEFAULT '',
  created_at      TEXT NOT NULL,
  updated_at      TEXT NOT NULL
);
CREATE INDEX idx_fu_due ON follow_ups(status, due);
CREATE INDEX idx_fu_doctor ON follow_ups(doctor_id);

CREATE TABLE vitals (
  id              TEXT PRIMARY KEY,
  patient_id      TEXT NOT NULL REFERENCES patients(id) ON DELETE CASCADE,
  queue_entry_id  TEXT REFERENCES queue(id) ON DELETE SET NULL,
  at              TEXT NOT NULL,
  by_user         TEXT REFERENCES users(id) ON DELETE SET NULL,
  data            TEXT NOT NULL
);
CREATE INDEX idx_vitals_patient ON vitals(patient_id, at);

CREATE TABLE nurse_notes (
  id         TEXT PRIMARY KEY,
  patient_id TEXT NOT NULL REFERENCES patients(id) ON DELETE CASCADE,
  at         TEXT NOT NULL,
  by_user    TEXT REFERENCES users(id) ON DELETE SET NULL,
  kind       TEXT NOT NULL DEFAULT 'observation',
  text_fa    TEXT NOT NULL,
  text_en    TEXT NOT NULL DEFAULT ''
);

CREATE TABLE procedures (
  id           TEXT PRIMARY KEY,
  patient_id   TEXT NOT NULL REFERENCES patients(id) ON DELETE CASCADE,
  service_id   TEXT REFERENCES services(id) ON DELETE SET NULL,
  encounter_id TEXT REFERENCES encounters(id) ON DELETE SET NULL,
  at           TEXT NOT NULL,
  by_user      TEXT REFERENCES users(id) ON DELETE SET NULL,
  note_fa      TEXT NOT NULL DEFAULT '',
  note_en      TEXT NOT NULL DEFAULT ''
);

CREATE TABLE notifications (
  id      TEXT PRIMARY KEY,
  at      TEXT NOT NULL,
  to_role TEXT,
  to_user TEXT REFERENCES users(id) ON DELETE CASCADE,
  type    TEXT NOT NULL DEFAULT 'system',
  crit    INTEGER NOT NULL DEFAULT 0,
  read    INTEGER NOT NULL DEFAULT 0,
  text_fa TEXT NOT NULL,
  text_en TEXT NOT NULL DEFAULT '',
  link    TEXT NOT NULL DEFAULT ''
);
CREATE INDEX idx_notif_user ON notifications(to_user, read);
CREATE INDEX idx_notif_role ON notifications(to_role, read);

CREATE TABLE audit_log (
  id         TEXT PRIMARY KEY,
  at         TEXT NOT NULL,
  user_id    TEXT,
  user_name  TEXT NOT NULL DEFAULT '',
  role       TEXT NOT NULL DEFAULT '',
  action     TEXT NOT NULL,
  entity     TEXT NOT NULL DEFAULT '',
  entity_id  TEXT NOT NULL DEFAULT '',
  details    TEXT NOT NULL DEFAULT '',
  outcome    TEXT NOT NULL DEFAULT 'success',
  ip         TEXT NOT NULL DEFAULT ''
);
CREATE INDEX idx_audit_at ON audit_log(at DESC);
CREATE INDEX idx_audit_entity ON audit_log(entity, entity_id);

CREATE TRIGGER audit_log_no_update BEFORE UPDATE ON audit_log
BEGIN SELECT RAISE(ABORT, 'audit_log is append-only'); END;
CREATE TRIGGER audit_log_no_delete BEFORE DELETE ON audit_log
BEGIN SELECT RAISE(ABORT, 'audit_log is append-only'); END;

-- Money is derived in one place so list, detail, reports and payment validation agree.
CREATE VIEW invoice_balance AS
SELECT id, no, patient_id, encounter_id, doctor_id, date, status, insurer_id,
       subtotal, discount, insurance, due, paid, outstanding
FROM (
  SELECT i.id, i.no, i.patient_id, i.encounter_id, i.doctor_id, i.date, i.status, i.insurer_id,
    it.subtotal AS subtotal,
    MIN(i.discount, it.subtotal) AS discount,
    CAST(ROUND((it.subtotal - MIN(i.discount, it.subtotal)) * i.cover / 100.0) AS INTEGER) AS insurance,
    it.subtotal - MIN(i.discount, it.subtotal)
      - CAST(ROUND((it.subtotal - MIN(i.discount, it.subtotal)) * i.cover / 100.0) AS INTEGER) AS due,
    COALESCE((SELECT SUM(CASE WHEN p.kind = 'refund' THEN -p.amount ELSE p.amount END)
              FROM payments p WHERE p.invoice_id = i.id), 0) AS paid,
    it.subtotal - MIN(i.discount, it.subtotal)
      - CAST(ROUND((it.subtotal - MIN(i.discount, it.subtotal)) * i.cover / 100.0) AS INTEGER)
      - COALESCE((SELECT SUM(CASE WHEN p.kind = 'refund' THEN -p.amount ELSE p.amount END)
                  FROM payments p WHERE p.invoice_id = i.id), 0) AS outstanding
  FROM invoices i
  JOIN (SELECT invoice_id, COALESCE(SUM(qty * price), 0) AS subtotal
        FROM invoice_items GROUP BY invoice_id) it ON it.invoice_id = i.id
);

CREATE VIEW patient_balance AS
SELECT patient_id,
       COALESCE(SUM(CASE WHEN status NOT IN ('paid','refunded','cancelled')
                         THEN MAX(outstanding, 0) ELSE 0 END), 0) AS outstanding
FROM invoice_balance
GROUP BY patient_id;
