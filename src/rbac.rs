use crate::error::{ApiResult, AppError};

pub const PATIENT_SEARCH: &str = "patient.search";
pub const PATIENT_REGISTER: &str = "patient.register";
pub const PATIENT_EDIT_ADMIN: &str = "patient.editAdmin";
pub const PATIENT_VIEW_LIMITED: &str = "patient.viewLimited";
pub const PATIENT_VIEW_CLINICAL: &str = "patient.viewClinical";
pub const PATIENT_MERGE: &str = "patient.merge";

pub const APPT_MANAGE: &str = "appt.manage";
pub const QUEUE_MANAGE: &str = "queue.manage";
pub const QUEUE_VIEW: &str = "queue.view";

pub const CLINICAL_WRITE: &str = "clinical.write";
pub const VITALS_WRITE: &str = "vitals.write";
pub const NURSING_NOTES: &str = "nursing.notes";
pub const ENCOUNTER_SIGN: &str = "encounter.sign";
pub const LAB_ORDER: &str = "lab.order";
pub const LAB_COLLECT: &str = "lab.collect";
pub const LAB_RESULT: &str = "lab.result";
pub const LAB_ACK: &str = "lab.ack";

pub const FU_MANAGE: &str = "fu.manage";
pub const FU_STATUS: &str = "fu.status";

pub const INVOICE_VIEW: &str = "invoice.view";
pub const PAYMENT_PROCESS: &str = "payment.process";
pub const REFUND_MANAGE: &str = "refund.manage";

pub const REPORTS_VIEW: &str = "reports.view";
pub const REPORTS_FINANCIAL: &str = "reports.financial";

pub const ADMIN_USERS: &str = "admin.users";
pub const ADMIN_CONFIG: &str = "admin.config";
pub const AUDIT_VIEW: &str = "audit.view";

pub const ROLES: [&str; 6] = ["admin", "receptionist", "doctor", "nurse", "lab", "accountant"];

pub fn perms(role: &str) -> &'static [&'static str] {
    match role {
        "admin" => &["*"],
        "receptionist" => &[
            PATIENT_SEARCH,
            PATIENT_REGISTER,
            PATIENT_EDIT_ADMIN,
            PATIENT_VIEW_LIMITED,
            APPT_MANAGE,
            QUEUE_MANAGE,
            QUEUE_VIEW,
            INVOICE_VIEW,
            PAYMENT_PROCESS,
            REPORTS_VIEW,
            FU_STATUS,
        ],
        "doctor" => &[
            PATIENT_SEARCH,
            PATIENT_VIEW_CLINICAL,
            CLINICAL_WRITE,
            ENCOUNTER_SIGN,
            LAB_ORDER,
            LAB_ACK,
            FU_MANAGE,
            QUEUE_VIEW,
            REPORTS_VIEW,
            INVOICE_VIEW,
        ],
        "nurse" => &[
            PATIENT_SEARCH,
            PATIENT_VIEW_CLINICAL,
            VITALS_WRITE,
            NURSING_NOTES,
            QUEUE_MANAGE,
            QUEUE_VIEW,
            FU_STATUS,
        ],
        "lab" => &[PATIENT_SEARCH, PATIENT_VIEW_LIMITED, LAB_COLLECT, LAB_RESULT],
        "accountant" => &[
            PATIENT_SEARCH,
            PATIENT_VIEW_LIMITED,
            INVOICE_VIEW,
            PAYMENT_PROCESS,
            REFUND_MANAGE,
            REPORTS_FINANCIAL,
            AUDIT_VIEW,
        ],
        _ => &[],
    }
}

pub fn can(role: &str, perm: &str) -> bool {
    let list = perms(role);
    list.contains(&"*") || list.contains(&perm)
}

pub fn require(user: &crate::auth::AuthUser, perm: &str) -> ApiResult<()> {
    if can(&user.role, perm) {
        Ok(())
    } else {
        Err(AppError::Forbidden)
    }
}

pub fn label(role: &str) -> &'static str {
    match role {
        "admin" => "System administrator",
        "receptionist" => "Receptionist",
        "doctor" => "Physician",
        "nurse" => "Nurse",
        "lab" => "Laboratory",
        "accountant" => "Accounting",
        _ => "Unknown",
    }
}
