// Clinical workspace: physician dashboard, waiting list, encounter editor, labs, follow-ups.
import {
  S, t, bi, esc, can, faNum, fmtDate, fmtTime, fmtDateTime, fmtAge, todayISO, isoToCal,
  patientName, doctorName, get, post, put, patch, go, toast, refresh, query, onAction, onForm,
  openModal, closeModal, confirmDlg, emptyState, pager, field, textField, textArea,
  selectField, dateField, readDate, formValues, badge, apptBadge, labBadge, encBadge,
  flagBadge, queueBadge, money, printDoc, patientPicker, spinner,
} from './lib.js';
import { ic } from './icons.js';

const FORM_MAP = [['tablet', 'قرص', 'Tablet'], ['capsule', 'کپسول', 'Capsule'], ['syrup', 'شربت', 'Syrup'], ['injection', 'تزریق', 'Injection'], ['inhaler', 'اسپری', 'Inhaler'], ['cream', 'پماد', 'Cream'], ['drops', 'قطره', 'Drops'], ['suppository', 'شیاف', 'Suppository']];
const ROUTE_MAP = [['PO', 'خوراکی', 'Oral'], ['IM', 'عضلانی', 'IM'], ['IV', 'وریدی', 'IV'], ['INH', 'استنشاقی', 'Inhaled'], ['TOP', 'موضعی', 'Topical'], ['SC', 'زیرجلدی', 'SC']];
const FREQ_MAP = [['OD', 'روزی یک‌بار', 'Once daily'], ['BD', 'روزی دو بار', 'Twice daily'], ['TDS', 'روزی سه بار', 'Three times daily'], ['QID', 'روزی چهار بار', 'Four times daily'], ['HS', 'شب‌ها', 'At bedtime'], ['PRN', 'در صورت نیاز', 'As needed'], ['STAT', 'فوری', 'Immediately'], ['WEEKLY', 'هفتگی', 'Weekly']];
const PRIORITY_MAP = [['routine', 'معمولی', 'Routine'], ['high', 'بالا', 'High'], ['urgent', 'فوری', 'Urgent']];
const LABPRI = [['routine', 'معمولی', 'Routine'], ['urgent', 'فوری', 'Urgent'], ['stat', 'بحرانی/فوری', 'STAT']];

const editor = { id: null, enc: null, amending: false };
let labTab = 'active';

const opt = (list) => list.map(([k, fa, en]) => [k, bi(fa, en)]);
const nameOf = (list, key) => {
  const f = list.find(([k]) => k === key);
  return f ? bi(f[1], f[2]) : key;
};
const dxList = () => S.meta?.icd10 || [];
const medList = () => S.meta?.meds || [];
const testList = () => S.meta?.labTests || [];

async function openConsult({ patientId, doctorId, appointmentId, queueEntryId }) {
  const res = await post('/api/encounters', {
    patientId,
    doctorId: doctorId || S.user.doctorId || '',
    appointmentId: appointmentId || null,
    queueEntryId: queueEntryId || null,
  });
  go(`#/consult/${res.id}`);
}

onAction('consult.start', async (el) => {
  await openConsult({ patientId: el.dataset.patient, doctorId: el.dataset.doctor });
});

/* ------------------------------------------------------------ doctor home */

export async function renderDoctorHome() {
  const [dash, queue] = await Promise.all([
    get('/api/dashboard'),
    get(`/api/queue${query({ date: todayISO(), doctorId: S.user.doctorId || '' })}`),
  ]);
  const waiting = (queue.items || []).filter((q) => ['waiting', 'called', 'in_progress'].includes(q.status));
  const k = dash.kpis;
  return `<div class="page-head"><div class="grow"><h2>${esc(bi('میز کار پزشک', 'Physician workspace'))}</h2>
      <div class="sub">${esc(fmtDate(dash.date))} · ${esc(bi('بیماران امروز شما', 'Your patients today'))}</div></div>
      <div class="head-actions">
        <button class="btn" data-link="#/doctor/consults">${ic('notes')}${esc(t('nav.consultations'))}</button>
        <button class="btn" data-link="#/doctor/labs">${ic('flask')}${esc(t('nav.lab_results'))}</button>
      </div></div>
    <div class="kpi-grid">
      <div class="stat accent"><div class="lbl">${ic('queue')}${esc(bi('در انتظار شما', 'Waiting for you'))}</div><div class="val">${faNum(waiting.length)}</div></div>
      <div class="stat"><div class="lbl">${ic('notes')}${esc(bi('پیش‌نویس‌ها', 'Drafts'))}</div><div class="val">${faNum(k.drafts)}</div></div>
      <div class="stat ${k.criticalResults ? 'err' : ''}"><div class="lbl">${ic('alert')}${esc(bi('نتایج بحرانی', 'Critical results'))}</div><div class="val">${faNum(k.criticalResults)}</div></div>
      <div class="stat warn"><div class="lbl">${ic('clock')}${esc(bi('پیگیری معوق', 'Overdue follow-ups'))}</div><div class="val">${faNum(k.overdueFollowUps)}</div></div>
      <div class="stat"><div class="lbl">${ic('check')}${esc(bi('ویزیت‌های امروز', 'Completed today'))}</div><div class="val">${faNum(k.completed)}</div></div>
    </div>
    <div class="split">
      <div class="card"><h3>${ic('queue')}${esc(bi('بیماران در انتظار', 'Patients waiting'))}</h3>
        ${waiting.length ? `<div class="stack">${waiting.map((q) => `<div class="flex-b" style="border-bottom:1px solid var(--line);padding-bottom:8px">
          <div class="flex"><span class="avatar" style="background:${q.priority ? 'var(--err)' : 'var(--pri)'}">${faNum(q.number)}</span>
            <div><a class="rowlink" href="#/patient/${esc(q.patientId)}">${esc(bi(q.patientName.fa, q.patientName.en))}</a>
              <div class="mini-note">${esc(q.patientCode)} · ${esc(fmtAge(q.dob))}</div></div></div>
          <div class="flex">${q.status === 'waiting' ? `<button class="btn xs" data-act="queue.status" data-id="${esc(q.id)}" data-status="called">${esc(bi('صدا زدن', 'Call'))}</button>` : ''}
            <button class="btn xs pri" data-act="queue.consult" data-id="${esc(q.id)}" data-patient="${esc(q.patientId)}" data-doctor="${esc(q.doctorId)}">${esc(bi('شروع ویزیت', 'Start visit'))}</button></div>
        </div>`).join('')}</div>` : emptyState('queue', bi('بیماری در انتظار نیست', 'Nobody is waiting'))}
      </div>
      <div class="stack">
        <div class="card"><h3>${ic('alert')}${esc(bi('نتایج بحرانی', 'Critical results'))}</h3>
          ${(dash.criticalResults || []).length ? `<div class="stack">${dash.criticalResults.map((c) => `<div class="flex-b" style="border-bottom:1px solid var(--line);padding-bottom:6px">
            <div><a class="rowlink" href="#/patient/${esc(c.patientId)}">${esc(bi(c.patientName.fa, c.patientName.en))}</a>
              <div class="mini-note">${esc(bi(c.test.fa, c.test.en))} · ${esc(c.code)} = <b style="color:var(--err)">${esc(c.value)}</b></div></div>
            <button class="btn xs" data-link="#/doctor/labs">${esc(bi('بررسی', 'Review'))}</button></div>`).join('')}</div>`
            : emptyState('check', bi('نتیجه بحرانی وجود ندارد', 'No critical results'))}
        </div>
        <div class="card"><h3>${ic('clock')}${esc(bi('پیگیری‌های معوق', 'Overdue follow-ups'))}</h3>
          ${(dash.overdueFollowUps || []).length ? `<div class="stack">${dash.overdueFollowUps.map((f) => `<div class="flex-b" style="border-bottom:1px solid var(--line);padding-bottom:6px">
            <div><a class="rowlink" href="#/patient/${esc(f.patientId)}">${esc(bi(f.patientName.fa, f.patientName.en))}</a>
              <div class="mini-note">${esc(bi(f.reason.fa, f.reason.en))} · ${esc(fmtDate(f.due))}</div></div>
            <button class="btn xs ok" data-act="fu.status" data-id="${esc(f.id)}" data-status="completed">${esc(bi('انجام شد', 'Done'))}</button></div>`).join('')}</div>`
            : emptyState('check', bi('پیگیری معوقی وجود ندارد', 'No overdue follow-ups'))}
        </div>
      </div>
    </div>`;
}

