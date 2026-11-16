// Scheduling: appointment worklist, booking with live slot availability, calendar,
// patient queue and nurse triage.
import {
  S, t, bi, esc, can, faNum, fmtDate, fmtTime, fmtDateTime, fmtAge, todayISO, addDays,
  weekday, isoToCal, calToISO, calMonths, calMonthLen, patientName, doctorName,
  get, post, patch, go, toast, refresh, query, onAction, onForm, openModal, closeModal,
  confirmDlg, emptyState, pager, field, textField, textArea, selectField, dateField,
  readDate, formValues, patientPicker, badge, apptBadge, queueBadge, money,
} from './lib.js';
import { ic } from './icons.js';

const apptState = { date: '', doctorId: '', status: '' };
const calState = { y: null, m: null };
const queueState = { date: '', doctorId: '' };

const TYPE_KEYS = [
  ['consultation', 'مشاوره', 'Consultation'],
  ['first_visit', 'ویزیت اول', 'First visit'],
  ['follow_up', 'پیگیری', 'Follow-up'],
  ['emergency', 'اورژانسی', 'Emergency'],
  ['procedure', 'رویه', 'Procedure'],
  ['telehealth', 'از راه دور', 'Telehealth'],
];
const SOURCE_KEYS = [
  ['walk_in', 'حضوری', 'Walk-in'],
  ['phone', 'تلفن', 'Phone'],
  ['portal', 'درگاه', 'Portal'],
  ['referral', 'ارجاع', 'Referral'],
  ['emergency', 'اورژانس', 'Emergency'],
];
const labelOf = (list, key) => {
  const found = list.find(([k]) => k === key);
  return found ? bi(found[1], found[2]) : key;
};
const doctorOptions = (includeAll = true) => (includeAll ? [['', bi('همه پزشکان', 'All doctors')]] : [])
  .concat((S.meta?.doctors || []).filter((d) => d.active || includeAll).map((d) => [d.id, doctorName(d)]));

async function openConsult({ patientId, doctorId, appointmentId, queueEntryId }) {
  const res = await post('/api/encounters', {
    patientId,
    doctorId: doctorId || S.user.doctorId || '',
    appointmentId: appointmentId || null,
    queueEntryId: queueEntryId || null,
  });
  go(`#/consult/${res.id}`);
}

/* ------------------------------------------------------------ appointments */

