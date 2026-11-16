// Administration: staff accounts, physicians, services, clinic settings and audit trail.
import {
  S, t, bi, esc, can, faNum, money, fmtDate, fmtDateTime, fmtTime, todayISO, isoToCal, addDays,
  roleLabel, get, post, patch, toast, refresh, query, onAction, onForm, openModal, closeModal,
  confirmDlg, emptyState, pager, textField, textArea, selectField, dateField, readDate, formValues,
  badge, doctorName,
} from './lib.js';
import { ic } from './icons.js';

const CLINIC_ROLES = ['admin', 'receptionist', 'doctor', 'nurse', 'lab', 'accountant'];
const WEEKDAYS = [
  [6, 'شنبه', 'Sat'], [0, 'یکشنبه', 'Sun'], [1, 'دوشنبه', 'Mon'], [2, 'سه‌شنبه', 'Tue'],
  [3, 'چهارشنبه', 'Wed'], [4, 'پنجشنبه', 'Thu'], [5, 'جمعه', 'Fri'],
];
const SERVICE_CATS = [['consult', 'ویزیت', 'Consultation'], ['procedure', 'رویه', 'Procedure'], ['diagnostic', 'تشخیصی', 'Diagnostic'], ['lab', 'آزمایشگاه', 'Laboratory'], ['other', 'سایر', 'Other']];
const catLabel = (c) => { const f = SERVICE_CATS.find(([k]) => k === c); return f ? bi(f[1], f[2]) : c; };
const auditState = { page: 1, action: '', entity: '', q: '', from: '', to: '' };
const doctors = () => S.meta?.doctors || [];
const departments = () => S.meta?.departments || [];
const deptName = (id) => { const d = departments().find((x) => x.id === id); return d ? bi(d.nameFa, d.nameEn) : '—'; };

/* ------------------------------------------------------------------- staff */

export async function renderStaff() {
  const users = await get('/api/users');
  return `<div class="page-head"><div class="grow"><h2>${esc(t('adm.users'))}</h2>
      <div class="sub">${esc(bi('حساب‌های کاربری، نقش‌ها و دسترسی‌ها', 'User accounts, roles and access'))}</div></div>
      <div class="head-actions"><button class="btn pri" data-act="staff.new">${ic('userplus')}${esc(bi('کاربر جدید', 'New user'))}</button></div></div>
    <div class="tbl-wrap"><table class="tbl"><thead><tr>
      <th>${esc(bi('نام', 'Name'))}</th><th>${esc(bi('نام کاربری', 'Username'))}</th><th>${esc(bi('نقش', 'Role'))}</th>
      <th>${esc(bi('پزشک مرتبط', 'Linked physician'))}</th><th>${esc(bi('آخرین ورود', 'Last login'))}</th><th>${esc(t('common.status'))}</th><th></th>
    </tr></thead><tbody>${(users || []).map((u) => `<tr>
      <td><b>${esc(bi(u.nameFa, u.nameEn))}</b>${u.id === S.user.id ? ` <span class="tag blue">${esc(bi('شما', 'you'))}</span>` : ''}</td>
      <td class="mono">${esc(u.username)}</td>
      <td>${badge(roleLabel(u.role), u.role === 'admin' ? 'b-violet' : u.role === 'doctor' ? 'b-blue' : 'b-gray')}</td>
      <td class="small">${esc(u.doctorNameFa ? bi(u.doctorNameFa, u.doctorNameEn) : '—')}</td>
      <td class="small">${esc(u.lastLoginAt ? fmtDateTime(u.lastLoginAt) : '—')}</td>
      <td>${u.active ? badge(bi('فعال', 'Active'), 'b-green') : badge(bi('غیرفعال', 'Disabled'), 'b-red')}</td>
      <td><div class="actions">
        <button class="btn xs" data-act="staff.edit" data-id="${esc(u.id)}">${esc(bi('ویرایش', 'Edit'))}</button>
        <button class="btn xs" data-act="staff.password" data-id="${esc(u.id)}" data-name="${esc(bi(u.nameFa, u.nameEn))}">${esc(bi('رمز', 'Password'))}</button>
        <button class="btn xs ${u.active ? 'err' : 'ok'}" data-act="staff.toggle" data-id="${esc(u.id)}" data-active="${u.active ? '1' : '0'}">${esc(u.active ? bi('غیرفعال', 'Disable') : bi('فعال', 'Enable'))}</button>
      </div></td>
    </tr>`).join('')}</tbody></table></div>
    <div class="alert blue mt14">${ic('shield')}<div>${esc(bi('هر کاربر فقط به بخش‌های مجاز نقش خود دسترسی دارد؛ همه اقدامات در گزارش رخدادها ثبت می‌شود.', 'Each account only reaches its role’s permitted areas; every action is written to the audit trail.'))}</div></div>`;
}

