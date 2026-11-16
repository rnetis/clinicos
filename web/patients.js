// Patient registry: search, registration with duplicate review, full profile.
import {
  S, t, bi, esc, can, faNum, money, fmtDate, fmtDateTime, fmtAge, isoToCal, calToISO,
  patientName, get, post, patch, del, go, toast, refresh, query, onAction, onForm,
  openModal, closeModal, confirmDlg, emptyState, spinner, pager, field, textField,
  textArea, selectField, dateField, readDate, formValues, patientPicker, badge,
  apptBadge, invBadge, labBadge, encBadge, flagBadge, doctorName, ago,
} from './lib.js';
import { ic } from './icons.js';

const state = { q: '', status: '', page: 1, tab: 'overview' };

const CATEGORY_KEYS = [
  ['general', 'عمومی', 'General'],
  ['child', 'کودک', 'Child'],
  ['senior', 'سالمند', 'Senior'],
  ['vip', 'ویژه', 'VIP'],
];
const SOURCE_KEYS = [
  ['walk_in', 'مراجعه حضوری', 'Walk-in'],
  ['phone', 'تلفن', 'Phone'],
  ['portal', 'درگاه بیمار', 'Patient portal'],
  ['referral', 'ارجاع', 'Referral'],
  ['emergency', 'اورژانس', 'Emergency'],
];
const BLOOD = ['', 'A+', 'A-', 'B+', 'B-', 'AB+', 'AB-', 'O+', 'O-'];

const insurerOptions = () => (S.meta?.insurers || []).map((i) => [i.id, bi(i.nameFa, i.nameEn) + ` (${faNum(i.cover)}%)`]);
const doctorOptions = (empty = true) => (empty ? [['', '—']] : []).concat((S.meta?.doctors || []).filter((d) => d.active).map((d) => [d.id, doctorName(d)]));

function categoryLabel(value) {
  const found = CATEGORY_KEYS.find(([k]) => k === value);
  return found ? bi(found[1], found[2]) : value;
}
function sourceLabel(value) {
  const found = SOURCE_KEYS.find(([k]) => k === value);
  return found ? bi(found[1], found[2]) : value;
}
function statusBadge(status) {
  const map = { active: ['b-green', 'فعال', 'Active'], inactive: ['b-gray', 'غیرفعال', 'Inactive'], archived: ['b-gray', 'بایگانی', 'Archived'], pending_verify: ['b-amber', 'در انتظار تأیید', 'Pending verification'] };
  const found = map[status] || ['b-gray', status, status];
  return badge(bi(found[1], found[2]), found[0]);
}
function genderLabel(g) {
  return g === 'M' ? t('common.male') : g === 'F' ? t('common.female') : '—';
}

/* ------------------------------------------------------------------- list */

export async function renderList() {
  const data = await get(`/api/patients${query({ q: state.q, status: state.status, page: state.page, perPage: 25 })}`);
  const rows = data.items || [];
  const page = data.meta;

  return `<div class="page-head">
      <div class="grow"><h2>${esc(bi('بیماران', 'Patients'))}</h2>
        <div class="sub">${esc(bi('جستجو در پرونده‌ها، کد ملی، تلفن و کد بیمار', 'Search records, national ID, phone and patient code'))}</div></div>
      <div class="head-actions">
        ${can('patient.register') ? `<button class="btn pri" data-link="#/patients/new">${ic('userplus')}${esc(t('nav.register'))}</button>` : ''}
      </div>
    </div>
    <div class="card">
      <form data-form="patients.search" class="inline-fields">
        <div class="field" style="flex:2"><label>${esc(t('common.search'))}</label>
          <input name="q" value="${esc(state.q)}" placeholder="${esc(bi('نام، کد، کد ملی یا تلفن', 'Name, code, national ID or phone'))}"></div>
        <div class="field" style="flex:1"><label>${esc(t('common.status'))}</label>
          <select name="status" data-change="patients.status">
            <option value="">${esc(t('common.all'))}</option>
            ${['active', 'inactive', 'archived', 'pending_verify'].map((s) => `<option value="${s}" ${state.status === s ? 'selected' : ''}>${esc(statusLabel(s))}</option>`).join('')}
          </select></div>
        <button class="btn pri" type="submit">${ic('search')}${esc(t('common.search'))}</button>
        <button class="btn" type="button" data-act="patients.clear">${esc(t('common.clear'))}</button>
      </form>
    </div>
    <div class="tbl-wrap mt14"><table class="tbl">
      <thead><tr>
        <th>${esc(bi('کد', 'Code'))}</th><th>${esc(bi('بیمار', 'Patient'))}</th><th>${esc(bi('سن/جنسیت', 'Age / sex'))}</th>
        <th>${esc(bi('تماس', 'Contact'))}</th><th>${esc(bi('بیمه', 'Insurance'))}</th><th>${esc(bi('پزشک', 'Doctor'))}</th>
        <th>${esc(bi('مانده', 'Balance'))}</th><th>${esc(t('common.status'))}</th><th></th>
      </tr></thead>
      <tbody>${rows.length ? rows.map((p) => `<tr>
        <td class="num">${esc(p.code)}</td>
        <td><a class="rowlink" href="#/patient/${esc(p.id)}">${esc(patientName(p))}</a>
          <div class="mini-note">${esc(bi(p.cityFa, p.cityEn) || '')}${p.lastVisit ? ' · ' + esc(bi('آخرین ویزیت', 'last visit')) + ' ' + esc(fmtDate(p.lastVisit, { short: true })) : ''}</div></td>
        <td>${esc(fmtAge(p.dob))} · ${esc(genderLabel(p.gender))}</td>
        <td class="mono">${esc(p.phone || '—')}</td>
        <td class="small">${esc(bi(p.insurerNameFa, p.insurerNameEn) || bi('بدون بیمه', 'Self-pay'))}</td>
        <td class="small">${esc(bi(p.doctorNameFa, p.doctorNameEn) || '—')}</td>
        <td class="num">${p.outstanding ? `<b style="color:var(--err)">${esc(money(p.outstanding))}</b>` : '—'}</td>
        <td>${statusBadge(p.status)}</td>
        <td><a class="btn xs" href="#/patient/${esc(p.id)}">${esc(bi('پرونده', 'Open'))}</a></td>
      </tr>`).join('') : `<tr><td colspan="9">${emptyState('users', bi('بیماری یافت نشد', 'No patients found'))}</td></tr>`}
      </tbody></table>
      ${pager(page, 'patients.page')}
    </div>`;
}

