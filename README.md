# ClinicOS

Bilingual (Persian / English, RTL-first) clinic management and electronic medical record
system. The original single-file prototype has been rebuilt as a Rust web service with a
SQLite database, server-side authentication and a no-build JavaScript frontend.

## Features

- **Patients** — registration with duplicate review, bilingual demographics, national ID and
  phone validation, allergies/conditions/medications, nursing notes, documents-free profile,
  duplicate merge with audit trail.
- **Scheduling** — per-doctor working hours, slot generation, double-booking prevention,
  day worklist, month calendar, appointment lifecycle (requested → confirmed → checked-in →
  in progress → completed / no-show / cancelled).
- **Queue and triage** — walk-in check-in, ticket numbers, priority, call/start/finish flow,
  vitals capture with automatic BMI, nursing notes.
- **Clinical documentation** — draft → signed → amended or entered-in-error, immutable signed
  versions, diagnosis (ICD-10) and prescription editor with allergy checks, lab ordering,
  follow-up creation, bilingual print layout.
- **Laboratory** — order worklist, result entry with automatic normal/abnormal/critical flags,
  critical-result notifications, clinician acknowledgement.
- **Billing** — invoices with line items, discounts, insurance cover, partial payments,
  refunds, cancellation rules, printable receipts, outstanding balances.
- **Reports** — date-ranged summary, visits by day, per physician, per service category,
  payment methods, top diagnoses, CSV exports (BOM, Excel friendly).
- **Administration** — staff accounts and roles, physicians and schedules, service catalogue,
  clinic settings, append-only audit log, JSON backup export.

## Stack

| Layer | Choice |
| --- | --- |
| Language | Rust (edition 2021) |
| HTTP | axum 0.8 / tower |
| Database | SQLite via sqlx 0.8 (WAL, foreign keys, compile-time-checked schema at startup) |
| Passwords | argon2id |
| Sessions | opaque random tokens in an HttpOnly cookie, hashed at rest |
| Frontend | ES modules, no build step, server-rendered shell with client-side routing |
| Money | integer IRR, totals computed once in SQL views |

## Quick start (demo)

```bash
cargo build
mkdir -p data
CLINICOS_DEMO=1 \
CLINICOS_ADMIN_PASSWORD='AdminClinic!2026' \
DATABASE_URL="sqlite://$PWD/data/clinicos.db" \
CLINICOS_BIND=127.0.0.1:8080 \
./target/debug/clinicos
```

Open <http://127.0.0.1:8080>. Demo accounts (password `DemoClinic!2026`):

| Username | Role |
| --- | --- |
| `admin` | administrator (`AdminClinic!2026`) |
| `reception` | receptionist |
| `dr.moradi`, `dr.ahmadi` | physicians |
| `nurse` | nurse |
| `lab` | laboratory |
| `finance` | accountant |

Demo mode seeds ten patients, physicians, appointments, queue entries, encounters, lab orders
and invoices. It must never be enabled on a real installation.

## Production setup

1. Build the release binary: `cargo build --release`.
2. Create a service account and a data directory (`/var/lib/clinicos`).
3. Export the variables from `.env.example`. Set a long `CLINICOS_ADMIN_PASSWORD`; it is used
   once, on first start with an empty database, and the administrator is forced to change it
   at first sign-in.
4. Run `./target/release/clinicos` behind a TLS-terminating reverse proxy that forwards
   `X-Forwarded-For` (`CLINICOS_TRUST_PROXY=1`, `CLINICOS_COOKIE_SECURE=1`).
5. Back up the SQLite file (plus `-wal`/`-shm`) with the database stopped, or use
   `GET /api/admin/backup` for a JSON export.

The server refuses to start in production without an administrator password when the database
has no users, and never prints or stores the password in plaintext.

## Configuration

All settings come from environment variables; see `.env.example` for the full list. The
clinic name, calendar (Jalali/Gregorian), slot length, working hours and maximum discount are
editable at runtime under **Settings** and are stored in the database.

## Roles

| Role | Typical access |
| --- | --- |
| `admin` | everything, staff/configuration, audit log |
| `receptionist` | patients, appointments, queue, invoices, reports |
| `doctor` | own waiting list, encounters, labs, follow-ups, patient clinical record |
| `nurse` | patient search, queue, vitals, nursing notes |
| `lab` | patient lookup (limited), lab orders and results |
| `accountant` | invoices, payments, refunds, financial reports |

Authorization is enforced on every API route (`src/rbac.rs`); the frontend only mirrors it for
navigation.

## Security notes

- Argon2id password hashing, per-user lockout after repeated failures.
- Sessions: 256-bit tokens, only their SHA-256 hash is stored; HttpOnly, SameSite=Strict,
  Secure when configured; server-side expiry and revocation.
- Every mutating request requires the `x-clinicos-csrf` header and a same-origin `Origin`/
  `Referer` when present, on top of the session cookie.
- Strict Content-Security-Policy (self scripts/styles, Google Fonts only), no inline handlers.
- Parameterised SQL throughout; input validation and normalisation in `src/util.rs`.
- Append-only `audit_log` protected by SQLite triggers (no update/delete), recording user,
  role, action, entity, details, outcome and client IP.
- Clinical records are versioned; signed encounters cannot be edited, only amended with a
  reason, or voided as entered-in-error — both audited.

## Layout

```
src/
  main.rs        router, middleware, static serving
  auth.rs        login/logout/session extractor, CSRF
  rbac.rs        permissions per role
  db.rs          pool, migrations, sequences, notifications, audit
  api/           patients, appointments, queue, encounters, labs, billing,
                 dashboard, reports, notifications, admin
migrations/      0001_init.sql (schema, views, triggers)
web/             index.html, app.css, i18n.js, icons.js, lib.js and view modules
```

## Tests

```bash
cargo test            # unit tests (reference-range parsing, Jalali helpers)
```

An end-to-end smoke script exercised login, patient registration, booking conflicts, queue
check-in, encounter signing, lab result flags, invoice payments/refunds, reports and RBAC
denials against a running server.

## Releases

Pushing a tag runs `.github/workflows/release.yml`, which tests the project, builds the
release server for Linux x86_64, macOS arm64 and Windows x86_64, and publishes a GitHub
Release:

```bash
git tag v1.0.0
git push origin v1.0.0
```

Each archive (`clinicos-<tag>-<target>.tar.gz`, or `.zip` on Windows) contains the
`clinicos` binary, the `web` frontend, migrations, `README.md` and `.env.example`; a
`SHA256SUMS` file is attached to the release. The same workflow can be started manually
from the Actions tab with a tag name.