function staffModal(user) {
  const editing = Boolean(user);
  openModal({
    title: editing ? bi('ویرایش کاربر', 'Edit user') : bi('کاربر جدید', 'New user'),
    size: '',
    body: `<form data-form="staff.save" ${editing ? `data-id="${esc(user.id)}"` : ''}>
      <div class="form">
        ${textField(bi('نام (فارسی)', 'Name (Persian)'), 'nameFa', user?.nameFa || '', 'f-6', { required: true })}
        ${textField('Name (English)', 'nameEn', user?.nameEn || '', 'f-6')}
        ${selectField(bi('نقش', 'Role'), 'role', CLINIC_ROLES.map((r) => [r, roleLabel(r)]), 'f-6', user?.role || 'receptionist')}
        ${selectField(bi('پزشک مرتبط', 'Linked physician'), 'doctorId', [['', bi('هیچ', 'None')]].concat(doctors().map((d) => [d.id, doctorName(d)])), 'f-6', user?.doctorId || '')}
        ${editing ? '' : textField(bi('نام کاربری', 'Username'), 'username', '', 'f-6', { required: true, dir: 'ltr' })}
        ${editing ? '' : textField(bi('رمز اولیه', 'Initial password'), 'password', '', 'f-6', { type: 'password', required: true, dir: 'ltr' })}
        ${editing ? `<div class="field f-12"><label class="checkline"><input type="checkbox" name="active" ${user.active ? 'checked' : ''}> ${esc(bi('حساب فعال است', 'Account is active'))}</label></div>` : ''}
      </div>
      <div class="mini-note mt8">${esc(bi('رمز باید حداقل ۱۰ نویسه و شامل حرف و رقم باشد.', 'Password must be at least 10 characters and include letters and digits.'))}</div>
      <div class="flex-b mt14"><button class="btn" type="button" data-close>${esc(t('common.cancel'))}</button>
        <button class="btn pri" type="submit">${esc(t('common.save'))}</button></div>
    </form>`,
  });
}

onAction('staff.new', () => staffModal(null));
onAction('staff.edit', async (el) => {
  const users = await get('/api/users');
  const user = (users || []).find((u) => u.id === el.dataset.id);
  if (user) staffModal(user);
});

onForm('staff.save', async (form) => {
  const v = formValues(form);
  const id = form.dataset.id;
  if (id) {
    await patch(`/api/users/${id}`, {
      nameFa: v.nameFa,
      nameEn: v.nameEn || '',
      role: v.role,
      doctorId: v.doctorId || '',
      active: Boolean(v.active),
    });
    toast(t('adm.saved'), 'ok');
  } else {
    await post('/api/users', {
      username: v.username,
      password: v.password,
      role: v.role,
      nameFa: v.nameFa,
      nameEn: v.nameEn || '',
      doctorId: v.doctorId || null,
    });
    toast(bi('کاربر ایجاد شد', 'User created'), 'ok');
  }
  closeModal();
  refresh();
});

onAction('staff.toggle', (el) => {
  const active = el.dataset.active === '1';
  confirmDlg(
    active ? bi('غیرفعال‌سازی حساب', 'Disable account') : bi('فعال‌سازی حساب', 'Enable account'),
    active ? bi('کاربر بلافاصله از سیستم خارج می‌شود.', 'The user is signed out immediately.') : bi('کاربر می‌تواند دوباره وارد شود.', 'The user will be able to sign in again.'),
    async () => {
      await patch(`/api/users/${el.dataset.id}`, { active: !active });
      refresh();
    },
    { danger: active },
  );
});