function statusLabel(s) {
  return s === 'active' ? bi('فعال', 'Active') : s === 'inactive' ? bi('غیرفعال', 'Inactive')
    : s === 'archived' ? bi('بایگانی', 'Archived') : bi('در انتظار تأیید', 'Pending verification');
}

onForm('patients.search', async (form) => {
  const data = formValues(form);
  state.q = data.q || '';
  state.status = data.status || '';
  state.page = 1;
  refresh();
});

onAction('patients.status', (el) => {
  state.status = el.value;
  state.page = 1;
  refresh();
});

onAction('patients.clear', () => {
  state.q = '';
  state.status = '';
  state.page = 1;
  refresh();
});

onAction('patients.page', (el) => {
  state.page = Number(el.dataset.page) || 1;
  refresh();
});

/* -------------------------------------------------------------- register */

function patientForm(p = {}, opts = {}) {
  const isEdit = Boolean(p.id);
  const nowCal = isoToCal(new Date().toISOString().slice(0, 10));
  return `<form data-form="${isEdit ? 'patient.update' : 'patient.create'}" ${isEdit ? `data-id="${esc(p.id)}"` : ''}>
    <div class="form">
      <div class="form-sect">${esc(bi('هویت', 'Identity'))}</div>
      ${textField(bi('نام (فارسی)', 'First name (Persian)'), 'firstNameFa', p.firstNameFa || '', 'f-3', { required: true })}
      ${textField(bi('نام خانوادگی (فارسی)', 'Last name (Persian)'), 'lastNameFa', p.lastNameFa || '', 'f-3', { required: true })}
      ${textField(bi('نام (انگلیسی)', 'First name (English)'), 'firstNameEn', p.firstNameEn || '', 'f-3')}
      ${textField(bi('نام خانوادگی (انگلیسی)', 'Last name (English)'), 'lastNameEn', p.lastNameEn || '', 'f-3')}
      ${textField(bi('نام پدر', "Father's name"), 'fatherName', p.fatherName || '', 'f-3')}
      ${dateField('dob', p.dob || '', 'f-3', { label: bi('تاریخ تولد', 'Date of birth'), range: [nowCal.y - 100, nowCal.y] })}
      ${selectField(bi('جنسیت', 'Sex'), 'gender', [['', '—'], ['F', t('common.female')], ['M', t('common.male')]], 'f-3', p.gender || '')}
      ${selectField(bi('گروه خونی', 'Blood group'), 'bloodGroup', BLOOD.map((b) => [b, b || '—']), 'f-3', p.bloodGroup || '')}
      ${textField(bi('کد ملی', 'National ID'), 'nationalId', p.nationalId || '', 'f-4', { dir: 'ltr' })}
      ${textField(bi('شماره گذرنامه', 'Passport'), 'passport', p.passport || '', 'f-4', { dir: 'ltr' })}
      ${selectField(bi('دسته', 'Category'), 'category', CATEGORY_KEYS.map(([k, fa, en]) => [k, bi(fa, en)]), 'f-4', p.category || 'general')}

      <div class="form-sect">${esc(bi('تماس و نشانی', 'Contact & address'))}</div>
      ${textField(bi('تلفن همراه', 'Mobile phone'), 'phone', p.phone || '', 'f-4', { type: 'tel', dir: 'ltr' })}
      ${textField(bi('تلفن دوم', 'Second phone'), 'phone2', p.phone2 || '', 'f-4', { type: 'tel', dir: 'ltr' })}
      ${textField(bi('ایمیل', 'Email'), 'email', p.email || '', 'f-4', { type: 'email', dir: 'ltr' })}
      ${textField(bi('شهر (فارسی)', 'City (Persian)'), 'cityFa', p.cityFa || 'تهران', 'f-4')}
      ${textField(bi('شهر (انگلیسی)', 'City (English)'), 'cityEn', p.cityEn || 'Tehran', 'f-4')}
      ${textField(bi('کد پستی', 'Postal code'), 'postal', p.postal || '', 'f-4', { dir: 'ltr' })}
      ${textField(bi('نشانی (فارسی)', 'Address (Persian)'), 'addressFa', p.addressFa || '', 'f-6')}
      ${textField(bi('نشانی (انگلیسی)', 'Address (English)'), 'addressEn', p.addressEn || '', 'f-6')}

      <div class="form-sect">${esc(bi('تماس اضطراری', 'Emergency contact'))}</div>
      ${textField(bi('نام', 'Name'), 'emergencyName', p.emergencyName || '', 'f-4')}
      ${textField(bi('نسبت', 'Relationship'), 'emergencyRel', p.emergencyRel || '', 'f-4')}
      ${textField(bi('تلفن', 'Phone'), 'emergencyPhone', p.emergencyPhone || '', 'f-4', { dir: 'ltr' })}

      <div class="form-sect">${esc(bi('بیمه و پذیرش', 'Insurance & registration'))}</div>
      ${selectField(bi('بیمه', 'Insurer'), 'insuranceProviderId', insurerOptions(), 'f-4', p.insuranceProviderId || 'ins-none')}
      ${textField(bi('شماره بیمه', 'Policy number'), 'policyNo', p.policyNo || '', 'f-4', { dir: 'ltr' })}
      ${selectField(bi('پزشک معالج', 'Assigned doctor'), 'assignedDoctorId', doctorOptions(), 'f-4', p.assignedDoctorId || '')}
      ${selectField(bi('منبع مراجعه', 'Source'), 'source', SOURCE_KEYS.map(([k, fa, en]) => [k, bi(fa, en)]), 'f-4', p.source || 'walk_in')}
      ${selectField(t('common.status'), 'status', [['active', statusLabel('active')], ['inactive', statusLabel('inactive')], ['pending_verify', statusLabel('pending_verify')], ['archived', statusLabel('archived')]], 'f-4', p.status || 'active')}

      <div class="form-sect">${esc(bi('سابقه پزشکی', 'Medical history'))}</div>
      ${textArea(bi('سابقه جراحی', 'Surgical history'), 'historySurgical', p.history?.surgical || '', 'f-4', { rows: 2 })}
      ${textArea(bi('سابقه خانوادگی', 'Family history'), 'historyFamily', p.history?.family || '', 'f-4', { rows: 2 })}
      ${textArea(bi('سبک زندگی', 'Lifestyle'), 'historyLifestyle', p.history?.lifestyle || '', 'f-4', { rows: 2 })}
      ${textArea(t('common.notes'), 'notes', p.notes || '', 'f-12', { rows: 2 })}
    </div>
    <div class="flex-b mt14">
      <button class="btn" type="button" data-close>${esc(t('common.cancel'))}</button>
      <button class="btn pri" type="submit">${ic('check')}${esc(isEdit ? t('common.save') : bi('ثبت بیمار', 'Register patient'))}</button>
    </div>
  </form>`;
}