export async function renderAppointments() {
  const params = new URLSearchParams(location.hash.split('?')[1] || '');
  if (params.get('date')) apptState.date = params.get('date');
  if (!apptState.date) apptState.date = todayISO();
  const date = apptState.date;

  const data = await get(`/api/appointments${query({ date, doctorId: apptState.doctorId, status: apptState.status, perPage: 200 })}`);
  const rows = data.items || [];

  return `<div class="page-head">
      <div class="grow"><h2>${esc(bi('نوبت‌ها', 'Appointments'))}</h2>
        <div class="sub">${esc(bi('مدیریت نوبت‌های روز با کنترل تداخل', 'Daily scheduling with conflict control'))} · ${esc(fmtDate(date))}</div></div>
      <div class="head-actions">
        <button class="btn pri" data-act="appt.new">${ic('plus')}${esc(bi('نوبت جدید', 'New appointment'))}</button>
        <button class="btn" data-link="#/calendar">${ic('calendar')}${esc(t('nav.calendar'))}</button>
      </div></div>

    <div class="card">
      <div class="inline-fields">
        <div class="field"><label>${esc(bi('تاریخ', 'Date'))}</label>
          <input type="date" name="date" value="${esc(date)}" data-change="appt.date"></div>
        <div class="field"><label>${esc(bi('پزشک', 'Doctor'))}</label>
          <select data-change="appt.doctor">${doctorOptions().map(([v, l]) => `<option value="${esc(v)}" ${apptState.doctorId === v ? 'selected' : ''}>${esc(l)}</option>`).join('')}</select></div>
        <div class="field"><label>${esc(t('common.status'))}</label>
          <select data-change="appt.filter">
            <option value="">${esc(t('common.all'))}</option>
            ${['requested', 'confirmed', 'checked_in', 'waiting', 'in_progress', 'completed', 'cancelled', 'no_show'].map((s) => `<option value="${s}" ${apptState.status === s ? 'selected' : ''}>${esc(t('status.' + s))}</option>`).join('')}
          </select></div>
        <button class="btn" type="button" data-act="appt.today">${esc(bi('امروز', 'Today'))}</button>
        <div class="right flex">
          <button class="btn" type="button" data-act="appt.move" data-delta="-1">‹ ${esc(bi('روز قبل', 'Previous'))}</button>
          <button class="btn" type="button" data-act="appt.move" data-delta="1">${esc(bi('روز بعد', 'Next'))} ›</button>
        </div>
      </div>
    </div>

    ${rows.length ? `<div class="tbl-wrap mt14"><table class="tbl"><thead><tr>
      <th>${esc(bi('زمان', 'Time'))}</th><th>${esc(bi('بیمار', 'Patient'))}</th><th>${esc(bi('پزشک', 'Doctor'))}</th>
      <th>${esc(bi('نوع', 'Type'))}</th><th>${esc(bi('علت', 'Reason'))}</th><th>${esc(t('common.status'))}</th><th>${esc(bi('اقدام', 'Actions'))}</th>
    </tr></thead><tbody>${rows.map((a) => `<tr>
      <td class="num"><b>${esc(fmtTime(a.time))}</b><div class="mini-note">${faNum(a.duration)}′</div></td>
      <td><a class="rowlink" href="#/patient/${esc(a.patientId)}">${esc(bi(a.patientNameFa, a.patientNameEn))}</a>
        <div class="mini-note">${esc(a.patientCode || '')} · ${esc(a.patientPhone || '')}</div></td>
      <td class="small">${esc(bi(a.doctorNameFa, a.doctorNameEn))}</td>
      <td class="small">${esc(labelOf(TYPE_KEYS, a.type))}</td>
      <td class="small">${esc(bi(a.reasonFa, a.reasonEn))}</td>
      <td>${apptBadge(a.status)}${a.queueNumber ? `<div class="mini-note">${esc(bi('صف', 'queue'))} ${faNum(a.queueNumber)}</div>` : ''}</td>
      <td><div class="actions">
        ${a.status === 'requested' ? `<button class="btn xs" data-act="appt.status" data-id="${esc(a.id)}" data-status="confirmed">${esc(bi('تأیید', 'Confirm'))}</button>` : ''}
        ${can('queue.manage') && ['requested', 'confirmed', 'arrived'].includes(a.status) ? `<button class="btn xs ok" data-act="appt.checkin" data-id="${esc(a.id)}">${esc(bi('پذیرش/صف', 'Check in'))}</button>` : ''}
        ${can('appointment') || can('clinical.write') ? '' : ''}
        ${can('clinical.write') && ['checked_in', 'waiting'].includes(a.status) ? `<button class="btn xs pri" data-act="appt.consult" data-id="${esc(a.id)}" data-patient="${esc(a.patientId)}" data-doctor="${esc(a.doctorId)}">${esc(bi('شروع ویزیت', 'Start visit'))}</button>` : ''}
        ${['in_progress'].includes(a.status) ? `<button class="btn xs ok" data-act="appt.status" data-id="${esc(a.id)}" data-status="completed">${esc(bi('پایان', 'Complete'))}</button>` : ''}
        ${['confirmed', 'checked_in', 'waiting'].includes(a.status) ? `<button class="btn xs" data-act="appt.noshow" data-id="${esc(a.id)}">${esc(bi('عدم حضور', 'No-show'))}</button>` : ''}
        ${['requested', 'confirmed', 'checked_in', 'waiting'].includes(a.status) ? `<button class="btn xs err" data-act="appt.cancel" data-id="${esc(a.id)}">${esc(t('common.cancel'))}</button>` : ''}
        <button class="btn xs" data-act="appt.new" data-patient="${esc(a.patientId)}">${ic('plus', 'ic-s')}</button>
      </div></td>
    </tr>`).join('')}</tbody></table></div>` : `<div class="card mt14">${emptyState('calendar', bi('برای این روز نوبتی ثبت نشده است', 'No appointments for this day'))}</div>`}`;
}

onAction('appt.date', (el) => {
  apptState.date = el.value || todayISO();
  refresh();
});
onAction('appt.doctor', (el) => { apptState.doctorId = el.value; refresh(); });
onAction('appt.filter', (el) => { apptState.status = el.value; refresh(); });
onAction('appt.today', () => { apptState.date = todayISO(); refresh(); });
onAction('appt.move', (el) => { apptState.date = addDays(apptState.date || todayISO(), Number(el.dataset.delta) || 0); refresh(); });