onAction('staff.password', (el) => {
  openModal({
    title: bi('بازنشانی رمز', 'Reset password'),
    size: 'narrow',
    body: `<form data-form="staff.password" data-id="${esc(el.dataset.id)}">
      <div class="mini-note mb8">${esc(el.dataset.name || '')}</div>
      <div class="form">${textField(bi('رمز جدید', 'New password'), 'password', '', 'f-12', { type: 'password', required: true, dir: 'ltr' })}</div>
      <div class="mini-note mt8">${esc(bi('حداقل ۱۰ نویسه شامل حرف و رقم. تمام نشست‌های کاربر بسته می‌شود.', 'At least 10 characters with letters and digits. All sessions are closed.'))}</div>
      <div class="flex-b mt14"><button class="btn" type="button" data-close>${esc(t('common.cancel'))}</button>
        <button class="btn pri" type="submit">${esc(bi('بازنشانی', 'Reset'))}</button></div>
    </form>`,
  });
});

onForm('staff.password', async (form) => {
  await post(`/api/users/${form.dataset.id}/password`, { password: formValues(form).password });
  closeModal();
  toast(bi('رمز بازنشانی شد', 'Password reset'), 'ok');
});

/* ----------------------------------------------------------------- doctors */

export async function renderDoctors() {
  const list = await get('/api/doctors');
  return `<div class="page-head"><div class="grow"><h2>${esc(t('nav.doctors'))}</h2>
      <div class="sub">${esc(bi('برنامه کاری، تعرفه و شماره نظام پزشکی', 'Working hours, fees and licence numbers'))}</div></div>
      <div class="head-actions"><button class="btn pri" data-act="doc.new">${ic('plus')}${esc(bi('پزشک جدید', 'New physician'))}</button></div></div>
    <div class="tbl-wrap"><table class="tbl"><thead><tr>
      <th>${esc(bi('پزشک', 'Physician'))}</th><th>${esc(bi('تخصص', 'Specialty'))}</th><th>${esc(bi('بخش', 'Department'))}</th>
      <th>${esc(bi('ساعات کاری', 'Hours'))}</th><th>${esc(bi('تعرفه', 'Fee'))}</th><th>${esc(bi('روزهای تعطیل', 'Days off'))}</th><th>${esc(t('common.status'))}</th><th></th>
    </tr></thead><tbody>${list.map((d) => `<tr>
      <td><span class="dot" style="background:${esc(d.color)}"></span><b>${esc(bi(d.nameFa, d.nameEn))}</b>
        <div class="mini-note">${esc(d.license || '')}</div></td>
      <td class="small">${esc(bi(d.specialtyFa, d.specialtyEn))}</td>
      <td class="small">${esc(deptName(d.deptId))}</td>
      <td class="small mono">${esc(d.workStart)}–${esc(d.workEnd)} · ${faNum(d.slotMin)}′</td>
      <td class="num">${esc(money(d.fee))}</td>
      <td class="small">${(d.daysOff || []).map((n) => { const w = WEEKDAYS.find(([k]) => k === n); return w ? bi(w[1], w[2]) : n; }).join(', ') || '—'}</td>
      <td>${d.active ? badge(bi('فعال', 'Active'), 'b-green') : badge(bi('غیرفعال', 'Inactive'), 'b-gray')}</td>
      <td><button class="btn xs" data-act="doc.edit" data-id="${esc(d.id)}">${esc(bi('ویرایش', 'Edit'))}</button></td>
    </tr>`).join('')}</tbody></table></div>`;
}