export async function renderRegister() {
  const cal = isoToCal(new Date().toISOString().slice(0, 10));
  return `<div class="page-head"><div class="grow"><h2>${esc(t('nav.register'))}</h2>
      <div class="sub">${esc(bi('تکرار پرونده به‌صورت خودکار بررسی می‌شود', 'Duplicate records are checked automatically'))}</div></div>
      <div class="head-actions"><button class="btn" data-link="#/patients">${ic('users')}${esc(bi('فهرست بیماران', 'Patient list'))}</button></div></div>
    <div class="card">
      <div class="form" style="margin-bottom:10px">
        <div class="f-12"><label class="small fs-bold">${esc(bi('پیش از ثبت، جستجو کنید', 'Search before registering'))}</label>
          <div class="inline-fields">
            <div class="field" style="flex:1"><input id="reg-search" placeholder="${esc(bi('نام یا تلفن بیمار', 'Name or phone'))}" autocomplete="off" data-change="reg.search"><div class="fnote" id="reg-search-note"></div></div>
          </div>
          <div id="reg-search-results" class="stack" style="margin-top:6px"></div>
        </div>
      </div>
      ${patientForm({ ...cal, status: 'active', source: 'walk_in', cityFa: 'تهران', cityEn: 'Tehran', insuranceProviderId: 'ins-none', category: 'general' })}
    </div>`;
}

onAction('reg.search', async (el) => {
  const box = document.getElementById('reg-search-results');
  if (!box) return;
  const term = el.value.trim();
  if (term.length < 2) {
    box.innerHTML = '';
    return;
  }
  const data = await get(`/api/patients?q=${encodeURIComponent(term)}&perPage=5`);
  if (!data.items.length) {
    box.innerHTML = `<div class="mini-note">${esc(bi('موردی یافت نشد — می‌توانید ادامه دهید', 'No match — you can continue'))}</div>`;
    return;
  }
  box.innerHTML = `<div class="alert amber">${ic('alert')}<div>${esc(bi('شاید این بیمار قبلاً ثبت شده باشد:', 'This patient may already exist:'))}</div></div>` +
    data.items.map((p) => `<div class="dup-item"><div><strong>${esc(patientName(p))}</strong>
      <div class="mini-note">${esc(p.code)} · ${esc(p.phone || '')} · ${esc(fmtAge(p.dob))}</div></div>
      <a class="btn sm right" href="#/patient/${esc(p.id)}">${esc(bi('باز کردن پرونده', 'Open record'))}</a></div>`).join('');
});

function patientPayload(form) {
  const v = formValues(form);
  return {
    firstNameFa: v.firstNameFa,
    firstNameEn: v.firstNameEn || '',
    lastNameFa: v.lastNameFa,
    lastNameEn: v.lastNameEn || '',
    fatherName: v.fatherName || '',
    dob: readDate(form, 'dob'),
    gender: v.gender || '',
    nationalId: v.nationalId || '',
    passport: v.passport || '',
    phone: v.phone || '',
    phone2: v.phone2 || '',
    email: v.email || '',
    addressFa: v.addressFa || '',
    addressEn: v.addressEn || '',
    cityFa: v.cityFa || '',
    cityEn: v.cityEn || '',
    postal: v.postal || '',
    emergencyName: v.emergencyName || '',
    emergencyRel: v.emergencyRel || '',
    emergencyPhone: v.emergencyPhone || '',
    insuranceProviderId: v.insuranceProviderId || '',
    policyNo: v.policyNo || '',
    category: v.category || 'general',
    status: v.status || 'active',
    source: v.source || 'walk_in',
    assignedDoctorId: v.assignedDoctorId || '',
    bloodGroup: v.bloodGroup || '',
    notes: v.notes || '',
    historySurgical: v.historySurgical || '',
    historyFamily: v.historyFamily || '',
    historyLifestyle: v.historyLifestyle || '',
  };
}

function showDuplicates(payload, dupes) {
  openModal({
    title: bi('پرونده تکراری احتمالی', 'Possible duplicate record'),
    body: `<div class="alert amber">${ic('alert')}<div>${esc(bi('الگوی زیر با بیمار موجود مطابقت دارد. اگر مطمئن هستید، ثبت ادامه یابد.', 'The details match an existing patient. Continue only if this is a different person.'))}</div></div>
      ${dupes.map((d) => `<div class="dup-item"><div><strong>${esc(bi(d.name.fa + ' ' + d.lastName.fa, d.name.en + ' ' + d.lastName.en))}</strong>
        <div class="why">${esc((d.reasons || []).join(' · '))}</div>
        <div class="mini-note">${esc(d.code)} · ${esc(fmtAge(d.dob))}</div></div>
        <a class="btn sm right" href="#/patient/${esc(d.id)}" data-close>${esc(bi('باز کردن', 'Open'))}</a></div>`).join('')}
      <div class="mini-note">${esc(bi('ثبت با وجود هشدار فقط با تأیید صریح انجام می‌شود و در گزارش رخدادها ثبت خواهد شد.', 'Forcing creation is recorded in the audit log.'))}</div>`,
    footer: `<button class="btn" data-close type="button">${esc(t('common.cancel'))}</button>
             <button class="btn warn" type="button" data-act="patient.force" data-payload="${esc(JSON.stringify(payload))}">${esc(bi('ثبت با وجود تکرار', 'Save anyway'))}</button>`,
  });
}

onForm('patient.create', async (form) => {
  const payload = patientPayload(form);
  try {
    const res = await post('/api/patients', payload);
    toast(bi('بیمار ثبت شد: ', 'Patient registered: ') + res.code, 'ok');
    closeModal();
    state.page = 1;
    go(`#/patient/${res.id}`);
  } catch (err) {
    if (err.status === 409 && err.payload?.error === 'duplicate') {
      showDuplicates(payload, err.payload.duplicates || []);
      return;
    }
    throw err;
  }
});

onAction('patient.force', async (el) => {
  const payload = JSON.parse(el.dataset.payload);
  const res = await post('/api/patients', { ...payload, force: true });
  toast(bi('بیمار با هشدار تکرار ثبت شد', 'Patient saved despite duplicate warning'), 'warn');
  closeModal();
  go(`#/patient/${res.id}`);
});

onForm('patient.update', async (form) => {
  const id = form.dataset.id;
  const payload = patientPayload(form);
  await patch(`/api/patients/${id}`, payload);
  closeModal();
  toast(t('adm.saved'), 'ok');
  refresh();
});