onAction('appt.status', async (el) => {
  await post(`/api/appointments/${el.dataset.id}/status`, { status: el.dataset.status });
  toast(t('adm.saved'), 'ok');
  refresh();
});

onAction('appt.checkin', async (el) => {
  const res = await post('/api/queue', { appointmentId: el.dataset.id });
  toast(bi('بیمار به صف اضافه شد — شماره ', 'Added to queue — number ') + faNum(res.number), 'ok');
  refresh();
});

onAction('appt.cancel', (el) => confirmDlg(t('common.cancel'), bi('این نوبت لغو شود؟', 'Cancel this appointment?'), async () => {
  await post(`/api/appointments/${el.dataset.id}/status`, { status: 'cancelled', note: 'cancelled from worklist' });
  refresh();
}, { danger: true }));

onAction('appt.noshow', (el) => confirmDlg(bi('ثبت عدم حضور', 'Mark no-show'), bi('عدم حضور بیمار ثبت شود؟', 'Record that the patient did not attend?'), async () => {
  await post(`/api/appointments/${el.dataset.id}/status`, { status: 'no_show' });
  refresh();
}));

onAction('appt.consult', async (el) => {
  await openConsult({ patientId: el.dataset.patient, doctorId: el.dataset.doctor, appointmentId: el.dataset.id });
});

/* ------------------------------------------------------- booking modal */

function appointmentModal(presetPatient = '') {
  openModal({
    title: bi('نوبت جدید', 'New appointment'),
    size: 'wide',
    body: `<form data-form="appt.create">
      <div class="form">
        ${patientPicker('patientId', bi('بیمار', 'Patient'), 'f-6')}
        ${selectField(bi('پزشک', 'Doctor'), 'doctorId', doctorOptions(false), 'f-6', S.user.doctorId || (S.meta?.doctors?.[0]?.id || ''), { required: true })}
        ${dateField('date', apptState.date || todayISO(), 'f-4', { label: bi('تاریخ', 'Date'), required: true, range: [isoToCal(todayISO()).y - 1, isoToCal(todayISO()).y + 1] })}
        ${selectField(bi('نوع نوبت', 'Visit type'), 'type', TYPE_KEYS.map(([k, fa, en]) => [k, bi(fa, en)]), 'f-4', 'consultation')}
        ${selectField(bi('منبع', 'Source'), 'source', SOURCE_KEYS.map(([k, fa, en]) => [k, bi(fa, en)]), 'f-4', 'walk_in')}
        <div class="field f-12"><label>${esc(bi('زمان‌های آزاد', 'Available slots'))}</label>
          <div id="slot-box" class="flex">${emptyState('clock', bi('تاریخ و پزشک را انتخاب کنید', 'Choose a doctor and date'))}</div>
          <input type="hidden" name="time" id="slot-value">
        </div>
        ${textField(bi('علت مراجعه', 'Reason for visit'), 'reasonFa', '', 'f-6')}
        ${textField(bi('English reason', 'Reason (English)'), 'reasonEn', '', 'f-6')}
        ${textArea(t('common.notes'), 'notes', '', 'f-12', { rows: 2 })}
        <div class="f-12 hidden" id="appt-error"></div>
      </div>
      <div class="flex-b mt14">
        <button class="btn" type="button" data-close>${esc(t('common.cancel'))}</button>
        <button class="btn pri" type="submit">${ic('calendar')}${esc(bi('ثبت نوبت', 'Book appointment'))}</button>
      </div>
    </form>`,
    onMount: (modal) => {
      const form = modal.querySelector('form');
      const reload = async () => {
        const box = modal.querySelector('#slot-box');
        const doctor = form.querySelector('[name="doctorId"]').value;
        const date = readDate(form, 'date');
        if (!box) return;
        if (!doctor || !date) {
          box.innerHTML = emptyState('clock', bi('تاریخ و پزشک را انتخاب کنید', 'Choose a doctor and date'));
          return;
        }
        box.innerHTML = `<span class="mini-note">${esc(bi('در حال بررسی…', 'Checking…'))}</span>`;
        try {
          const res = await get(`/api/appointments/slots?doctorId=${encodeURIComponent(doctor)}&date=${encodeURIComponent(date)}`);
          if (res.closed) {
            box.innerHTML = `<div class="alert amber" style="margin:0">${ic('alert')}${esc(bi('پزشک در این روز فعال نیست', 'Doctor is not scheduled that day'))}</div>`;
            return;
          }
          box.innerHTML = res.slots.filter((s) => s.available).length
            ? res.slots.map((s) => `<button type="button" class="btn sm" data-act="appt.pick" data-time="${esc(s.time)}" ${s.available ? '' : 'disabled'}>${esc(fmtTime(s.time))}</button>`).join('')
            : `<div class="alert amber" style="margin:0">${ic('alert')}${esc(bi('زمان آزادی در این روز وجود ندارد', 'No free slot on this day'))}</div>`;
        } catch (err) {
          box.innerHTML = `<div class="alert red" style="margin:0">${esc(err.payload?.message || err.message)}</div>`;
        }
      };
      modal.addEventListener('change', (event) => {
        if (event.target.name === 'doctorId' || event.target.name?.endsWith('__y') || event.target.name?.endsWith('__m') || event.target.name?.endsWith('__d')) {
          reload();
        }
      });
      if (presetPatient) {
        const hidden = modal.querySelector('#pp-patientId');
        const search = modal.querySelector('#pp-patientId-search');
        if (hidden) hidden.value = presetPatient;
        if (search) search.placeholder = bi('بیمار انتخاب شد', 'Patient selected');
      }
      reload();
    },
  });
}