export async function renderWaiting() {
  const data = await get(`/api/queue${query({ date: todayISO(), doctorId: S.user.doctorId || '' })}`);
  const rows = (data.items || []).filter((q) => ['waiting', 'called', 'in_progress'].includes(q.status));
  return `<div class="page-head"><div class="grow"><h2>${esc(t('nav.waiting'))}</h2>
      <div class="sub">${esc(bi('بیماران در صف امروز شما', 'Your queue today'))}</div></div></div>
    <div class="card"><div class="stack">${rows.length ? rows.map((q) => `<div class="flex-b" style="border-bottom:1px solid var(--line);padding-bottom:9px">
      <div class="flex"><span class="avatar" style="background:${q.priority ? 'var(--err)' : 'var(--pri)'}">${faNum(q.number)}</span>
        <div><a class="rowlink" href="#/patient/${esc(q.patientId)}">${esc(bi(q.patientName.fa, q.patientName.en))}</a>
          <div class="mini-note">${esc(q.patientCode)} · ${esc(fmtAge(q.dob))} ${q.allergyCount ? ' · <span style="color:var(--err)">⚠ ' + faNum(q.allergyCount) + ' ' + esc(bi('حساسیت', 'allergies')) + '</span>' : ''}</div></div></div>
      <div class="flex">${queueBadge(q.status)}
        <button class="btn sm pri" data-act="queue.consult" data-id="${esc(q.id)}" data-patient="${esc(q.patientId)}" data-doctor="${esc(q.doctorId)}">${esc(bi('شروع ویزیت', 'Start visit'))}</button>
        <button class="btn sm" data-act="queue.status" data-id="${esc(q.id)}" data-status="done">${esc(bi('پایان', 'Done'))}</button></div>
    </div>`).join('') : emptyState('queue', bi('صف خالی است', 'Queue is empty'))}</div></div>`;
}

export async function renderConsults() {
  const params = new URLSearchParams(location.hash.split('?')[1] || '');
  const status = params.get('status') || '';
  const data = await get(`/api/encounters${query({ doctorId: S.user.role === 'doctor' ? S.user.doctorId : '', status, perPage: 50 })}`);
  return `<div class="page-head"><div class="grow"><h2>${esc(t('nav.consultations'))}</h2>
      <div class="sub">${esc(bi('اسناد بالینی، پیش‌نویس‌ها و نسخه‌های امضاشده', 'Clinical documents, drafts and signed notes'))}</div></div>
      <div class="head-actions">
        <select data-change="consults.status" class="btn" style="padding:7px 10px">
          <option value="">${esc(t('common.all'))}</option>
          ${['draft', 'signed', 'amended', 'entered_in_error'].map((s) => `<option value="${s}" ${status === s ? 'selected' : ''}>${esc(t('enc.v_' + s))}</option>`).join('')}
        </select>
      </div></div>
    <div class="tbl-wrap"><table class="tbl"><thead><tr>
      <th>${esc(bi('تاریخ', 'Date'))}</th><th>${esc(bi('بیمار', 'Patient'))}</th><th>${esc(bi('شکایت اصلی', 'Chief complaint'))}</th>
      <th>${esc(bi('تشخیص', 'Diagnosis'))}</th><th>${esc(bi('نسخه', 'Rx'))}</th><th>${esc(t('common.status'))}</th><th></th>
    </tr></thead><tbody>${(data.items || []).length ? data.items.map((e) => `<tr>
      <td>${esc(fmtDate(e.date))}<div class="mini-note">${esc(bi(e.doctorName.fa, e.doctorName.en))}</div></td>
      <td><a class="rowlink" href="#/patient/${esc(e.patientId)}">${esc(bi(e.patientName.fa, e.patientName.en))}</a>
        <div class="mini-note">${esc(e.patientCode)}</div></td>
      <td class="small">${esc(bi(e.chief?.fa, e.chief?.en))}</td>
      <td class="small">${esc(e.primaryDiagnosis?.code || '—')}</td>
      <td class="num">${faNum(e.prescriptionCount || 0)}</td>
      <td>${encBadge(e.status)}</td>
      <td><a class="btn xs" href="#/consult/${esc(e.id)}">${esc(e.status === 'draft' ? bi('ادامه', 'Continue') : bi('مشاهده', 'Open'))}</a></td>
    </tr>`).join('') : `<tr><td colspan="7">${emptyState('notes', bi('سندی یافت نشد', 'No documents found'))}</td></tr>`}</tbody></table></div>`;
}

onAction('consults.status', (el) => {
  const status = el.value;
  go(`#/doctor/consults${status ? '?status=' + encodeURIComponent(status) : ''}`);
});

/* --------------------------------------------------------- encounter view */

function vitalsValue(v, key) {
  return v?.[key] ?? '';
}

function dxRow(dx = {}) {
  return `<div class="dx-row">
    <select class="dx-code">${['', ...dxList().map((c) => c.code)].map((code) => {
      const item = dxList().find((c) => c.code === code);
      return `<option value="${esc(code)}" ${dx.code === code ? 'selected' : ''}>${esc(code ? `${code} · ${bi(item.titleFa, item.titleEn)}` : bi('انتخاب تشخیص', 'Select diagnosis'))}</option>`;
    }).join('')}</select>
    <select class="dx-type">${[['primary', bi('اصلی', 'Primary')], ['secondary', bi('ثانویه', 'Secondary')]].map(([k, l]) => `<option value="${k}" ${(dx.type || 'secondary') === k ? 'selected' : ''}>${esc(l)}</option>`).join('')}</select>
    <input class="dx-note" placeholder="${esc(bi('یادداشت', 'Note'))}" value="${esc(dx.note?.fa || '')}">
    <button class="btn xs err" type="button" data-act="consult.deldx">${ic('x', 'ic-s')}</button>
  </div>`;
}