onAction('patient.edit', async (el) => {
  const id = el.dataset.id;
  const p = await get(`/api/patients/${id}`);
  openModal({
    title: bi('ویرایش پرونده', 'Edit patient'),
    size: 'wide',
    body: patientForm(p),
  });
});

/* ---------------------------------------------------------------- profile */

export async function renderProfile(id) {
  if (!id) return `<div class="card">${emptyState('alert', bi('شناسه بیمار نامعتبر است', 'Invalid patient id'))}</div>`;
  const p = await get(`/api/patients/${id}`);
  const clinical = can('patient.viewClinical');
  const cal = isoToCal(new Date().toISOString().slice(0, 10));
  const tabs = [
    ['overview', bi('نمای کلی', 'Overview')],
    ['personal', bi('اطلاعات', 'Demographics')],
    ...(clinical ? [['medical', bi('پزشکی', 'Medical')]] : []),
    ...(clinical ? [['visits', bi('ویزیت‌ها', 'Visits')]] : []),
    ...(clinical ? [['prescriptions', bi('نسخه‌ها', 'Prescriptions')]] : []),
    ...(clinical ? [['labs', bi('آزمایش‌ها', 'Labs')]] : []),
    ['billing', bi('مالی', 'Billing')],
    ['followups', bi('پیگیری‌ها', 'Follow-ups')],
  ];
  if (!tabs.some(([k]) => k === state.tab)) state.tab = 'overview';

  let html = `<div class="page-head" style="margin-bottom:10px">
    <button class="btn" data-link="#/patients">${ic('users')}${esc(bi('بیماران', 'Patients'))}</button>
  </div>
  <div class="pp-head">
    <div class="avatar">${esc(patientName(p).trim()[0] || '?')}</div>
    <div>
      <h2>${esc(patientName(p))}</h2>
      <div class="sub">
        <span>${esc(p.code)}</span>
        <span>${esc(fmtAge(p.dob))} · ${esc(genderLabel(p.gender))}</span>
        <span>${esc(p.phone || '—')}</span>
        <span>${esc(bi(p.insurerNameFa, p.insurerNameEn) || bi('بدون بیمه', 'Self-pay'))}</span>
        ${p.bloodGroup ? `<span class="tag red">${esc(p.bloodGroup)}</span>` : ''}
        ${statusBadge(p.status)}
      </div>
    </div>
    <div class="pp-actions">
      ${clinical ? `<button class="btn pri" data-act="consult.start" data-patient="${esc(p.id)}" data-doctor="${esc(p.assignedDoctorId || S.user.doctorId || '')}">${ic('notes')}${esc(bi('شروع ویزیت', 'Start visit'))}</button>` : ''}
      ${can('appt.manage') ? `<button class="btn" data-act="appt.new" data-patient="${esc(p.id)}">${ic('calendar')}${esc(bi('نوبت', 'Appointment'))}</button>` : ''}
      ${can('patient.editAdmin') ? `<button class="btn" data-act="patient.edit" data-id="${esc(p.id)}">${ic('edit')}${esc(t('common.edit'))}</button>` : ''}
      ${can('patient.merge') ? `<button class="btn" data-act="patient.merge" data-id="${esc(p.id)}" data-name="${esc(patientName(p))}">${ic('merge')}${esc(bi('ادغام', 'Merge'))}</button>` : ''}
    </div>
  </div>`;

  if (p.alerts?.length) {
    html += `<div class="alert red mt14">${ic('alert')}<div>${p.alerts.map((a) => esc(bi(a.textFa, a.textEn))).join(' · ')}</div></div>`;
  }
  if (p.mergedInto) {
    html += `<div class="alert amber mt14">${ic('merge')}<div>${esc(bi('این پرونده در پرونده دیگری ادغام شده است.', 'This record was merged into another record.'))}
      <a href="#/patient/${esc(p.mergedInto)}">${esc(bi('مشاهده پرونده اصلی', 'Open surviving record'))}</a></div></div>`;
  }

  html += `<div class="tabs mt14">${tabs.map(([key, label]) => `<button class="${state.tab === key ? 'on' : ''}" data-act="patient.tab" data-tab="${key}">${esc(label)}</button>`).join('')}</div>`;
  html += `<div id="patient-tab">${profileTab(p, appliedTab(state.tab, p, clinical), clinical)}</div>`;
  return html;
}

function appliedTab(tab, p, clinical) {
  return tab;
}

onAction('patient.tab', (el) => {
  state.tab = el.dataset.tab;
  refresh();
});

function kv(label, value) {
  return `<dt>${esc(label)}</dt><dd>${value || '—'}</dd>`;
}

function overviewTab(p) {
  const c = p.counts || {};
  return `<div class="grid2">
    <div class="card"><h3>${ic('user')}${esc(bi('خلاصه پرونده', 'Record summary'))}</h3>
      <dl class="kv">
        ${kv(bi('کد بیمار', 'Patient code'), esc(p.code))}
        ${kv(bi('تاریخ تولد', 'Date of birth'), esc(fmtDate(p.dob)) + ' · ' + esc(fmtAge(p.dob)))}
        ${kv(bi('کد ملی', 'National ID'), `<span class="mono">${esc(p.nationalId || '—')}</span>`)}
        ${kv(bi('تلفن', 'Phone'), `<span class="mono">${esc(p.phone || '—')}</span>`)}
        ${kv(bi('ایمیل', 'Email'), esc(p.email || '—'))}
        ${kv(bi('بیمه', 'Insurance'), esc(bi(p.insurerNameFa, p.insurerNameEn) || '—') + (p.policyNo ? ' · ' + esc(p.policyNo) : ''))}
        ${kv(bi('پزشک معالج', 'Assigned doctor'), esc(bi(p.doctorNameFa, p.doctorNameEn) || '—'))}
        ${kv(bi('تاریخ ثبت', 'Registered'), esc(fmtDate(p.createdAt)))}
      </dl>
      ${p.notes ? `<div class="note-box mt14">${esc(p.notes)}</div>` : ''}
    </div>
    <div class="card"><h3>${ic('chart')}${esc(bi('شمارش‌ها', 'Activity'))}</h3>
      <div class="kpi-grid" style="grid-template-columns:1fr 1fr">
        <div class="stat"><div class="lbl">${ic('calendar')}${esc(bi('نوبت‌ها', 'Appointments'))}</div><div class="val">${faNum(c.appointments || 0)}</div></div>
        <div class="stat"><div class="lbl">${ic('notes')}${esc(bi('ویزیت‌ها', 'Visits'))}</div><div class="val">${faNum(c.encounters || 0)}</div></div>
        <div class="stat"><div class="lbl">${ic('flask')}${esc(bi('آزمایش‌ها', 'Labs'))}</div><div class="val">${faNum(c.labs || 0)}</div></div>
        <div class="stat ${p.outstanding ? 'err' : ''}"><div class="lbl">${ic('money')}${esc(bi('مانده', 'Balance'))}</div><div class="val">${esc(money(p.outstanding || 0))}</div></div>
      </div>
      ${(p.alerts || []).length ? `<div class="tag-row mt8">${p.alerts.map((a) => `<span class="tag red">${esc(bi(a.textFa, a.textEn))}</span>`).join('')}</div>` : ''}
    </div>
  </div>
  <div class="grid2 mt14">
    <div class="card"><h3>${ic('calendar')}${esc(bi('آخرین نوبت‌ها', 'Recent appointments'))}</h3>
      ${(p.recentAppointments || []).length ? `<div class="stack">${p.recentAppointments.map((a) => `<div class="flex-b" style="border-bottom:1px solid var(--line);padding-bottom:6px">
        <div><b>${esc(fmtDate(a.date))} · ${esc(fmtTime(a.time))}</b><div class="mini-note">${esc(bi(a.doctorName.fa, a.doctorName.en))} · ${esc(bi(a.reason.fa, a.reason.en))}</div></div>
        ${apptBadge(a.status)}</div>`).join('')}</div>` : emptyState('calendar', bi('نوبتی ثبت نشده', 'No appointments'))}
    </div>
    <div class="card"><h3>${ic('clock')}${esc(bi('پیگیری‌ها', 'Follow-ups'))}</h3>
      ${(p.followUps || []).length ? `<div class="stack">${p.followUps.map((f) => `<div class="flex-b" style="border-bottom:1px solid var(--line);padding-bottom:6px">
        <div><b>${esc(fmtDate(f.due))}</b><div class="mini-note">${esc(bi(f.reason.fa, f.reason.en))}</div></div>
        <span class="tag ${f.status === 'pending' && f.due < new Date().toISOString().slice(0, 10) ? 'red' : 'gray'}">${esc(fuStatus(f))}</span></div>`).join('')}</div>` : emptyState('clock', bi('پیگیری فعالی وجود ندارد', 'No follow-ups'))}
    </div>
  </div>`;
}