onAction('appt.new', (el) => appointmentModal(el?.dataset?.patient || ''));
onAction('appt.pick', (el) => {
  const modal = el.closest('.modal');
  const hidden = modal.querySelector('#slot-value');
  if (hidden) hidden.value = el.dataset.time;
  modal.querySelectorAll('[data-act="appt.pick"]').forEach((b) => b.classList.remove('pri'));
  el.classList.add('pri');
});

onForm('appt.create', async (form) => {
  const v = formValues(form);
  const errorBox = form.querySelector('#appt-error');
  if (!v.patientId) throw new Error(bi('بیمار را انتخاب کنید', 'Select a patient'));
  if (!v.time) throw new Error(bi('یک زمان آزاد انتخاب کنید', 'Pick an available slot'));
  const payload = {
    patientId: v.patientId,
    doctorId: v.doctorId,
    date: readDate(form, 'date'),
    time: v.time,
    type: v.type,
    source: v.source,
    reasonFa: v.reasonFa || '',
    reasonEn: v.reasonEn || '',
    notes: v.notes || '',
  };
  try {
    await post('/api/appointments', payload);
    closeModal();
    toast(bi('نوبت ثبت شد', 'Appointment booked'), 'ok');
    apptState.date = payload.date;
    if (location.hash.startsWith('#/appointments')) refresh();
    else go('#/appointments');
  } catch (err) {
    if (errorBox) {
      errorBox.classList.remove('hidden');
      errorBox.innerHTML = `<div class="alert red">${ic('alert')}<div>${esc(err.payload?.message || err.message)}</div></div>`;
      return;
    }
    throw err;
  }
});

/* --------------------------------------------------------------- calendar */