function rxRow(rx = {}) {
  return `<div class="rx-row">
    <select class="rx-med">${['', ...medList().map((m) => m.id)].map((id) => {
      const m = medList().find((x) => x.id === id);
      return `<option value="${esc(id)}" ${rx.medId === id ? 'selected' : ''}>${esc(id ? bi(m.nameFa, m.nameEn) : bi('انتخاب دارو', 'Select drug'))}</option>`;
    }).join('')}</select>
    <input class="rx-dose" placeholder="${esc(bi('دوز', 'Dose'))}" value="${esc(rx.dose || '')}">
    <select class="rx-form">${opt(FORM_MAP).map(([k, l]) => `<option value="${k}" ${(rx.form || 'tablet') === k ? 'selected' : ''}>${esc(l)}</option>`).join('')}</select>
    <select class="rx-route">${opt(ROUTE_MAP).map(([k, l]) => `<option value="${k}" ${(rx.route || 'PO') === k ? 'selected' : ''}>${esc(l)}</option>`).join('')}</select>
    <select class="rx-freq">${opt(FREQ_MAP).map(([k, l]) => `<option value="${k}" ${(rx.freq || 'OD') === k ? 'selected' : ''}>${esc(l)}</option>`).join('')}</select>
    <input class="rx-days" type="number" min="1" max="365" value="${esc(rx.durDays || 7)}" title="${esc(bi('روز', 'days'))}">
    <input class="rx-qty" type="number" min="1" max="3650" value="${esc(rx.qty || 7)}" title="${esc(bi('تعداد', 'qty'))}">
    <input class="rx-note" placeholder="${esc(bi('دستور مصرف', 'Instructions'))}" value="${esc(rx.instructions?.fa || '')}">
    <button class="btn xs err" type="button" data-act="consult.delrx">${ic('x', 'ic-s')}</button>
  </div>`;
}

function emptyData() {
  return { chief: { fa: '', en: '' }, hpi: { fa: '', en: '' }, exam: { fa: '', en: '' }, plan: { fa: '', en: '' }, vitals: {}, diagnoses: [], prescriptions: [], labTests: [], labPriority: 'routine', followUp: null, referral: { fa: '', en: '' }, notes: '' };
}

function editForm(enc) {
  const d = enc.data || emptyData();
  const v = d.vitals || {};
  const fu = d.followUp;
  return `<form data-form="consult.save" data-id="${esc(enc.id)}">
    <div class="card">
      <div class="enc-section"><h4>${ic('notes')}${esc(bi('شرح حال', 'History'))}</h4>
        <div class="form">
          ${textArea(bi('شکایت اصلی (فارسی)', 'Chief complaint (Persian)'), 'chiefFa', d.chief?.fa || '', 'f-6', { rows: 2, required: true })}
          ${textArea('Chief complaint (English)', 'chiefEn', d.chief?.en || '', 'f-6', { rows: 2 })}
          ${textArea(bi('شرح حال حاضر (فارسی)', 'HPI (Persian)'), 'hpiFa', d.hpi?.fa || '', 'f-6', { rows: 4 })}
          ${textArea('HPI (English)', 'hpiEn', d.hpi?.en || '', 'f-6', { rows: 4 })}
        </div>
      </div>
      <div class="enc-section"><h4>${ic('heart')}${esc(bi('علائم حیاتی', 'Vital signs'))}</h4>
        <div class="form">
          ${textField(bi('فشار خون', 'Blood pressure'), 'vBp', vitalsValue(v, 'bp'), 'f-2', { placeholder: '120/80', dir: 'ltr' })}
          ${textField(bi('ضربان', 'Heart rate'), 'vHr', vitalsValue(v, 'hr'), 'f-2', { type: 'number', dir: 'ltr' })}
          ${textField(bi('تنفس', 'Respiratory'), 'vRr', vitalsValue(v, 'rr'), 'f-2', { type: 'number', dir: 'ltr' })}
          ${textField(bi('دما', 'Temperature'), 'vTemp', vitalsValue(v, 'temp'), 'f-2', { type: 'number', dir: 'ltr' })}
          ${textField(bi('SpO₂', 'SpO₂'), 'vSpo2', vitalsValue(v, 'spo2'), 'f-2', { type: 'number', dir: 'ltr' })}
          ${textField(bi('قند خون', 'Glucose'), 'vGlucose', vitalsValue(v, 'glucose'), 'f-2', { type: 'number', dir: 'ltr' })}
          ${textField(bi('قد (cm)', 'Height (cm)'), 'vHeight', vitalsValue(v, 'height'), 'f-2', { type: 'number', dir: 'ltr' })}
          ${textField(bi('وزن (kg)', 'Weight (kg)'), 'vWeight', vitalsValue(v, 'weight'), 'f-2', { type: 'number', dir: 'ltr' })}
        </div>
      </div>
      <div class="enc-section"><h4>${ic('stetho')}${esc(bi('معاینه', 'Examination'))}</h4>
        <div class="form">
          ${textArea(bi('یافته‌های معاینه (فارسی)', 'Examination findings (Persian)'), 'examFa', d.exam?.fa || '', 'f-6', { rows: 3 })}
          ${textArea('Examination findings (English)', 'examEn', d.exam?.en || '', 'f-6', { rows: 3 })}
        </div>
      </div>
      <div class="enc-section"><h4>${ic('clipboard')}${esc(bi('تشخیص‌ها', 'Diagnoses'))}</h4>
        <div id="dx-list">${(d.diagnoses || []).map(dxRow).join('')}</div>
        <button class="btn sm" type="button" data-act="consult.adddx">${ic('plus')}${esc(bi('افزودن تشخیص', 'Add diagnosis'))}</button>
      </div>
      <div class="enc-section"><h4>${ic('pill')}${esc(bi('نسخه', 'Prescription'))}</h4>
        <div id="rx-list">${(d.prescriptions || []).map(rxRow).join('')}</div>
        <button class="btn sm" type="button" data-act="consult.addrx">${ic('plus')}${esc(bi('افزودن دارو', 'Add medication'))}</button>
      </div>
      <div class="enc-section"><h4>${ic('flask')}${esc(bi('آزمایش‌ها', 'Laboratory'))}</h4>
        <div class="form">
          <div class="field f-12"><div class="chip-tests">${testList().map((x) => `<label><input type="checkbox" class="lab-test" value="${esc(x.id)}" ${(d.labTests || []).includes(x.id) ? 'checked' : ''}> ${esc(bi(x.nameFa, x.nameEn))}</label>`).join('')}</div></div>
          ${selectField(bi('اولویت', 'Priority'), 'labPriority', opt(LABPRI), 'f-4', d.labPriority || 'routine')}
        </div>
      </div>
      <div class="enc-section"><h4>${ic('clock')}${esc(bi('پیگیری', 'Follow-up'))}</h4>
        <div class="form">
          <div class="field f-12"><label class="checkline"><input type="checkbox" id="fu-on" ${fu ? 'checked' : ''}> ${esc(bi('نیاز به پیگیری دارد', 'Needs follow-up'))}</label></div>
          ${dateField('fuDue', fu?.due || '', 'f-4', { label: bi('تاریخ پیگیری', 'Follow-up date'), range: [isoToCal(todayISO()).y, isoToCal(todayISO()).y + 2] })}
          ${textField(bi('علت پیگیری', 'Follow-up reason'), 'fuReasonFa', fu?.reason?.fa || '', 'f-4')}
          ${selectField(bi('اولویت', 'Priority'), 'fuPriority', opt(PRIORITY_MAP), 'f-4', fu?.priority || 'routine')}
          ${textField(bi('آزمایش‌های لازم قبل از پیگیری', 'Tests before follow-up'), 'fuTestsFa', fu?.testsBefore?.fa || '', 'f-6')}
        </div>
      </div>
      <div class="enc-section"><h4>${ic('folder')}${esc(bi('برنامه و ارجاع', 'Plan & referral'))}</h4>
        <div class="form">
          ${textArea(bi('برنامه درمانی (فارسی)', 'Plan (Persian)'), 'planFa', d.plan?.fa || '', 'f-6', { rows: 3 })}
          ${textArea('Plan (English)', 'planEn', d.plan?.en || '', 'f-6', { rows: 3 })}
          ${textField(bi('ارجاع', 'Referral'), 'referralFa', d.referral?.fa || '', 'f-6')}
          ${textField(bi('یادداشت داخلی', 'Internal note'), 'notes', d.notes || '', 'f-6')}
        </div>
      </div>
    </div>
    <div class="flex-b mt14">
      <div class="flex">
        <button class="btn" type="submit">${ic('check')}${esc(t('common.save'))}</button>
        ${editor.amending ? `<button class="btn" type="button" data-act="consult.cancelamend">${esc(t('common.cancel'))}</button>` : ''}
      </div>
      <div class="flex">
        ${editor.amending
          ? `<button class="btn warn" type="button" data-act="consult.amend.save">${ic('edit')}${esc(bi('ثبت اصلاحیه', 'Save amendment'))}</button>`
          : `<button class="btn ok" type="button" data-act="consult.sign">${ic('shield')}${esc(bi('امضا و ثبت نهایی', 'Sign & finalise'))}</button>`}
        <button class="btn" type="button" data-act="consult.print">${ic('print')}${esc(t('common.print'))}</button>
      </div>
    </div>
  </form>`;
}