function doctorModal(doc) {
  const editing = Boolean(doc);
  openModal({
    title: editing ? bi('ویرایش پزشک', 'Edit physician') : bi('پزشک جدید', 'New physician'),
    size: 'wide',
    body: `<form data-form="doc.save" ${editing ? `data-id="${esc(doc.id)}"` : ''}>
      <div class="form">
        ${textField(bi('نام (فارسی)', 'Name (Persian)'), 'nameFa', doc?.nameFa || '', 'f-6', { required: true })}
        ${textField('Name (English)', 'nameEn', doc?.nameEn || '', 'f-6')}
        ${textField(bi('تخصص (فارسی)', 'Specialty (Persian)'), 'specialtyFa', doc?.specialtyFa || '', 'f-6')}
        ${textField('Specialty (English)', 'specialtyEn', doc?.specialtyEn || '', 'f-6')}
        ${textField(bi('شماره نظام پزشکی', 'Licence number'), 'license', doc?.license || '', 'f-4', { dir: 'ltr' })}
        ${selectField(bi('بخش', 'Department'), 'deptId', [['', '—']].concat(departments().map((d) => [d.id, bi(d.nameFa, d.nameEn)])), 'f-4', doc?.deptId || '')}
        ${textField(bi('رنگ', 'Colour'), 'color', doc?.color || '#0e7490', 'f-4', { type: 'color' })}
        ${textField(bi('شروع کار', 'Work start'), 'workStart', doc?.workStart || '09:00', 'f-3', { type: 'time', dir: 'ltr' })}
        ${textField(bi('پایان کار', 'Work end'), 'workEnd', doc?.workEnd || '17:00', 'f-3', { type: 'time', dir: 'ltr' })}
        ${textField(bi('طول نوبت (دقیقه)', 'Slot minutes'), 'slotMin', doc?.slotMin || 30, 'f-3', { type: 'number', min: 5, max: 120, dir: 'ltr' })}
        ${textField(bi('تعرفه ویزیت (ریال)', 'Visit fee (IRR)'), 'fee', doc?.fee || 0, 'f-3', { type: 'number', min: 0, dir: 'ltr' })}
        <div class="field f-12"><label>${esc(bi('روزهای تعطیل', 'Days off'))}</label>
          <div class="chip-tests">${WEEKDAYS.map(([n, fa, en]) => `<label><input type="checkbox" class="dayoff" value="${n}" ${(doc?.daysOff || []).includes(n) ? 'checked' : ''}> ${esc(bi(fa, en))}</label>`).join('')}</div></div>
        <div class="field f-12"><label class="checkline"><input type="checkbox" name="active" ${doc ? (doc.active ? 'checked' : '') : 'checked'}> ${esc(bi('پزشک فعال است', 'Physician is active'))}</label></div>
      </div>
      <div class="flex-b mt14"><button class="btn" type="button" data-close>${esc(t('common.cancel'))}</button>
        <button class="btn pri" type="submit">${esc(t('common.save'))}</button></div>
    </form>`,
  });
}

onAction('doc.new', () => doctorModal(null));
onAction('doc.edit', async (el) => {
  const list = await get('/api/doctors');
  const doc = list.find((d) => d.id === el.dataset.id);
  if (doc) doctorModal(doc);
});

onForm('doc.save', async (form) => {
  const v = formValues(form);
  const payload = {
    nameFa: v.nameFa,
    nameEn: v.nameEn || '',
    specialtyFa: v.specialtyFa || '',
    specialtyEn: v.specialtyEn || '',
    license: v.license || '',
    deptId: v.deptId || null,
    workStart: v.workStart,
    workEnd: v.workEnd,
    slotMin: Number(v.slotMin) || 30,
    fee: Number(v.fee) || 0,
    daysOff: [...form.querySelectorAll('.dayoff:checked')].map((x) => Number(x.value)),
    color: v.color || '#0e7490',
    active: Boolean(v.active),
  };
  const id = form.dataset.id;
  if (id) await patch(`/api/doctors/${id}`, payload);
  else await post('/api/doctors', payload);
  closeModal();
  S.meta = await get('/api/meta');
  toast(t('adm.saved'), 'ok');
  refresh();
});

/* ---------------------------------------------------------------- services */