export async function renderCalendar() {
  const now = isoToCal(todayISO());
  if (calState.y === null) { calState.y = now.y; calState.m = now.m; }
  const y = calState.y;
  const m = calState.m;
  const monthStart = calToISO({ y, m, d: 1 });
  const startOffset = weekday(monthStart);
  const gridStart = addDays(monthStart, -startOffset);
  const gridEnd = addDays(gridStart, 41);
  const data = await get(`/api/calendar${query({ from: gridStart, to: gridEnd })}`);
  const byDate = new Map((data.days || []).map((d) => [d.date, d]));

  const timeM = todayISO();
  const cells = [];
  for (let i = 0; i < 42; i += 1) {
    const iso = addDays(gridStart, i);
    const c = isoToCal(iso);
    const info = byDate.get(iso);
    const active = info ? Object.entries(info.byStatus || {}).filter(([k]) => !['cancelled', 'no_show'].includes(k)).reduce((s, [, n]) => s + n, 0) : 0;
    cells.push(`<div class="cal-cell ${c.m !== m ? 'other' : ''} ${iso === timeM ? 'today' : ''}" data-link="#/appointments?date=${esc(iso)}" title="${esc(fmtDate(iso))}">
      <div class="d">${faNum(c.d)}</div>
      ${active ? `<div class="c">${faNum(active)} ${esc(bi('نوبت', 'appt'))}</div>` : ''}
      ${info?.byStatus?.cancelled ? `<div class="mini-note">${faNum(info.byStatus.cancelled)} ${esc(bi('لغو', 'cancelled'))}</div>` : ''}
    </div>`);
  }
  const weekdays = S.lang === 'fa'
    ? ['یکشنبه', 'دوشنبه', 'سه‌شنبه', 'چهارشنبه', 'پنجشنبه', 'جمعه', 'شنبه']
    : ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'];

  return `<div class="page-head"><div class="grow"><h2>${esc(t('nav.calendar'))}</h2>
      <div class="sub">${esc(bi('نمای ماهانه نوبت‌ها', 'Monthly appointment view'))}</div></div>
      <div class="head-actions"><button class="btn pri" data-act="appt.new">${ic('plus')}${esc(bi('نوبت جدید', 'New appointment'))}</button></div></div>
    <div class="card">
      <div class="flex-b mb14">
        <div class="flex">
          <button class="btn sm" data-act="cal.prev">‹</button>
          <b style="font-size:15px">${esc(calMonths()[m - 1])} ${faNum(y)}</b>
          <button class="btn sm" data-act="cal.next">›</button>
        </div>
        <div class="flex">
          <button class="btn sm" data-act="cal.today">${esc(bi('ماه جاری', 'Current month'))}</button>
          <button class="btn sm" data-link="#/appointments">${esc(bi('نمای روز', 'Day view'))}</button>
        </div>
      </div>
      <div class="cal-head">${weekdays.map((d) => `<div>${esc(d)}</div>`).join('')}</div>
      <div class="cal-grid">${cells.join('')}</div>
      <div class="mini-note mt8">${esc(bi('برای دیدن نوبت‌های هر روز روی آن کلیک کنید.', 'Click a day to open its appointment list.'))}</div>
    </div>`;
}

onAction('cal.prev', () => {
  calState.m -= 1;
  if (calState.m < 1) { calState.m = 12; calState.y -= 1; }
  refresh();
});
onAction('cal.next', () => {
  calState.m += 1;
  if (calState.m > 12) { calState.m = 1; calState.y += 1; }
  refresh();
});
onAction('cal.today', () => {
  const now = isoToCal(todayISO());
  calState.y = now.y;
  calState.m = now.m;
  refresh();
});

/* ------------------------------------------------------------------ queue */