function section(title, value) {
  const fa = value?.fa || '';
  const en = value?.en || '';
  if (!fa && !en) return '';
  return `<h3 class="mt14">${esc(title)}</h3><div class="note-box">${esc(bi(fa, en))}</div>`;
}

function readView(enc) {
  const d = enc.data || emptyData();
  const v = d.vitals || {};
  const rx = d.prescriptions || [];
  const dx = d.diagnoses || [];
  const tests = (d.labTests || []).map((id) => testList().find((x) => x.id === id)).filter(Boolean);
  const fu = d.followUp;
  return `<div class="card">
    <div class="flex-b">
      <div class="flex">${encBadge(enc.status)} <span class="mini-note">${esc(bi('نسخه', 'version'))} ${faNum(enc.version)} · ${esc(fmtDateTime(enc.signedAt || enc.updatedAt))}</span></div>
      <div class="flex">
        ${enc.status === 'signed' || enc.status === 'amended' ? `<button class="btn sm" data-act="consult.print">${ic('print')}${esc(t('common.print'))}</button>` : ''}
        ${['signed', 'amended'].includes(enc.status) ? `<button class="btn sm warn" data-act="consult.amend">${ic('edit')}${esc(bi('اصلاحیه', 'Amend'))}</button>` : ''}
        ${can('encounter.sign') && enc.status !== 'entered_in_error' ? `<button class="btn sm err" data-act="consult.void">${ic('alert')}${esc(bi('ثبت اشتباه', 'Entered in error'))}</button>` : ''}
      </div>
    </div>
    ${enc.status !== 'draft' ? `<div class="lock-note mt8">${ic('shield')}<div>${esc(bi('این سند امضا شده و قابل ویرایش نیست؛ برای تغییر از «اصلاحیه» استفاده کنید.', 'This document is signed and immutable; use Amend to change it.'))}</div></div>` : ''}
    ${section(bi('شکایت اصلی', 'Chief complaint'), d.chief)}
    ${section(bi('شرح حال حاضر', 'History of present illness'), d.hpi)}
    <h3 class="mt14">${esc(bi('علائم حیاتی', 'Vital signs'))}</h3>
    <div class="enc-vitals">${[['BP', v.bp, ''], ['HR', v.hr, 'bpm'], ['RR', v.rr, '/min'], ['Temp', v.temp, '°C'], ['SpO₂', v.spo2, '%'], ['Glucose', v.glucose, 'mg/dL'], ['Height', v.height, 'cm'], ['Weight', v.weight, 'kg']].map(([label, value, unit]) => `<div class="bmi-box" style="background:var(--slate-l)"><div class="small muted">${esc(label)}</div><b style="font-size:15px">${esc(value ?? '—')}${value ? ' ' + esc(unit) : ''}</b></div>`).join('')}</div>
    ${section(bi('معاینه', 'Examination'), d.exam)}
    <h3 class="mt14">${esc(bi('تشخیص‌ها', 'Diagnoses'))}</h3>
    ${dx.length ? `<div class="tbl-wrap"><table class="tbl"><thead><tr><th>${esc(bi('کد', 'Code'))}</th><th>${esc(bi('عنوان', 'Title'))}</th><th>${esc(bi('نوع', 'Type'))}</th><th>${esc(bi('یادداشت', 'Note'))}</th></tr></thead>
      <tbody>${dx.map((x) => { const icd = dxList().find((c) => c.code === x.code); return `<tr><td class="num">${esc(x.code)}</td><td>${esc(icd ? bi(icd.titleFa, icd.titleEn) : '')}</td>
      <td>${badge(x.type === 'primary' ? bi('اصلی', 'Primary') : bi('ثانویه', 'Secondary'), x.type === 'primary' ? 'b-blue' : 'b-gray')}</td>
      <td class="small">${esc(bi(x.note?.fa, x.note?.en))}</td></tr>`; }).join('')}</tbody></table></div>` : `<div class="mini-note">${esc(bi('تشخیصی ثبت نشده', 'No diagnosis recorded'))}</div>`}
    <h3 class="mt14">${esc(bi('نسخه دارویی', 'Prescription'))}</h3>
    ${rx.length ? `<div class="tbl-wrap"><table class="tbl"><thead><tr><th>${esc(bi('دارو', 'Drug'))}</th><th>${esc(bi('دوز', 'Dose'))}</th><th>${esc(bi('فرم', 'Form'))}</th><th>${esc(bi('مسیر', 'Route'))}</th><th>${esc(bi('فراوانی', 'Frequency'))}</th><th>${esc(bi('مدت', 'Duration'))}</th><th>${esc(bi('تعداد', 'Qty'))}</th><th>${esc(bi('دستور', 'Instructions'))}</th></tr></thead>
      <tbody>${rx.map((x) => `<tr><td>${esc(bi(x.drug?.fa, x.drug?.en))}</td><td class="num">${esc(x.dose)}</td>
        <td>${esc(nameOf(FORM_MAP, x.form))}</td><td>${esc(nameOf(ROUTE_MAP, x.route))}</td><td>${esc(nameOf(FREQ_MAP, x.freq))}</td>
        <td>${faNum(x.durDays)} ${esc(t('common.days'))}</td><td>${faNum(x.qty)}</td><td class="small">${esc(bi(x.instructions?.fa, x.instructions?.en))}</td></tr>`).join('')}</tbody></table></div>` : `<div class="mini-note">${esc(bi('دارویی تجویز نشده', 'No medication prescribed'))}</div>`}
    ${tests.length ? `<h3 class="mt14">${esc(bi('آزمایش‌های درخواستی', 'Laboratory orders'))}</h3><div class="tag-row">${tests.map((x) => `<span class="tag">${esc(bi(x.nameFa, x.nameEn))}</span>`).join('')}<span class="tag amber">${esc(nameOf(LABPRI, d.labPriority || 'routine'))}</span></div>` : ''}
    ${section(bi('برنامه درمانی', 'Plan'), d.plan)}
    ${d.referral?.fa || d.referral?.en ? `<h3 class="mt14">${esc(bi('ارجاع', 'Referral'))}</h3><div class="note-box">${esc(bi(d.referral.fa, d.referral.en))}</div>` : ''}
    ${fu ? `<h3 class="mt14">${esc(bi('پیگیری', 'Follow-up'))}</h3><div class="note-box">${esc(bi(fu.reason?.fa, fu.reason?.en))} · ${esc(fmtDate(fu.due))} · ${esc(nameOf(PRIORITY_MAP, fu.priority))}</div>` : ''}
    ${d.notes ? `<h3 class="mt14">${esc(bi('یادداشت', 'Note'))}</h3><div class="note-box">${esc(d.notes)}</div>` : ''}
    <h3 class="mt14">${esc(bi('تاریخچه نسخه‌ها', 'Version history'))}</h3>
    <div>${(enc.versions || []).map((v2) => `<div class="version-row"><div class="flex" style="min-width:120px">${badge('v' + faNum(v2.version), 'b-gray')}</div>
      <div class="grow"><b>${esc(fmtDateTime(v2.at))}</b> · ${esc(bi(v2.by.fa, v2.by.en))}${v2.reason ? `<div class="mini-note">${esc(v2.reason)}</div>` : ''}</div></div>`).join('')}</div>
    <div class="mini-note mt8">${esc(bi('امضاکننده:', 'Signed by:'))} ${esc(enc.doctor?.name ? bi(enc.doctor.name.fa, enc.doctor.name.en) : '—')}${enc.doctor?.license ? ' · ' + esc(enc.doctor.license) : ''}</div>
  </div>`;
}