export async function renderServices() {
  const list = await get('/api/services');
  return `<div class="page-head"><div class="grow"><h2>${esc(t('adm.services'))}</h2>
      <div class="sub">${esc(bi('کاتالوگ خدمات، تعرفه و مدت زمان', 'Service catalogue, pricing and duration'))}</div></div>
      <div class="head-actions"><button class="btn pri" data-act="svc.new">${ic('plus')}${esc(bi('خدمت جدید', 'New service'))}</button></div></div>
    <div class="tbl-wrap"><table class="tbl"><thead><tr>
      <th>${esc(bi('کد', 'Code'))}</th><th>${esc(bi('خدمت', 'Service'))}</th><th>${esc(bi('دسته', 'Category'))}</th>
      <th>${esc(bi('مدت', 'Duration'))}</th><th>${esc(bi('تعرفه', 'Price'))}</th><th>${esc(t('common.status'))}</th><th></th>
    </tr></thead><tbody>${list.map((s) => `<tr>
      <td class="mono">${esc(s.code)}</td>
      <td><b>${esc(bi(s.nameFa, s.nameEn))}</b></td>
      <td>${badge(catLabel(s.category), 'b-blue')}</td>
      <td class="num">${faNum(s.duration)}′</td>
      <td class="num">${esc(money(s.price))}</td>
      <td>${s.active ? badge(bi('فعال', 'Active'), 'b-green') : badge(bi('غیرفعال', 'Inactive'), 'b-gray')}</td>
      <td><button class="btn xs" data-act="svc.edit" data-id="${esc(s.id)}">${esc(bi('ویرایش', 'Edit'))}</button></td>
    </tr>`).join('')}</tbody></table></div>`;
}

function serviceModal(svc) {
  const editing = Boolean(svc);
  openModal({
    title: editing ? bi('ویرایش خدمت', 'Edit service') : bi('خدمت جدید', 'New service'),
    size: '',
    body: `<form data-form="svc.save" ${editing ? `data-id="${esc(svc.id)}"` : ''}>
      <div class="form">
        ${textField(bi('کد', 'Code'), 'code', svc?.code || '', 'f-4', { required: true, dir: 'ltr', placeholder: 'CONS-01' })}
        ${textField(bi('نام (فارسی)', 'Name (Persian)'), 'nameFa', svc?.nameFa || '', 'f-4', { required: true })}
        ${textField('Name (English)', 'nameEn', svc?.nameEn || '', 'f-4')}
        ${selectField(bi('دسته', 'Category'), 'category', SERVICE_CATS.map(([k, fa, en]) => [k, bi(fa, en)]), 'f-4', svc?.category || 'consult')}
        ${textField(bi('تعرفه (ریال)', 'Price (IRR)'), 'price', svc?.price ?? 0, 'f-4', { type: 'number', min: 0, dir: 'ltr' })}
        ${textField(bi('مدت (دقیقه)', 'Duration (minutes)'), 'duration', svc?.duration ?? 30, 'f-4', { type: 'number', min: 1, max: 480, dir: 'ltr' })}
        ${selectField(bi('بخش', 'Department'), 'deptId', [['', '—']].concat(departments().map((d) => [d.id, bi(d.nameFa, d.nameEn)])), 'f-4', svc?.deptId || '')}
        <div class="field f-8"><label class="checkline" style="margin-top:26px"><input type="checkbox" name="active" ${svc ? (svc.active ? 'checked' : '') : 'checked'}> ${esc(bi('خدمت فعال است', 'Service is active'))}</label></div>
      </div>
      <div class="flex-b mt14"><button class="btn" type="button" data-close>${esc(t('common.cancel'))}</button>
        <button class="btn pri" type="submit">${esc(t('common.save'))}</button></div>
    </form>`,
  });
}

onAction('svc.new', () => serviceModal(null));
onAction('svc.edit', async (el) => {
  const list = await get('/api/services');
  const svc = list.find((s) => s.id === el.dataset.id);
  if (svc) serviceModal(svc);
});

onForm('svc.save', async (form) => {
  const v = formValues(form);
  const payload = {
    code: v.code,
    nameFa: v.nameFa,
    nameEn: v.nameEn || '',
    category: v.category,
    price: Number(v.price) || 0,
    duration: Number(v.duration) || 30,
    deptId: v.deptId || null,
    active: Boolean(v.active),
  };
  const id = form.dataset.id;
  if (id) await patch(`/api/services/${id}`, payload);
  else await post('/api/services', payload);
  closeModal();
  S.meta = await get('/api/meta');
  toast(t('adm.saved'), 'ok');
  refresh();
});