export async function renderQueue() {
  const date = queueState.date || todayISO();
  const data = await get(`/api/queue${query({ date, doctorId: queueState.doctorId })}`);
  const rows = data.items || [];
  const waiting = rows.filter((q) => q.status === 'waiting' || q.status === 'called');
  const active = rows.filter((q) => q.status === 'in_progress');

  const card = (q) => `<div class="card" style="box-shadow:none;padding:12px 14px">
    <div class="flex-b">
      <div class="flex">
        <span class="avatar" style="width:38px;height:38px;border-radius:11px;background:${q.priority ? 'var(--err)' : 'var(--pri)'}">${faNum(q.number)}</span>
        <div><a class="rowlink" href="#/patient/${esc(q.patientId)}">${esc(bi(q.patientName.fa, q.patientName.en))}</a>
          <div class="mini-note">${esc(q.patientCode)} · ${esc(fmtAge(q.dob))} · ${esc(bi(q.doctorName.fa, q.doctorName.en))}
          ${q.allergyCount ? ' · <span style="color:var(--err)">⚠ ' + faNum(q.allergyCount) + ' ' + esc(bi('حساسیت', 'allergies')) + '</span>' : ''}</div>
          <div class="mini-note">${esc(bi('ورود', 'in'))}: ${esc(fmtTime(new Date(q.checkedInAt).toTimeString().slice(0, 5)))}${q.appointmentTime ? ' · ' + esc(bi('نوبت', 'appt')) + ' ' + esc(fmtTime(q.appointmentTime)) : ''}</div></div>
      </div>
      <div class="flex">${queueBadge(q.status)}${q.priority ? `<span class="tag red">${esc(bi('اولویت', 'Priority'))}</span>` : ''}</div>
    </div>
    <div class="actions mt8">
      ${q.status === 'waiting' ? `<button class="btn xs" data-act="queue.status" data-id="${esc(q.id)}" data-status="called">${esc(bi('صدا زدن', 'Call'))}</button>` : ''}
      ${q.status === 'called' ? `<button class="btn xs" data-act="queue.status" data-id="${esc(q.id)}" data-status="waiting">${esc(bi('بازگشت', 'Back'))}</button>` : ''}
      ${['waiting', 'called'].includes(q.status) ? `<button class="btn xs pri" data-act="queue.status" data-id="${esc(q.id)}" data-status="in_progress">${esc(bi('ورود به اتاق', 'Start'))}</button>` : ''}
      ${q.status === 'in_progress' ? `<button class="btn xs ok" data-act="queue.status" data-id="${esc(q.id)}" data-status="done">${esc(bi('پایان', 'Done'))}</button>` : ''}
      ${can('vitals.write') ? `<button class="btn xs" data-act="queue.vitals" data-id="${esc(q.id)}" data-patient="${esc(q.patientId)}">${ic('heart', 'ic-s')}${esc(bi('علائم حیاتی', 'Vitals'))}</button>` : ''}
      ${can('nursing.notes') ? `<button class="btn xs" data-act="queue.note" data-patient="${esc(q.patientId)}">${ic('notes', 'ic-s')}${esc(bi('یادداشت', 'Note'))}</button>` : ''}
      ${can('clinical.write') ? `<button class="btn xs pri" data-act="queue.consult" data-id="${esc(q.id)}" data-patient="${esc(q.patientId)}" data-doctor="${esc(q.doctorId)}">${esc(bi('ویزیت', 'Visit'))}</button>` : ''}
      <button class="btn xs err" data-act="queue.status" data-id="${esc(q.id)}" data-status="cancelled">${esc(t('common.cancel'))}</button>
    </div>
  </div>`;

  return `<div class="page-head"><div class="grow"><h2>${esc(t('nav.queue'))}</h2>
      <div class="sub">${esc(fmtDate(date))} · ${esc(bi('صف انتظار و نوبت‌دهی', 'Live queue'))}</div></div>
      <div class="head-actions">
        ${can('queue.manage') ? `<button class="btn pri" data-act="queue.new">${ic('userplus')}${esc(bi('پذیرش بدون نوبت', 'Walk-in check-in'))}</button>` : ''}
      </div></div>
    <div class="card"><div class="inline-fields">
      <div class="field"><label>${esc(bi('تاریخ', 'Date'))}</label><input type="date" value="${esc(date)}" data-change="queue.date"></div>
      <div class="field"><label>${esc(bi('پزشک', 'Doctor'))}</label><select data-change="queue.doctor">${doctorOptions().map(([v, l]) => `<option value="${esc(v)}" ${queueState.doctorId === v ? 'selected' : ''}>${esc(l)}</option>`).join('')}</select></div>
      <button class="btn" data-act="queue.today">${esc(bi('امروز', 'Today'))}</button>
      <div class="right mini-note">${faNum(waiting.length)} ${esc(bi('در انتظار', 'waiting'))} · ${faNum(active.length)} ${esc(bi('در حال ویزیت', 'in progress'))}</div>
    </div></div>
    <div class="grid2 mt14">
      <div class="card"><h3>${ic('queue')}${esc(bi('در انتظار', 'Waiting'))}</h3>
        <div class="stack">${waiting.length ? waiting.map(card).join('') : emptyState('queue', bi('کسی در انتظار نیست', 'Nobody waiting'))}</div></div>
      <div class="card"><h3>${ic('notes')}${esc(bi('در حال ویزیت', 'In progress'))}</h3>
        <div class="stack">${active.length ? active.map(card).join('') : emptyState('notes', bi('ویزیت فعالی وجود ندارد', 'No active visits'))}</div></div>
    </div>`;
}

onAction('queue.date', (el) => { queueState.date = el.value; refresh(); });
onAction('queue.doctor', (el) => { queueState.doctorId = el.value; refresh(); });
onAction('queue.today', () => { queueState.date = todayISO(); refresh(); });

onAction('queue.status', async (el) => {
  await post(`/api/queue/${el.dataset.id}/status`, { status: el.dataset.status });
  refresh();
});