export async function renderConsult(id) {
  if (!id) return `<div class="card">${emptyState('alert', bi('شناسه سند نامعتبر است', 'Invalid document id'))}</div>`;
  const enc = await get(`/api/encounters/${id}`);
  if (editor.id !== id) {
    editor.id = id;
    editor.enc = enc;
    editor.amending = false;
  } else {
    editor.enc = enc;
  }
  const editable = enc.status === 'draft' || editor.amending;
  const patient = enc.patient || {};
  return `<div class="page-head">
      <div class="grow"><h2>${esc(bi('ویزیت', 'Encounter'))} · ${esc(bi(patient.firstNameFa, patient.firstNameEn))} ${esc(bi(patient.lastNameFa, patient.lastNameEn))}</h2>
        <div class="sub">${esc(fmtDate(enc.date))} · ${esc(enc.doctor?.name ? bi(enc.doctor.name.fa, enc.doctor.name.en) : '')} · ${esc(patient.code || '')}</div></div>
      <div class="head-actions">
        <a class="btn" href="#/patient/${esc(enc.patientId)}">${ic('user')}${esc(bi('پرونده بیمار', 'Patient record'))}</a>
        ${encappointmentLink(enc)}
      </div></div>
    ${editable ? editForm(enc) : readView(enc)}`;
}

function encappointmentLink(enc) {
  return enc.appointmentId ? `<button class="btn" data-link="#/appointments">${ic('calendar')}${esc(bi('نوبت', 'Appointment'))}</button>` : '';
}

/* ---------------------------------------------------------- editor actions */

onAction('consult.adddx', () => {
  document.getElementById('dx-list')?.insertAdjacentHTML('beforeend', dxRow());
});

onAction('consult.deldx', (el) => { el.closest('.dx-row')?.remove(); });
onAction('consult.addrx', () => {
  document.getElementById('rx-list')?.insertAdjacentHTML('beforeend', rxRow());
});
onAction('consult.delrx', (el) => { el.closest('.rx-row')?.remove(); });

function collect(form) {
  const val = (name) => form.querySelector(`[name="${name}"]`)?.value.trim() || '';
  const numOr = (name, fallback = 0) => {
    const raw = form.querySelector(`[name="${name}"]`)?.value;
    const n = Number(raw);
    return Number.isFinite(n) && n > 0 ? n : fallback;
  };
  const vitals = {};
  [['vBp', 'bp'], ['vHr', 'hr'], ['vRr', 'rr'], ['vTemp', 'temp'], ['vSpo2', 'spo2'], ['vGlucose', 'glucose'], ['vHeight', 'height'], ['vWeight', 'weight']].forEach(([fieldName, key]) => {
    const raw = val(fieldName);
    if (!raw) return;
    if (key === 'bp') { vitals.bp = raw; return; }
    const n = Number(raw);
    if (Number.isFinite(n)) vitals[key] = n;
  });

  const diagnoses = [...form.querySelectorAll('.dx-row')].map((row) => ({
    code: row.querySelector('.dx-code').value,
    type: row.querySelector('.dx-type').value,
    note: { fa: row.querySelector('.dx-note').value.trim(), en: '' },
  })).filter((d) => d.code);

  const prescriptions = [...form.querySelectorAll('.rx-row')].map((row) => ({
    medId: row.querySelector('.rx-med').value,
    dose: row.querySelector('.rx-dose').value.trim(),
    form: row.querySelector('.rx-form').value,
    route: row.querySelector('.rx-route').value,
    freq: row.querySelector('.rx-freq').value,
    durDays: Number(row.querySelector('.rx-days').value) || 7,
    qty: Number(row.querySelector('.rx-qty').value) || 7,
    instructions: { fa: row.querySelector('.rx-note').value.trim(), en: '' },
  })).filter((r) => r.medId);

  const fuOn = form.querySelector('#fu-on')?.checked;
  const fuDue = readDate(form, 'fuDue');
  const followUp = fuOn && fuDue ? {
    due: fuDue,
    reason: { fa: val('fuReasonFa'), en: '' },
    priority: val('fuPriority') || 'routine',
    testsBefore: { fa: val('fuTestsFa'), en: '' },
  } : null;

  return {
    chief: { fa: val('chiefFa'), en: val('chiefEn') },
    hpi: { fa: val('hpiFa'), en: val('hpiEn') },
    exam: { fa: val('examFa'), en: val('examEn') },
    plan: { fa: val('planFa'), en: val('planEn') },
    vitals,
    diagnoses,
    prescriptions,
    labTests: [...form.querySelectorAll('.lab-test:checked')].map((x) => x.value),
    labPriority: val('labPriority') || 'routine',
    followUp,
    referral: { fa: val('referralFa'), en: '' },
    notes: val('notes'),
  };
}

onForm('consult.save', async (form) => {
  const enc = editor.enc;
  const data = collect(form);
  await put(`/api/encounters/${form.dataset.id}`, { baseVersion: enc.version, data });
  toast(bi('پیش‌نویس ذخیره شد', 'Draft saved'), 'ok');
  refresh();
});

onAction('consult.sign', () => {
  const form = document.querySelector('form[data-form="consult.save"]');
  if (!form) return;
  const data = collect(form);
  if (!data.chief.fa && !data.chief.en) { toast(bi('شکایت اصلی الزامی است', 'Chief complaint is required'), 'err'); return; }
  if (!data.diagnoses.length) { toast(bi('حداقل یک تشخیص لازم است', 'At least one diagnosis is required'), 'err'); return; }
  confirmDlg(
    bi('امضای سند بالینی', 'Sign clinical document'),
    bi('پس از امضا، سند قابل ویرایش نیست و فقط با «اصلاحیه» تغییر می‌کند. آزمایش‌ها و پیگیری‌های تعیین‌شده ثبت خواهند شد.', 'After signing, the record is immutable (amend only) and ordered labs and follow-ups are created.'),
    async () => {
      await post(`/api/encounters/${editor.id}/sign`, { data });
      toast(bi('سند امضا و ثبت شد', 'Document signed'), 'ok');
      editor.amending = false;
      refresh();
    },
    { okText: bi('امضا', 'Sign'), danger: false },
  );
});

onAction('consult.amend', () => {
  editor.amending = true;
  refresh();
});

onAction('consult.cancelamend', () => {
  editor.amending = false;
  refresh();
});