/* ---------------------------------------------------------------- settings */

export async function renderSettings() {
  const settings = await get('/api/settings');
  const v = (k) => settings[k] ?? '';
  return `<div class="page-head"><div class="grow"><h2>${esc(t('nav.settings'))}</h2>
      <div class="sub">${esc(bi('اطلاعات کلینیک، تقویم و پیکربندی نوبت‌دهی', 'Clinic identity, calendar and scheduling configuration'))}</div></div>
      <div class="head-actions"><a class="btn" href="/api/admin/backup" download>${ic('download')}${esc(bi('پشتیبان JSON', 'JSON backup'))}</a></div></div>
    <form data-form="settings.save">
      <div class="grid2">
        <div class="card"><h3>${ic('building')}${esc(bi('اطلاعات کلینیک', 'Clinic identity'))}</h3>
          <div class="form">
            ${textField(bi('نام (فارسی)', 'Name (Persian)'), 'clinic_name_fa', v('clinic_name_fa'), 'f-12')}
            ${textField('Name (English)', 'clinic_name_en', v('clinic_name_en'), 'f-12')}
            ${textField(bi('تلفن', 'Phone'), 'clinic_phone', v('clinic_phone'), 'f-6', { dir: 'ltr' })}
            ${textField(bi('آدرس (فارسی)', 'Address (Persian)'), 'clinic_address_fa', v('clinic_address_fa'), 'f-6')}
            ${textField('Address (English)', 'clinic_address_en', v('clinic_address_en'), 'f-12')}
          </div>
        </div>
        <div class="card"><h3>${ic('calendar')}${esc(bi('نوبت‌دهی و مالی', 'Scheduling & billing'))}</h3>
          <div class="form">
            ${selectField(bi('تقویم نمایش', 'Display calendar'), 'calendar', [['jalali', bi('شمسی', 'Jalali')], ['gregorian', bi('میلادی', 'Gregorian')]], 'f-6', v('calendar') || 'jalali')}
            ${textField(bi('طول نوبت پیش‌فرض (دقیقه)', 'Default slot (minutes)'), 'slot_minutes', v('slot_minutes') || 30, 'f-6', { type: 'number', min: 5, max: 120, dir: 'ltr' })}
            ${textField(bi('شروع کار', 'Work start'), 'work_start', v('work_start') || '09:00', 'f-6', { type: 'time', dir: 'ltr' })}
            ${textField(bi('پایان کار', 'Work end'), 'work_end', v('work_end') || '17:00', 'f-6', { type: 'time', dir: 'ltr' })}
            ${textField(bi('حداکثر تخفیف (%)', 'Maximum discount (%)'), 'discount_limit_pct', v('discount_limit_pct') || 0, 'f-6', { type: 'number', min: 0, max: 100, dir: 'ltr' })}
          </div>
          <div class="alert blue mt14">${ic('shield')}<div>${esc(bi('تغییرات در گزارش رخدادها ثبت می‌شود.', 'Changes are recorded in the audit trail.'))}</div></div>
        </div>
      </div>
      <div class="flex-b mt14"><span></span><button class="btn pri" type="submit">${ic('check')}${esc(t('common.save'))}</button></div>
    </form>
    <div class="card mt14"><h3>${ic('download')}${esc(bi('پشتیبان‌گیری', 'Backup'))}</h3>
      <p class="small muted">${esc(bi('خروجی JSON از داده‌های بالینی و مالی (بدون رمزها) برای آرشیو یا انتقال.', 'A JSON export of clinical and financial data (no password hashes) for archiving or migration.'))}</p>
      <a class="btn" href="/api/admin/backup" download>${ic('download')}${esc(bi('دریافت پشتیبان', 'Download backup'))}</a>
      <div class="mini-note mt8">${esc(bi('قبل از به‌روزرسانی نسخه، از فایل پایگاه‌داده نیز نسخه پشتیبان بگیرید.', 'Also copy the database file before upgrading the application.'))}</div>
    </div>`;
}