function fuStatus(f) {
  const map = { pending: ['در انتظار', 'Pending'], scheduled: ['زمان‌بندی‌شده', 'Scheduled'], contacted: ['تماس گرفته شد', 'Contacted'], completed: ['انجام شد', 'Completed'], cancelled: ['لغو شده', 'Cancelled'] };
  const m = map[f.status] || [f.status, f.status];
  return bi(m[0], m[1]);
}

function personalTab(p) {
  return `<div class="grid2">
    <div class="card"><h3>${ic('user')}${esc(bi('مشخصات', 'Demographics'))}</h3>
      <dl class="kv">
        ${kv(bi('نام', 'Name'), esc(patientName(p)))}
        ${kv(bi('نام پدر', "Father's name"), esc(p.fatherName || '—'))}
        ${kv(bi('تاریخ تولد', 'Date of birth'), esc(fmtDate(p.dob)))}
        ${kv(bi('جنسیت', 'Sex'), esc(genderLabel(p.gender)))}
        ${kv(bi('گروه خونی', 'Blood group'), esc(p.bloodGroup || '—'))}
        ${kv(bi('کد ملی', 'National ID'), `<span class="mono">${esc(p.nationalId || '—')}</span>`)}
        ${kv(bi('گذرنامه', 'Passport'), `<span class="mono">${esc(p.passport || '—')}</span>`)}
        ${kv(bi('دسته', 'Category'), esc(categoryLabel(p.category)))}
        ${kv(bi('منبع مراجعه', 'Source'), esc(sourceLabel(p.source)))}
      </dl>
    </div>
    <div class="card"><h3>${ic('building')}${esc(bi('تماس و نشانی', 'Contact'))}</h3>
      <dl class="kv">
        ${kv(bi('تلفن همراه', 'Mobile'), `<span class="mono">${esc(p.phone || '—')}</span>`)}
        ${kv(bi('تلفن دوم', 'Second phone'), `<span class="mono">${esc(p.phone2 || '—')}</span>`)}
        ${kv(bi('ایمیل', 'Email'), esc(p.email || '—'))}
        ${kv(bi('شهر', 'City'), esc(bi(p.cityFa, p.cityEn) || '—'))}
        ${kv(bi('نشانی', 'Address'), esc(bi(p.addressFa, p.addressEn) || '—'))}
        ${kv(bi('کد پستی', 'Postal code'), `<span class="mono">${esc(p.postal || '—')}</span>`)}
        ${kv(bi('تماس اضطراری', 'Emergency contact'), esc([p.emergencyName, p.emergencyRel, p.emergencyPhone].filter(Boolean).join(' · ') || '—'))}
      </dl>
    </div>
  </div>
  <div class="card mt14"><h3>${ic('notes')}${esc(bi('سابقه', 'History'))}</h3>
    <div class="grid3">
      <div><div class="small muted">${esc(bi('جراحی', 'Surgical'))}</div><div class="note-box mt8">${esc(p.history?.surgical || '—')}</div></div>
      <div><div class="small muted">${esc(bi('خانوادگی', 'Family'))}</div><div class="note-box mt8">${esc(p.history?.family || '—')}</div></div>
      <div><div class="small muted">${esc(bi('سبک زندگی', 'Lifestyle'))}</div><div class="note-box mt8">${esc(p.history?.lifestyle || '—')}</div></div>
    </div>
  </div>`;
}