onAction('consult.amend.save', () => {
  const form = document.querySelector('form[data-form="consult.save"]');
  if (!form) return;
  const data = collect(form);
  openModal({
    title: bi('ثبت اصلاحیه', 'Record amendment'),
    size: 'narrow',
    body: `<form data-form="consult.amend" data-id="${esc(editor.id)}">
      <div class="alert amber">${ic('alert')}<div>${esc(bi('نسخه قبلی حفظ می‌شود و نسخه جدید با دلیل اصلاح ثبت خواهد شد.', 'The previous version is retained and the new version records the reason.'))}</div></div>
      <div class="field"><label>${esc(bi('دلیل اصلاحیه', 'Reason for amendment'))} <span class="req">*</span></label>
        <textarea name="reason" rows="3" required></textarea></div>
      <div class="flex-b mt14">
        <button class="btn" type="button" data-close>${esc(t('common.cancel'))}</button>
        <button class="btn warn" type="submit">${esc(bi('ثبت اصلاحیه', 'Save amendment'))}</button>
      </div>
    </form>`,
    onMount: () => { editor._pendingAmend = data; },
  });
});

onForm('consult.amend', async (form) => {
  const reason = formValues(form).reason;
  if (!reason || reason.trim().length < 5) throw new Error(bi('دلیل اصلاح حداقل ۵ نویسه باشد', 'Reason must be at least 5 characters'));
  await post(`/api/encounters/${form.dataset.id}/amend`, { reason, data: editor._pendingAmend || {} });
  closeModal();
  toast(bi('اصلاحیه ثبت شد', 'Amendment saved'), 'ok');
  editor.amending = false;
  refresh();
});

onAction('consult.void', () => {
  openModal({
    title: bi('ثبت به‌عنوان اشتباه', 'Mark entered in error'),
    size: 'narrow',
    body: `<form data-form="consult.void" data-id="${esc(editor.id)}">
      <div class="alert red">${ic('alert')}<div>${esc(bi('این سند از گزارش‌ها کنار گذاشته می‌شود. این عمل در گزارش رخدادها ثبت می‌گردد.', 'This document is excluded from reports. The action is audited.'))}</div></div>
      <div class="field"><label>${esc(bi('دلیل', 'Reason'))} <span class="req">*</span></label><textarea name="reason" rows="3" required></textarea></div>
      <div class="flex-b mt14"><button class="btn" type="button" data-close>${esc(t('common.cancel'))}</button>
        <button class="btn err" type="submit">${esc(bi('ثبت', 'Confirm'))}</button></div>
    </form>`,
  });
});

onForm('consult.void', async (form) => {
  await post(`/api/encounters/${form.dataset.id}/entered-in-error`, { reason: formValues(form).reason });
  closeModal();
  toast(bi('سند به‌عنوان اشتباه ثبت شد', 'Marked entered in error'), 'warn');
  refresh();
});

/* ------------------------------------------------------------------ print */

onAction('consult.print', () => {
  const enc = editor.enc;
  if (!enc) return;
  const d = enc.data || {};
  const clinic = S.meta?.clinic || {};
  const patient = enc.patient || {};
  const patientFull = `${bi(patient.firstNameFa, patient.firstNameEn)} ${bi(patient.lastNameFa, patient.lastNameEn)}`;
  const dx = d.diagnoses || [];
  const rx = d.prescriptions || [];
  const fu = d.followUp;
  const html = `<div class="pr-doc">
    <div class="pr-head"><div><h2>${esc(clinic.nameFa || 'ClinicOS')}</h2><small>${esc(clinic.nameEn || '')} · ${esc(clinic.phone || '')}</small></div>
      <div style="text-align:end"><b>${esc(bi('نسخه پزشکی', 'Medical record'))}</b><div><small>${esc(fmtDate(enc.date))} · ${esc(enc.doctor?.license || '')}</small></div></div></div>
    <div class="pr-meta"><div><span>${esc(bi('بیمار', 'Patient'))}:</span> ${esc(patientFull)}</div><div><span>${esc(bi('کد', 'Code'))}:</span> ${esc(patient.code || '')}</div>
      <div><span>${esc(bi('پزشک', 'Physician'))}:</span> ${esc(enc.doctor?.name ? bi(enc.doctor.name.fa, enc.doctor.name.en) : '')}</div><div><span>${esc(bi('سن', 'Age'))}:</span> ${esc(fmtAge(patient.dob))}</div></div>
    <h3>${esc(bi('شکایت اصلی', 'Chief complaint'))}</h3><p>${esc(bi(d.chief?.fa, d.chief?.en))}</p>
    ${d.hpi?.fa ? `<h3>${esc(bi('شرح حال', 'History'))}</h3><p>${esc(bi(d.hpi.fa, d.hpi.en))}</p>` : ''}
    ${d.exam?.fa ? `<h3>${esc(bi('معاینه', 'Examination'))}</h3><p>${esc(bi(d.exam.fa, d.exam.en))}</p>` : ''}
    <h3>${esc(bi('تشخیص', 'Diagnosis'))}</h3>
    <table class="pr-tbl"><thead><tr><th>${esc(bi('کد', 'Code'))}</th><th>${esc(bi('عنوان', 'Title'))}</th><th>${esc(bi('نوع', 'Type'))}</th></tr></thead>
      <tbody>${dx.map((x) => { const icd = dxList().find((c) => c.code === x.code); return `<tr><td>${esc(x.code)}</td><td>${esc(icd ? bi(icd.titleFa, icd.titleEn) : '')}</td><td>${esc(x.type)}</td></tr>`; }).join('')}</tbody></table>
    ${rx.length ? `<h3>${esc(bi('نسخه دارویی', 'Prescription'))}</h3><table class="pr-tbl"><thead><tr><th>${esc(bi('دارو', 'Drug'))}</th><th>${esc(bi('دوز', 'Dose'))}</th><th>${esc(bi('فراوانی', 'Frequency'))}</th><th>${esc(bi('مدت', 'Duration'))}</th></tr></thead>
      <tbody>${rx.map((x) => `<tr><td>${esc(bi(x.drug?.fa, x.drug?.en))}</td><td>${esc(x.dose)}</td><td>${esc(nameOf(FREQ_MAP, x.freq))}</td><td>${faNum(x.durDays)} ${esc(t('common.days'))}</td></tr>`).join('')}</tbody></table>` : ''}
    ${d.plan?.fa ? `<h3>${esc(bi('برنامه درمانی', 'Plan'))}</h3><p>${esc(bi(d.plan.fa, d.plan.en))}</p>` : ''}
    ${fu ? `<p><b>${esc(bi('پیگیری', 'Follow-up'))}:</b> ${esc(fmtDate(fu.due))} — ${esc(bi(fu.reason?.fa, fu.reason?.en))}</p>` : ''}
    <div class="pr-sign"><div>${esc(bi('امضای پزشک', 'Physician signature'))}</div><div>${esc(bi('تاریخ', 'Date'))}</div></div>
    <div class="pr-foot">${esc(bi('این سند توسط ClinicOS تولید شده است.', 'Generated by ClinicOS'))}</div>
  </div>`;
  printDoc(html);
});

/* ------------------------------------------------------------- doctor labs */