onAction('queue.consult', async (el) => {
  await openConsult({ patientId: el.dataset.patient, doctorId: el.dataset.doctor, queueEntryId: el.dataset.id });
});

onAction('queue.new', () => {
  openModal({
    title: bi('پذیرش بدون نوبت', 'Walk-in check-in'),
    size: '',
    body: `<form data-form="queue.create">
      <div class="form">
        ${patientPicker('patientId', bi('بیمار', 'Patient'), 'f-12')}
        ${selectField(bi('پزشک', 'Doctor'), 'doctorId', doctorOptions(false), 'f-8', S.user.doctorId || (S.meta?.doctors?.[0]?.id || ''))}
        <div class="field f-4"><label class="checkline" style="margin-top:26px"><input type="checkbox" name="priority"> ${esc(bi('اولویت', 'Priority'))}</label></div>
      </div>
      <div class="flex-b mt14">
        <button class="btn" type="button" data-close>${esc(t('common.cancel'))}</button>
        <button class="btn pri" type="submit">${esc(bi('افزودن به صف', 'Add to queue'))}</button>
      </div>
    </form>`,
  });
});

onForm('queue.create', async (form) => {
  const v = formValues(form);
  if (!v.patientId) throw new Error(bi('بیمار را انتخاب کنید', 'Select a patient'));
  const res = await post('/api/queue', { patientId: v.patientId, doctorId: v.doctorId, priority: Boolean(v.priority) });
  closeModal();
  toast(bi('شماره صف: ', 'Queue number: ') + faNum(res.number), 'ok');
  refresh();
});

/* ---------------------------------------------------------------- vitals */

function vitalsModal(queueId, patientId) {
  openModal({
    title: bi('ثبت علائم حیاتی', 'Record vitals'),
    size: '',
    body: `<form data-form="vitals.create" data-queue="${esc(queueId || '')}" data-patient="${esc(patientId)}">
      <div class="form">
        ${textField(bi('فشار خون', 'Blood pressure'), 'bp', '', 'f-4', { placeholder: '120/80', dir: 'ltr' })}
        ${textField(bi('ضربان قلب', 'Heart rate'), 'hr', '', 'f-4', { type: 'number', dir: 'ltr' })}
        ${textField(bi('تعداد تنفس', 'Respiratory rate'), 'rr', '', 'f-4', { type: 'number', dir: 'ltr' })}
        ${textField(bi('دما', 'Temperature'), 'temp', '', 'f-4', { type: 'number', dir: 'ltr' })}
        ${textField(bi('اشباع اکسیژن', 'SpO₂'), 'spo2', '', 'f-4', { type: 'number', dir: 'ltr' })}
        ${textField(bi('قند خون', 'Glucose'), 'glucose', '', 'f-4', { type: 'number', dir: 'ltr' })}
        ${textField(bi('قد (سانتی‌متر)', 'Height (cm)'), 'height', '', 'f-4', { type: 'number', dir: 'ltr' })}
        ${textField(bi('وزن (کیلوگرم)', 'Weight (kg)'), 'weight', '', 'f-4', { type: 'number', dir: 'ltr' })}
        ${textArea(t('common.notes'), 'note', '', 'f-12', { rows: 2 })}
      </div>
      <div class="flex-b mt14">
        <button class="btn" type="button" data-close>${esc(t('common.cancel'))}</button>
        <button class="btn pri" type="submit">${esc(t('common.save'))}</button>
      </div>
    </form>`,
  });
}

onAction('queue.vitals', (el) => vitalsModal(el.dataset.id, el.dataset.patient));
onAction('patient.vitals', (el) => vitalsModal('', el.dataset.patient));

onForm('vitals.create', async (form) => {
  const v = formValues(form);
  const payload = {
    bp: v.bp || '',
    hr: v.hr ? Number(v.hr) : null,
    rr: v.rr ? Number(v.rr) : null,
    temp: v.temp ? Number(v.temp) : null,
    spo2: v.spo2 ? Number(v.spo2) : null,
    glucose: v.glucose ? Number(v.glucose) : null,
    height: v.height ? Number(v.height) : null,
    weight: v.weight ? Number(v.weight) : null,
    note: v.note || '',
  };
  const queueId = form.dataset.queue;
  const path = queueId ? `/api/queue/${queueId}/vitals` : `/api/patients/${form.dataset.patient}/vitals`;
  await post(path, payload);
  closeModal();
  toast(t('adm.saved'), 'ok');
  refresh();
});