function medicalTab(p) {
  const v = p.latestVitals;
  let bmi = null;
  if (v?.data?.height && v?.data?.weight) {
    const h = Number(v.data.height) / 100;
    bmi = (Number(v.data.weight) / (h * h)).toFixed(1);
  }
  return `<div class="grid2">
    <div class="card"><h3>${ic('alert')}${esc(bi('حساسیت‌ها', 'Allergies'))}</h3>
      ${(p.allergies || []).length ? `<div class="stack">${p.allergies.map((a) => `<div class="flex-b" style="border-bottom:1px solid var(--line);padding-bottom:6px">
        <div><b>${esc(bi(a.nameFa, a.nameEn))}</b><div class="mini-note">${esc(a.note || '')} · ${esc(fmtDate(a.notedAt))}</div></div>
        <div class="flex"><span class="tag ${a.severity === 'severe' ? 'red' : a.severity === 'moderate' ? 'amber' : 'gray'}">${esc(a.severity)}</span>
        <button class="btn xs" data-act="patient.allergy.del" data-id="${esc(p.id)}" data-sub="${esc(a.id)}">${ic('trash', 'ic-s')}</button></div>
      </div>`).join('')}</div>` : emptyState('alert', bi('حساسیتی ثبت نشده', 'No allergies recorded'))}
      <button class="btn sm mt8" data-act="patient.allergy.add" data-id="${esc(p.id)}">${ic('plus')}${esc(bi('افزودن حساسیت', 'Add allergy'))}</button>
    </div>
    <div class="card"><h3>${ic('clipboard')}${esc(bi('تشخیص‌های فعال', 'Active conditions'))}</h3>
      ${(p.conditions || []).length ? `<div class="stack">${p.conditions.map((c) => `<div class="flex-b" style="border-bottom:1px solid var(--line);padding-bottom:6px">
        <div><b>${esc(bi(c.nameFa, c.nameEn))}</b><div class="mini-note">${esc(c.code || '')} · ${esc(fmtDate(c.notedAt))}</div></div>
        <div class="flex">${c.chronic ? badge(bi('مزمن', 'Chronic'), 'b-violet') : ''}
        <button class="btn xs" data-act="patient.condition.del" data-id="${esc(p.id)}" data-sub="${esc(c.id)}">${ic('trash', 'ic-s')}</button></div>
      </div>`).join('')}</div>` : emptyState('clipboard', bi('تشخیصی ثبت نشده', 'No conditions recorded'))}
      <button class="btn sm mt8" data-act="patient.condition.add" data-id="${esc(p.id)}">${ic('plus')}${esc(bi('افزودن تشخیص', 'Add condition'))}</button>
    </div>
  </div>
  <div class="grid2 mt14">
    <div class="card"><h3>${ic('pill')}${esc(bi('داروهای مصرفی', 'Current medications'))}</h3>
      ${(p.meds || []).length ? `<div class="tag-row">${p.meds.map((m) => `<span class="tag">${esc(bi(m.textFa, m.textEn))} <button class="btn xs" data-act="patient.med.del" data-id="${esc(p.id)}" data-sub="${esc(m.id)}">×</button></span>`).join('')}</div>` : emptyState('pill', bi('دارویی ثبت نشده', 'No medications recorded'))}
      <button class="btn sm mt8" data-act="patient.med.add" data-id="${esc(p.id)}">${ic('plus')}${esc(bi('افزودن دارو', 'Add medication'))}</button>
    </div>
    <div class="card"><h3>${ic('heart')}${esc(bi('آخرین علائم حیاتی', 'Latest vitals'))}</h3>
      ${v ? `<div class="enc-vitals">${vitalBox(bi('فشار خون', 'Blood pressure'), v.data.bp)}
        ${vitalBox(bi('ضربان', 'Heart rate'), v.data.hr)}
        ${vitalBox(bi('تنفس', 'Respiratory'), v.data.rr)}
        ${vitalBox(bi('دما', 'Temperature'), v.data.temp)}
        ${vitalBox(bi('اشباع اکسیژن', 'SpO₂'), v.data.spo2)}
        ${vitalBox(bi('قد', 'Height'), v.data.height)}
        ${vitalBox(bi('وزن', 'Weight'), v.data.weight)}
        ${vitalBox(bi('قند خون', 'Glucose'), v.data.glucose)}
        ${bmi ? `<div class="bmi-box"><div class="small">BMI</div><b>${faNum(bmi)}</b></div>` : ''}
      </div><div class="mini-note mt8">${esc(fmtDateTime(v.at))}</div>` : emptyState('heart', bi('علائم حیاتی ثبت نشده', 'No vitals recorded'))}
    </div>
  </div>
  <div class="card mt14"><h3>${ic('alert')}${esc(bi('هشدارهای پرونده', 'Record alerts'))}</h3>
    ${(p.alerts || []).length ? `<div class="stack">${p.alerts.map((a) => `<div class="flex-b"><span class="tag red">${esc(bi(a.textFa, a.textEn))}</span>
      <button class="btn xs" data-act="patient.alert.del" data-id="${esc(p.id)}" data-sub="${esc(a.id)}">${ic('trash', 'ic-s')}</button></div>`).join('')}</div>` : `<div class="mini-note">${esc(bi('هشداری ثبت نشده', 'No alerts'))}</div>`}
    <button class="btn sm mt8" data-act="patient.alert.add" data-id="${esc(p.id)}">${ic('plus')}${esc(bi('افزودن هشدار', 'Add alert'))}</button>
  </div>`;
}

function vitalBox(label, value) {
  return `<div class="bmi-box" style="background:var(--slate-l)"><div class="small muted">${esc(label)}</div><b style="font-size:15px">${esc(value ?? '—')}</b></div>`;
}

function visitsTab(p) {
  return `<div class="card"><h3>${ic('notes')}${esc(bi('ویزیت‌ها و اسناد بالینی', 'Visits and clinical documents'))}</h3>
    ${(p.encounters || []).length ? `<div class="tbl-wrap"><table class="tbl"><thead><tr>
      <th>${esc(bi('تاریخ', 'Date'))}</th><th>${esc(bi('پزشک', 'Doctor'))}</th><th>${esc(bi('شکایت اصلی', 'Chief complaint'))}</th>
      <th>${esc(bi('تشخیص', 'Diagnosis'))}</th><th>${esc(t('common.status'))}</th><th></th></tr></thead>
      <tbody>${p.encounters.map((e) => `<tr>
        <td>${esc(fmtDate(e.date))}</td>
        <td>${esc(bi(e.doctorName.fa, e.doctorName.en))}</td>
        <td class="small">${esc(bi(e.chief?.fa, e.chief?.en))}</td>
        <td class="small">${esc((e.diagnoses || []).map((d) => d.code).join(', ') || '—')}</td>
        <td>${encBadge(e.status)}</td>
        <td><a class="btn xs" href="#/consult/${esc(e.id)}">${esc(bi('مشاهده', 'Open'))}</a></td>
      </tr>`).join('')}</tbody></table></div>` : emptyState('notes', bi('ویزیتی ثبت نشده', 'No visits recorded'))}
  </div>`;
}