export async function renderDoctorLabs() {
  const data = await get(`/api/labs${query({ doctorId: S.user.role === 'doctor' ? S.user.doctorId : '', perPage: 50 })}`);
  const rows = data.items || [];
  return `<div class="page-head"><div class="grow"><h2>${esc(t('nav.lab_results'))}</h2>
      <div class="sub">${esc(bi('نتایج آزمایش و تأیید پزشک', 'Lab results and clinician acknowledgement'))}</div></div></div>
    <div class="tbl-wrap"><table class="tbl"><thead><tr>
      <th>${esc(bi('تاریخ', 'Date'))}</th><th>${esc(bi('بیمار', 'Patient'))}</th><th>${esc(bi('آزمون‌ها', 'Tests'))}</th>
      <th>${esc(bi('نتیجه', 'Results'))}</th><th>${esc(t('common.status'))}</th><th></th></tr></thead>
      <tbody>${rows.length ? rows.map((l) => `<tr>
        <td>${esc(fmtDate(l.date))}</td>
        <td><a class="rowlink" href="#/patient/${esc(l.patientId)}">${esc(bi(l.patientName.fa, l.patientName.en))}</a><div class="mini-note">${esc(l.patientCode)}</div></td>
        <td class="num">${faNum(l.testCount)}</td>
        <td>${faNum(l.resultCount)}${l.criticalCount ? ` · <b style="color:var(--err)">${faNum(l.criticalCount)} ${esc(bi('بحرانی', 'critical'))}</b>` : ''}</td>
        <td>${labBadge(l.status)}${l.priority !== 'routine' ? `<div class="mini-note">${esc(l.priority)}</div>` : ''}</td>
        <td><button class="btn xs" data-act="lab.open" data-id="${esc(l.id)}">${esc(bi('مشاهده', 'Open'))}</button></td>
      </tr>`).join('') : `<tr><td colspan="6">${emptyState('flask', bi('آزمایشی ثبت نشده', 'No lab orders'))}</td></tr>`}</tbody></table></div>`;
}

/* ---------------------------------------------------------------- followups */

export async function renderFollowUps() {
  const data = await get(`/api/followups${query({ perPage: 100 })}`);
  const items = data.items || [];
  return `<div class="page-head"><div class="grow"><h2>${esc(t('nav.followups'))}</h2>
      <div class="sub">${esc(bi('پیگیری بیماران و اقدامات لازم', 'Patient follow-up worklist'))}</div></div></div>
    <div class="tbl-wrap"><table class="tbl"><thead><tr>
      <th>${esc(bi('سررسید', 'Due'))}</th><th>${esc(bi('بیمار', 'Patient'))}</th><th>${esc(bi('علت', 'Reason'))}</th>
      <th>${esc(bi('آزمایش قبل', 'Tests before'))}</th><th>${esc(bi('اولویت', 'Priority'))}</th><th>${esc(t('common.status'))}</th><th></th></tr></thead>
      <tbody>${items.length ? items.map((f) => `<tr>
        <td>${esc(fmtDate(f.due))}${f.overdue ? `<div class="mini-note" style="color:var(--err)">${esc(bi('سررسید گذشته', 'overdue'))}</div>` : ''}</td>
        <td><a class="rowlink" href="#/patient/${esc(f.patientId)}">${esc(bi(f.patientName.fa, f.patientName.en))}</a>
          <div class="mini-note">${esc(f.patientCode)} · ${esc(f.phone || '')}</div></td>
        <td class="small">${esc(bi(f.reason.fa, f.reason.en))}</td>
        <td class="small">${esc(bi(f.testsBefore.fa, f.testsBefore.en) || '—')}</td>
        <td><span class="tag ${f.priority === 'urgent' ? 'red' : f.priority === 'high' ? 'amber' : 'gray'}">${esc(nameOf(PRIORITY_MAP, f.priority))}</span></td>
        <td>${esc(nameOf([['pending', 'در انتظار', 'Pending'], ['scheduled', 'زمان‌بندی', 'Scheduled'], ['contacted', 'تماس شد', 'Contacted'], ['completed', 'انجام شد', 'Completed'], ['cancelled', 'لغو', 'Cancelled']], f.status))}</td>
        <td><div class="actions">
          ${['pending', 'scheduled'].includes(f.status) ? `<button class="btn xs" data-act="fu.status" data-id="${esc(f.id)}" data-status="contacted">${esc(bi('تماس شد', 'Contacted'))}</button>
          <button class="btn xs ok" data-act="fu.status" data-id="${esc(f.id)}" data-status="completed">${esc(bi('انجام شد', 'Complete'))}</button>` : ''}
          <button class="btn xs" data-act="fu.reschedule" data-id="${esc(f.id)}">${esc(bi('تعویق', 'Reschedule'))}</button>
        </div></td>
      </tr>`).join('') : `<tr><td colspan="7">${emptyState('clock', bi('پیگیری‌ای وجود ندارد', 'No follow-ups'))}</td></tr>`}</tbody></table></div>`;
}

onAction('fu.reschedule', (el) => {
  openModal({
    title: bi('تعویق پیگیری', 'Reschedule follow-up'),
    size: 'narrow',
    body: `<form data-form="fu.reschedule" data-id="${esc(el.dataset.id)}">
      <div class="form">${dateField('due', todayISO(), 'f-12', { label: bi('تاریخ جدید', 'New date'), range: [isoToCal(todayISO()).y, isoToCal(todayISO()).y + 2] })}</div>
      <div class="flex-b mt14"><button class="btn" type="button" data-close>${esc(t('common.cancel'))}</button>
      <button class="btn pri" type="submit">${esc(t('common.save'))}</button></div></form>`,
  });
});

onForm('fu.reschedule', async (form) => {
  const due = readDate(form, 'due');
  if (!due) throw new Error(bi('تاریخ را انتخاب کنید', 'Choose a date'));
  await patch(`/api/followups/${form.dataset.id}`, { due, status: 'scheduled' });
  closeModal();
  toast(t('adm.saved'), 'ok');
  refresh();
});

/* ------------------------------------------------------------ lab workspace */