/* ------------------------------------------------------------------ notes */

onAction('queue.note', (el) => {
  openModal({
    title: bi('یادداشت پرستاری', 'Nursing note'),
    size: 'narrow',
    body: `<form data-form="note.create" data-patient="${esc(el.dataset.patient)}">
      <div class="form">
        ${selectField(bi('نوع', 'Type'), 'kind', [['observation', bi('مشاهده', 'Observation')], ['triage', bi('تریاژ', 'Triage')], ['handover', bi('تحویل شیفت', 'Handover')], ['education', bi('آموزش', 'Education')]], 'f-12', 'observation')}
        ${textArea(bi('متن یادداشت', 'Note'), 'textFa', '', 'f-12', { rows: 3, required: true })}
        ${textArea('English', 'textEn', '', 'f-12', { rows: 2 })}
      </div>
      <div class="flex-b mt14">
        <button class="btn" type="button" data-close>${esc(t('common.cancel'))}</button>
        <button class="btn pri" type="submit">${esc(t('common.save'))}</button>
      </div>
    </form>`,
  });
});

onForm('note.create', async (form) => {
  const v = formValues(form);
  await post(`/api/patients/${form.dataset.patient}/nurse-notes`, { kind: v.kind, textFa: v.textFa, textEn: v.textEn || '' });
  closeModal();
  toast(t('adm.saved'), 'ok');
});

/* ------------------------------------------------------------------ nurse */

export async function renderNurse() {
  const data = await get(`/api/queue${query({ date: todayISO() })}`);
  const rows = (data.items || []).filter((q) => ['waiting', 'called', 'in_progress'].includes(q.status));
  return `<div class="page-head"><div class="grow"><h2>${esc(bi('تریاژ و آماده‌سازی بیمار', 'Triage & patient preparation'))}</h2>
      <div class="sub">${esc(fmtDate(todayISO()))} · ${esc(bi('ثبت علائم حیاتی و یادداشت پرستاری', 'Record vitals and nursing notes'))}</div></div>
      <div class="head-actions"><button class="btn" data-link="#/queue">${ic('queue')}${esc(t('nav.queue'))}</button></div></div>
    <div class="card"><h3>${ic('clipboard')}${esc(bi('بیماران امروز', "Today's patients"))}</h3>
      ${rows.length ? `<div class="stack">${rows.map((q) => `<div class="flex-b" style="border-bottom:1px solid var(--line);padding-bottom:9px">
        <div class="flex"><span class="avatar" style="border-radius:9px;background:${q.priority ? 'var(--err)' : 'var(--pri)'}">${faNum(q.number)}</span>
          <div><a class="rowlink" href="#/patient/${esc(q.patientId)}">${esc(bi(q.patientName.fa, q.patientName.en))}</a>
            <div class="mini-note">${esc(q.patientCode)} · ${esc(fmtAge(q.dob))} · ${esc(bi(q.doctorName.fa, q.doctorName.en))}
            ${q.allergyCount ? ' · <span style="color:var(--err)">⚠ ' + faNum(q.allergyCount) + '</span>' : ''}</div></div></div>
        <div class="flex">${queueBadge(q.status)}
          <button class="btn sm" data-act="queue.vitals" data-id="${esc(q.id)}" data-patient="${esc(q.patientId)}">${ic('heart', 'ic-s')}${esc(bi('علائم حیاتی', 'Vitals'))}</button>
          <button class="btn sm" data-act="queue.note" data-patient="${esc(q.patientId)}">${ic('notes', 'ic-s')}${esc(bi('یادداشت', 'Note'))}</button>
          <a class="btn sm" href="#/patient/${esc(q.patientId)}">${esc(bi('پرونده', 'Record'))}</a>
        </div>
      </div>`).join('')}</div>` : emptyState('clipboard', bi('بیماری در صف نیست', 'No patients in the queue'))}
    </div>
    <div class="alert blue mt14">${ic('shield')}<div>${esc(bi('پرستار به یادداشت‌های بالینی و علائم حیاتی دسترسی دارد و نمی‌تواند نسخه یا تشخیص صادر کند.', 'Nurses can record vitals and notes but cannot issue prescriptions or diagnoses.'))}</div></div>`;
}