function prescriptionsTab(p) {
  const rows = [];
  (p.encounters || []).forEach((e) => {
    (e.prescriptions || []).forEach((rx) => rows.push({ e, rx }));
  });
  return `<div class="card"><h3>${ic('pill')}${esc(bi('نسخه‌های صادرشده', 'Prescriptions'))}</h3>
    ${rows.length ? `<div class="tbl-wrap"><table class="tbl"><thead><tr>
      <th>${esc(bi('تاریخ', 'Date'))}</th><th>${esc(bi('دارو', 'Drug'))}</th><th>${esc(bi('دوز', 'Dose'))}</th>
      <th>${esc(bi('فراوانی', 'Frequency'))}</th><th>${esc(bi('مدت', 'Duration'))}</th><th>${esc(bi('تعداد', 'Qty'))}</th></tr></thead>
      <tbody>${rows.map(({ e, rx }) => `<tr>
        <td>${esc(fmtDate(e.date))}</td>
        <td>${esc(bi(rx.drug?.fa, rx.drug?.en))}</td>
        <td class="num">${esc(rx.dose || '')}</td>
        <td>${esc(t('freq.' + (rx.freq || 'OD')))}</td>
        <td>${faNum(rx.durDays || 0)} ${esc(t('common.days'))}</td>
        <td>${faNum(rx.qty || 0)}</td>
      </tr>`).join('')}</tbody></table></div>` : emptyState('pill', bi('نسخه‌ای یافت نشد', 'No prescriptions found'))}
  </div>`;
}

function labsTab(p) {
  return `<div class="card"><h3>${ic('flask')}${esc(bi('آزمایش‌های بیمار', 'Laboratory orders'))}</h3>
    ${(p.labOrders || []).length ? `<div class="stack">${p.labOrders.map((o) => `<div class="card" style="box-shadow:none;margin:0">
      <div class="flex-b"><div class="flex">${labBadge(o.status)} <b>${esc(fmtDate(o.date))}</b>
        ${o.priority !== 'routine' ? `<span class="tag ${o.priority === 'stat' ? 'red' : 'amber'}">${esc(o.priority)}</span>` : ''}</div>
        <span class="mini-note">${faNum((o.results || []).length)} ${esc(bi('نتیجه', 'results'))}</span></div>
      ${(o.results || []).length ? `<div class="tbl-wrap mt8"><table class="tbl"><thead><tr>
        <th>${esc(bi('آزمون', 'Test'))}</th><th>${esc(bi('نتیجه', 'Value'))}</th><th>${esc(bi('واحد', 'Unit'))}</th><th>${esc(bi('محدوده', 'Reference'))}</th><th>${esc(bi('وضعیت', 'Flag'))}</th></tr></thead>
        <tbody>${o.results.map((r) => `<tr><td>${esc(bi(r.name.fa, r.name.en))}</td>
          <td class="num"><b>${esc(r.value)}</b></td><td class="small">${esc(r.unit || '')}</td>
          <td class="small mono">${esc(r.ref || '')}</td><td>${flagBadge(r.flag)}</td></tr>`).join('')}</tbody></table></div>` : `<div class="mini-note mt8">${esc(bi('در انتظار نتیجه', 'Awaiting results'))}</div>`}
    </div>`).join('')}</div>` : emptyState('flask', bi('آزمایشی ثبت نشده', 'No lab orders'))}
  </div>`;
}

function billingTab(p) {
  return `<div class="card"><h3>${ic('receipt')}${esc(bi('صورتحساب‌ها', 'Invoices'))}</h3>
    <div class="flex-b mb8"><span>${esc(bi('مانده کل', 'Total outstanding'))}</span><b style="color:${p.outstanding ? 'var(--err)' : 'var(--ok)'}">${esc(money(p.outstanding || 0))}</b></div>
    ${(p.invoices || []).length ? `<div class="tbl-wrap"><table class="tbl"><thead><tr>
      <th>${esc(bi('شماره', 'No.'))}</th><th>${esc(bi('تاریخ', 'Date'))}</th><th>${esc(bi('مبلغ', 'Total'))}</th>
      <th>${esc(bi('پرداخت', 'Paid'))}</th><th>${esc(bi('مانده', 'Due'))}</th><th>${esc(t('common.status'))}</th><th></th></tr></thead>
      <tbody>${p.invoices.map((i) => `<tr><td class="num">${esc(i.no)}</td><td>${esc(fmtDate(i.date))}</td>
        <td class="num">${esc(money(i.due))}</td><td class="num">${esc(money(i.paid))}</td>
        <td class="num"><b>${esc(money(i.outstanding))}</b></td><td>${invBadge(i.status)}</td>
        <td><a class="btn xs" href="#/invoice/${esc(i.id)}">${esc(bi('مشاهده', 'Open'))}</a></td></tr>`).join('')}</tbody></table></div>`
      : emptyState('receipt', bi('صورتحسابی ثبت نشده', 'No invoices'))}
  </div>`;
}

function followupsTab(p) {
  return `<div class="card"><h3>${ic('clock')}${esc(bi('پیگیری‌ها', 'Follow-ups'))}</h3>
    ${(p.followUps || []).length ? `<div class="tbl-wrap"><table class="tbl"><thead><tr>
      <th>${esc(bi('سررسید', 'Due'))}</th><th>${esc(bi('علت', 'Reason'))}</th><th>${esc(bi('اولویت', 'Priority'))}</th>
      <th>${esc(t('common.status'))}</th><th></th></tr></thead>
      <tbody>${p.followUps.map((f) => `<tr><td>${esc(fmtDate(f.due))}</td>
        <td class="small">${esc(bi(f.reason.fa, f.reason.en))}</td>
        <td>${priorityTag(f.priority)}</td><td>${esc(fuStatus(f))}</td>
        <td>${f.status === 'pending' || f.status === 'scheduled' ? `<button class="btn xs ok" data-act="fu.status" data-id="${esc(f.id)}" data-status="completed">${esc(bi('انجام شد', 'Complete'))}</button>` : ''}</td></tr>`).join('')}</tbody></table></div>`
      : emptyState('clock', bi('پیگیری‌ای ثبت نشده', 'No follow-ups'))}
  </div>`;
}

function priorityTag(priority) {
  const map = { routine: ['gray', 'معمولی', 'Routine'], high: ['amber', 'بالا', 'High'], urgent: ['red', 'فوری', 'Urgent'] };
  const m = map[priority] || map.routine;
  return `<span class="tag ${m[0]}">${esc(bi(m[1], m[2]))}</span>`;
}

function profileTab(p, tab, clinical) {
  switch (tab) {
    case 'personal': return personalTab(p);
    case 'medical': return clinical ? medicalTab(p) : personalTab(p);
    case 'visits': return visitsTab(p);
    case 'prescriptions': return prescriptionsTab(p);
    case 'labs': return labsTab(p);
    case 'billing': return billingTab(p);
    case 'followups': return followupsTab(p);
    default: return overviewTab(p);
  }
}

/* --------------------------------------------------------- sub-resources */

function subModal(id, title, body, formName, extra = '') {
  openModal({
    title,
    size: 'narrow',
    body: `<form data-form="${formName}" data-id="${esc(id)}">
      <div class="form">${body}</div>
      <div class="flex-b mt14">
        <button class="btn" type="button" data-close>${esc(t('common.cancel'))}</button>
        <button class="btn pri" type="submit">${esc(t('common.save'))}</button>
      </div></form>${extra}`,
  });
}