export async function renderLabWorkspace() {
  const data = await get(`/api/labs${query({ perPage: 100 })}`);
  const all = data.items || [];
  const tabs = [
    ['active', bi('در جریان', 'Active'), (l) => ['ordered', 'collected', 'in_progress'].includes(l.status)],
    ['completed', bi('آماده بررسی', 'Ready'), (l) => l.status === 'completed'],
    ['reviewed', bi('تأییدشده', 'Reviewed'), (l) => l.status === 'reviewed'],
    ['all', bi('همه', 'All'), () => true],
  ];
  const activeTab = tabs.find(([k]) => k === labTab) || tabs[0];
  const rows = all.filter(activeTab[2]);
  const critical = all.filter((l) => l.criticalCount > 0 && l.status !== 'reviewed');

  return `<div class="page-head"><div class="grow"><h2>${esc(t('nav.lab_orders'))}</h2>
      <div class="sub">${esc(bi('دریافت نمونه، ثبت نتایج و هشدار مقادیر بحرانی', 'Specimen processing, result entry and critical alerts'))}</div></div>
      <div class="head-actions">${can('lab.order') ? `<button class="btn" data-act="lab.order">${ic('plus')}${esc(bi('درخواست آزمایش', 'New order'))}</button>` : ''}</div></div>
    ${critical.length ? `<div class="banner-crit">${ic('alert')}<span>${esc(bi('نتایج بحرانی در انتظار تأیید پزشک', 'Critical results awaiting clinician review'))}</span></div>` : ''}
    <div class="subtabs">${tabs.map(([k, label, fn]) => `<button class="${labTab === k ? 'on' : ''}" data-act="lab.tab" data-tab="${k}">${esc(label)} <span class="mini-note">${faNum(all.filter(fn).length)}</span></button>`).join('')}</div>
    <div class="tbl-wrap"><table class="tbl"><thead><tr>
      <th>${esc(bi('تاریخ', 'Date'))}</th><th>${esc(bi('بیمار', 'Patient'))}</th><th>${esc(bi('پزشک', 'Doctor'))}</th>
      <th>${esc(bi('آزمون', 'Tests'))}</th><th>${esc(bi('نتایج', 'Results'))}</th><th>${esc(bi('اولویت', 'Priority'))}</th><th>${esc(t('common.status'))}</th><th></th></tr></thead>
      <tbody>${rows.length ? rows.map((l) => `<tr>
        <td>${esc(fmtDate(l.date))}</td>
        <td><a class="rowlink" href="#/patient/${esc(l.patientId)}">${esc(bi(l.patientName.fa, l.patientName.en))}</a><div class="mini-note">${esc(l.patientCode)}</div></td>
        <td class="small">${esc(bi(l.doctorName.fa, l.doctorName.en))}</td>
        <td class="num">${faNum(l.testCount)}</td>
        <td class="num">${faNum(l.resultCount)}${l.criticalCount ? ` <b style="color:var(--err)">(${faNum(l.criticalCount)})</b>` : ''}</td>
        <td>${l.priority === 'routine' ? '—' : `<span class="tag ${l.priority === 'stat' ? 'red' : 'amber'}">${esc(l.priority)}</span>`}</td>
        <td>${labBadge(l.status)}</td>
        <td><button class="btn xs pri" data-act="lab.open" data-id="${esc(l.id)}">${esc(bi('ثبت نتیجه', 'Enter results'))}</button></td>
      </tr>`).join('') : `<tr><td colspan="8">${emptyState('flask', bi('صف آزمایش خالی است', 'No lab orders in this view'))}</td></tr>`}</tbody></table></div>`;
}

onAction('lab.tab', (el) => { labTab = el.dataset.tab; refresh(); });

export async function openLab(id) {
  const order = await get(`/api/labs/${id}`);
  const closed = ['reviewed', 'cancelled'].includes(order.status);
  const canResult = can('lab.result') && !closed;
  openModal({
    title: `${bi('درخواست آزمایش', 'Lab order')} · ${order.patient.code}`,
    size: 'wide',
    body: `<div class="flex-b mb8"><div>
        <b>${esc(order.patient.firstNameFa)} ${esc(order.patient.lastNameFa)}</b>
        <div class="mini-note">${esc(fmtDate(order.date))} · ${esc(bi(order.doctorName.fa, order.doctorName.en))}</div>
      </div><div class="flex">${labBadge(order.status)}${order.priority !== 'routine' ? `<span class="tag ${order.priority === 'stat' ? 'red' : 'amber'}">${esc(order.priority)}</span>` : ''}</div></div>
      <form data-form="lab.results" data-id="${esc(order.id)}">
        <div class="tbl-wrap"><table class="tbl"><thead><tr>
          <th>${esc(bi('آزمون', 'Test'))}</th><th>${esc(bi('محدوده', 'Reference'))}</th><th>${esc(bi('نتیجه', 'Value'))}</th><th>${esc(bi('وضعیت', 'Flag'))}</th></tr></thead>
          <tbody>${(order.tests || []).map((t) => `<tr>
            <td><b>${esc(t.code)}</b> · ${esc(bi(t.name.fa, t.name.en))}<div class="mini-note">${esc(t.unit || '')}</div></td>
            <td class="small mono">${esc(order.patient.gender === 'M' ? t.refM : t.refF)}</td>
            <td>${canResult ? `<input class="lab-value" data-test="${esc(t.id)}" value="${esc(t.value || '')}" style="width:110px;border:1px solid var(--line2);border-radius:7px;padding:4px 7px" dir="ltr">` : esc(t.value || '—')}</td>
            <td>${t.flag ? flagBadge(t.flag) : `<span class="mini-note">${esc(bi('در انتظار', 'Pending'))}</span>`}</td>
          </tr>`).join('')}</tbody></table></div>
        ${canResult ? `<div class="flex-b mt14"><div class="mini-note">${esc(bi('خارج از محدوده به‌صورت خودکار علامت‌گذاری می‌شود.', 'Out-of-range values are flagged automatically.'))}</div>
          <button class="btn pri" type="submit">${ic('check')}${esc(bi('ثبت نتایج', 'Save results'))}</button></div>` : ''}
      </form>
      ${can('lab.ack') && order.status === 'completed' ? `<div class="flex-b mt14"><span class="mini-note">${esc(bi('همه نتایج ثبت شده‌اند.', 'All results are in.'))}</span>
        <button class="btn ok" data-act="lab.review" data-id="${esc(order.id)}">${ic('check')}${esc(bi('تأیید و بررسی', 'Mark reviewed'))}</button></div>` : ''}`,
    onMount: null,
  });
}

onAction('lab.open', async (el) => { await openLab(el.dataset.id); });

onForm('lab.results', async (form) => {
  const results = [...form.querySelectorAll('.lab-value')]
    .map((input) => ({ testId: input.dataset.test, value: input.value.trim() }))
    .filter((r) => r.value !== '');
  if (!results.length) throw new Error(bi('حداقل یک نتیجه وارد کنید', 'Enter at least one result'));
  const res = await post(`/api/labs/${form.dataset.id}/results`, { results });
  closeModal();
  toast(res.critical?.length ? bi('نتیجه بحرانی ثبت شد و به پزشک اطلاع داده شد', 'Critical result recorded and the physician notified') : bi('نتایج ذخیره شد', 'Results saved'), res.critical?.length ? 'warn' : 'ok');
  refresh();
});

onAction('lab.review', async (el) => {
  await post(`/api/labs/${el.dataset.id}/review`, {});
  closeModal();
  toast(bi('نتیجه تأیید شد', 'Result acknowledged'), 'ok');
  refresh();
});

onAction('lab.order', () => {
  openModal({
    title: bi('درخواست آزمایش', 'New lab order'),
    size: '',
    body: `<form data-form="lab.create">
      <div class="form">
        ${patientPicker('patientId', bi('بیمار', 'Patient'), 'f-12')}
        <div class="field f-12"><label>${esc(bi('آزمون‌ها', 'Tests'))}</label>
          <div class="chip-tests">${testList().map((x) => `<label><input type="checkbox" class="lab-test" value="${esc(x.id)}"> ${esc(x.code)} · ${esc(bi(x.nameFa, x.nameEn))}</label>`).join('')}</div></div>
        ${selectField(bi('اولویت', 'Priority'), 'priority', opt(LABPRI), 'f-6', 'routine')}
      </div>
      <div class="flex-b mt14"><button class="btn" type="button" data-close>${esc(t('common.cancel'))}</button>
        <button class="btn pri" type="submit">${esc(bi('ثبت درخواست', 'Create order'))}</button></div>
    </form>`,
  });
});

onForm('lab.create', async (form) => {
  const v = formValues(form);
  const testIds = [...form.querySelectorAll('.lab-test:checked')].map((x) => x.value);
  if (!v.patientId) throw new Error(bi('بیمار را انتخاب کنید', 'Select a patient'));
  if (!testIds.length) throw new Error(bi('حداقل یک آزمون انتخاب کنید', 'Select at least one test'));
  await post('/api/labs', { patientId: v.patientId, testIds, priority: v.priority, doctorId: S.user.doctorId || null });
  closeModal();
  toast(bi('درخواست آزمایش ثبت شد', 'Lab order created'), 'ok');
  refresh();
});