onForm('settings.save', async (form) => {
  const v = formValues(form);
  await patch('/api/settings', v);
  S.meta = await get('/api/meta');
  toast(t('adm.saved'), 'ok');
  refresh();
});

/* ------------------------------------------------------------------- audit */

export async function renderAudit() {
  const data = await get(`/api/audit${query({ ...auditState, perPage: 60 })}`);
  const items = data.items || [];
  const outcomeColor = (o) => (o === 'success' ? 'b-green' : o === 'denied' ? 'b-red' : 'b-amber');
  return `<div class="page-head"><div class="grow"><h2>${esc(t('nav.audit'))}</h2>
      <div class="sub">${esc(bi('گزارش فقط‌افزودنی همه اقدامات کاربران', 'Append-only record of every user action'))}</div></div>
      <div class="head-actions"><a class="btn" href="/api/admin/backup" download>${ic('download')}${esc(bi('پشتیبان', 'Backup'))}</a></div></div>
    <div class="card"><form data-form="audit.filter" class="inline-fields">
      <div class="field grow"><label>${esc(t('common.search'))}</label><input name="q" value="${esc(auditState.q)}" placeholder="${esc(bi('کاربر، جزئیات یا شناسه', 'User, details or id'))}"></div>
      <div class="field"><label>${esc(bi('اقدام', 'Action'))}</label><input name="action" value="${esc(auditState.action)}" dir="ltr"></div>
      <div class="field"><label>${esc(bi('موجودیت', 'Entity'))}</label><input name="entity" value="${esc(auditState.entity)}" dir="ltr"></div>
      <div class="field"><label>${esc(bi('از', 'From'))}</label><input type="date" name="from" value="${esc(auditState.from)}"></div>
      <div class="field"><label>${esc(bi('تا', 'To'))}</label><input type="date" name="to" value="${esc(auditState.to)}"></div>
      <button class="btn pri" type="submit">${ic('search')}${esc(bi('جست‌وجو', 'Search'))}</button>
      <button class="btn" type="button" data-act="audit.clear">${esc(bi('پاک‌کردن', 'Clear'))}</button>
    </form></div>
    <div class="tbl-wrap mt14"><table class="tbl"><thead><tr>
      <th>${esc(bi('زمان', 'Time'))}</th><th>${esc(bi('کاربر', 'User'))}</th><th>${esc(bi('اقدام', 'Action'))}</th>
      <th>${esc(bi('موجودیت', 'Entity'))}</th><th>${esc(bi('جزئیات', 'Details'))}</th><th>${esc(bi('نتیجه', 'Outcome'))}</th><th>${esc(bi('IP', 'IP'))}</th>
    </tr></thead><tbody>${items.length ? items.map((a) => `<tr>
      <td class="small mono">${esc(fmtDateTime(a.at))}</td>
      <td><b>${esc(a.user || '—')}</b><div class="mini-note">${esc(roleLabel(a.role))}</div></td>
      <td class="mono small">${esc(a.action)}</td>
      <td class="small">${esc(a.entity)}${a.entityId ? `<div class="mini-note mono">${esc(a.entityId.slice(0, 10))}…</div>` : ''}</td>
      <td class="small">${esc(a.details || '—')}</td>
      <td>${badge(a.outcome, outcomeColor(a.outcome))}</td>
      <td class="mono small">${esc(a.ip || '—')}</td>
    </tr>`).join('') : `<tr><td colspan="7">${emptyState('history', bi('رخدادی یافت نشد', 'No audit entries found'))}</td></tr>`}</tbody></table>
    ${pager(data.meta, 'audit.page')}</div>`;
}

onForm('audit.filter', (form) => {
  const v = formValues(form);
  Object.assign(auditState, { q: v.q || '', action: v.action || '', entity: v.entity || '', from: v.from || '', to: v.to || '', page: 1 });
  refresh();
});
onAction('audit.clear', () => {
  Object.assign(auditState, { q: '', action: '', entity: '', from: '', to: '', page: 1 });
  refresh();
});
onAction('audit.page', (el) => {
  const page = Number(el.dataset.page);
  if (page > 0) {
    auditState.page = page;
    refresh();
  }
});