onAction('patient.allergy.add', (el) => {
  subModal(el.dataset.id, bi('افزودن حساسیت', 'Add allergy'), `
    ${textField(bi('نام دارو/ماده', 'Allergen'), 'nameFa', '', 'f-6', { required: true })}
    ${textField(bi('نام انگلیسی', 'English name'), 'nameEn', '', 'f-6')}
    ${selectField(bi('شدت', 'Severity'), 'severity', [['mild', bi('خفیف', 'Mild')], ['moderate', bi('متوسط', 'Moderate')], ['severe', bi('شدید', 'Severe')]], 'f-6', 'moderate')}
    ${textField(bi('برچسب', 'Tag'), 'tag', '', 'f-6')}
    ${textArea(bi('توضیح واکنش', 'Reaction note'), 'note', '', 'f-12', { rows: 2 })}`, 'patient.allergy.create');
});

onForm('patient.allergy.create', async (form) => {
  const v = formValues(form);
  await post(`/api/patients/${form.dataset.id}/allergies`, { nameFa: v.nameFa, nameEn: v.nameEn || '', severity: v.severity, tag: v.tag || '', note: v.note || '' });
  closeModal();
  toast(t('adm.saved'), 'ok');
  refresh();
});

onAction('patient.allergy.del', (el) => confirmDlg(bi('حذف حساسیت', 'Delete allergy'), bi('این مورد حذف شود؟', 'Remove this allergy?'), async () => {
  await del(`/api/patients/${el.dataset.id}/allergies/${el.dataset.sub}`);
  refresh();
}, { danger: true }));

onAction('patient.condition.add', (el) => {
  const options = (S.meta?.icd10 || []).map((c) => [c.code, `${c.code} · ${bi(c.titleFa, c.titleEn)}`]);
  subModal(el.dataset.id, bi('افزودن تشخیص', 'Add condition'), `
    ${selectField(bi('کد ICD-10', 'ICD-10 code'), 'code', [['', '—']].concat(options), 'f-12')}
    ${textField(bi('عنوان (در صورت نبود کد)', 'Title (if no code)'), 'nameFa', '', 'f-6')}
    ${textField(bi('عنوان انگلیسی', 'English title'), 'nameEn', '', 'f-6')}
    <div class="field f-12"><label class="checkline"><input type="checkbox" name="chronic"> ${esc(bi('بیماری مزمن', 'Chronic condition'))}</label></div>`, 'patient.condition.create');
});

onForm('patient.condition.create', async (form) => {
  const v = formValues(form);
  await post(`/api/patients/${form.dataset.id}/conditions`, {
    code: v.code || '', nameFa: v.nameFa || '', nameEn: v.nameEn || '', chronic: Boolean(v.chronic),
  });
  closeModal();
  toast(t('adm.saved'), 'ok');
  refresh();
});

onAction('patient.condition.del', (el) => confirmDlg(bi('حذف تشخیص', 'Delete condition'), bi('این تشخیص حذف شود؟', 'Remove this condition?'), async () => {
  await del(`/api/patients/${el.dataset.id}/conditions/${el.dataset.sub}`);
  refresh();
}, { danger: true }));

onAction('patient.med.add', (el) => {
  subModal(el.dataset.id, bi('افزودن داروی مصرفی', 'Add medication'), `
    ${textField(bi('توضیح دارو', 'Medication'), 'textFa', '', 'f-12', { required: true })}
    ${textField(bi('English', 'English'), 'textEn', '', 'f-12')}`, 'patient.med.create');
});

onForm('patient.med.create', async (form) => {
  const v = formValues(form);
  await post(`/api/patients/${form.dataset.id}/meds`, { textFa: v.textFa, textEn: v.textEn || '' });
  closeModal();
  refresh();
});

onAction('patient.med.del', async (el) => {
  await del(`/api/patients/${el.dataset.id}/meds/${el.dataset.sub}`);
  refresh();
});

onAction('patient.alert.add', (el) => {
  subModal(el.dataset.id, bi('افزودن هشدار پرونده', 'Add record alert'), `
    ${textField(bi('متن هشدار', 'Alert text'), 'textFa', '', 'f-12', { required: true })}
    ${textField('English', 'textEn', '', 'f-12')}`, 'patient.alert.create');
});

onForm('patient.alert.create', async (form) => {
  const v = formValues(form);
  await post(`/api/patients/${form.dataset.id}/alerts`, { textFa: v.textFa, textEn: v.textEn || '' });
  closeModal();
  refresh();
});

onAction('patient.alert.del', async (el) => {
  await del(`/api/patients/${el.dataset.id}/alerts/${el.dataset.sub}`);
  refresh();
});

/* ------------------------------------------------------------------ merge */

onAction('patient.merge', (el) => {
  openModal({
    title: bi('ادغام پرونده', 'Merge patient record'),
    size: 'narrow',
    body: `<div class="alert amber">${ic('alert')}<div>${esc(bi('همه سوابق پرونده ', 'All records of '))}<b>${esc(el.dataset.name)}</b>${esc(bi(' به پروندهٔ مقصد منتقل می‌شود و این پرونده بایگانی خواهد شد. این عمل قابل بازگشت نیست.', ' will move to the target record and this record will be archived. This cannot be undone.'))}</div></div>
      <form data-form="patient.merge" data-id="${esc(el.dataset.id)}">
        ${patientPicker('targetId', bi('پروندهٔ مقصد', 'Target record'), 'f-12')}
        <div class="flex-b mt14">
          <button class="btn" type="button" data-close>${esc(t('common.cancel'))}</button>
          <button class="btn err" type="submit">${ic('merge')}${esc(bi('ادغام', 'Merge'))}</button>
        </div>
      </form>`,
  });
});

onForm('patient.merge', async (form) => {
  const v = formValues(form);
  if (!v.targetId) throw new Error(bi('پروندهٔ مقصد را انتخاب کنید', 'Select a target record'));
  await post(`/api/patients/${form.dataset.id}/merge`, { targetId: v.targetId, reason: 'manual merge' });
  closeModal();
  toast(bi('پرونده‌ها ادغام شدند', 'Records merged'), 'ok');
  go(`#/patient/${v.targetId}`);
});

/* ---------------------------------------------------------------- followup */

onAction('fu.status', async (el) => {
  const status = el.dataset.status;
  await patch(`/api/followups/${el.dataset.id}`, { status });
  toast(t('adm.saved'), 'ok');
  refresh();
});
