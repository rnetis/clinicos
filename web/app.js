// Application shell: boot, authentication, role navigation, dashboard, search.
import {
  S, t, bi, esc, can, faNum, num, money, fmtDate, fmtTime, fmtDateTime, fmtAge, ago,
  apptBadge, badge, patientName, roleLabel, get, post, api, go, toast, errorToast,
  openModal, closeModal, emptyState, spinner, onAction, onForm, installHandlers,
  field, invBadge, labBadge, patientName as pname, query, refresh,
} from './lib.js';
import { ic } from './icons.js';
import * as patientsView from './patients.js';
import * as schedulingView from './scheduling.js';
import * as clinicalView from './clinical.js';
import * as financeView from './finance.js';
import * as adminView from './admin.js';

const ROLE_HOME = {
  admin: '#/admin',
  receptionist: '#/reception',
  doctor: '#/doctor',
  nurse: '#/nurse',
  lab: '#/lab',
  accountant: '#/finance',
};

const NAV = {
  admin: [
    ['#/admin', 'nav.dashboard', 'dashboard'],
    ['#/patients', 'nav.patients', 'users'],
    ['#/appointments', 'nav.appointments', 'calendar'],
    ['#/queue', 'nav.queue', 'queue'],
    ['#/finance', 'nav.finance', 'money'],
    ['#/reports', 'nav.reports', 'chart'],
    ['#/admin/staff', 'adm.users', 'userplus'],
    ['#/admin/doctors', 'nav.doctors', 'stetho'],
    ['#/admin/services', 'adm.services', 'folder'],
    ['#/admin/settings', 'nav.settings', 'gear'],
    ['#/admin/audit', 'nav.audit', 'history'],
  ],
  receptionist: [
    ['#/reception', 'nav.dashboard', 'dashboard'],
    ['#/patients', 'nav.patient_search', 'search'],
    ['#/patients/new', 'nav.register', 'userplus'],
    ['#/appointments', 'nav.appointments', 'calendar'],
    ['#/calendar', 'nav.calendar', 'calendar'],
    ['#/queue', 'nav.queue', 'queue'],
    ['#/finance/invoices', 'nav.invoices', 'receipt'],
    ['#/reports', 'nav.reports', 'chart'],
  ],
  doctor: [
    ['#/doctor', 'nav.dashboard', 'dashboard'],
    ['#/doctor/waiting', 'nav.waiting', 'queue'],
    ['#/doctor/consults', 'nav.consultations', 'notes'],
    ['#/patients', 'nav.patients', 'users'],
    ['#/doctor/labs', 'nav.lab_results', 'flask'],
    ['#/doctor/followups', 'nav.followups', 'clock'],
  ],
  nurse: [
    ['#/nurse', 'nav.triage', 'clipboard'],
    ['#/queue', 'nav.queue', 'queue'],
    ['#/patients', 'nav.patients', 'users'],
  ],
  lab: [
    ['#/lab', 'nav.lab_orders', 'flask'],
    ['#/patients', 'nav.patients', 'users'],
  ],
  accountant: [
    ['#/finance', 'nav.dashboard', 'dashboard'],
    ['#/finance/invoices', 'nav.invoices', 'receipt'],
    ['#/reports', 'nav.reports', 'chart'],
    ['#/patients', 'nav.patients', 'users'],
  ],
};

const ACCESS = {
  patients: ['admin', 'receptionist', 'doctor', 'nurse', 'lab', 'accountant'],
  patient: ['admin', 'receptionist', 'doctor', 'nurse', 'lab', 'accountant'],
  appointments: ['admin', 'receptionist'],
  calendar: ['admin', 'receptionist'],
  queue: ['admin', 'receptionist', 'nurse'],
  consult: ['admin', 'doctor'],
  doctor: ['doctor'],
  nurse: ['nurse', 'admin'],
  lab: ['lab', 'admin'],
  finance: ['admin', 'accountant', 'receptionist'],
  invoice: ['admin', 'accountant', 'receptionist'],
  reports: ['admin', 'receptionist', 'doctor', 'accountant'],
  admin: ['admin'],
  reception: ['admin', 'receptionist'],
};

let publicCfg = { demo: false, demoUsers: [], clinicFa: '', clinicEn: '' };

function applyDir() {
  document.documentElement.lang = S.lang === 'en' ? 'en' : 'fa';
  document.documentElement.dir = S.lang === 'en' ? 'ltr' : 'rtl';
}

function homeFor(role) {
  return ROLE_HOME[role] || '#/patients';
}

/* ------------------------------------------------------------------- login */

function renderLogin(message = '') {
  applyDir();
  const demoBlock = publicCfg.demo && publicCfg.demoUsers?.length
    ? `<hr class="divider">
       <div class="small fs-bold muted mb8">${esc(bi('حساب‌های نمایشی', 'Demo accounts'))}</div>
       <div class="login-demo">${publicCfg.demoUsers.map((u) => `<button type="button" data-act="login.demo" data-user="${esc(u.username)}">${esc(roleLabel(u.role))} · ${esc(bi(u.nameFa, u.nameEn))}</button>`).join('')}</div>
       ${publicCfg.demoPassword ? `<div class="mini-note mt8">${esc(bi('رمز مشترک حساب‌های نمایشی:', 'Shared demo password:'))} <code class="kbd">${esc(publicCfg.demoPassword)}</code></div>` : ''}`
    : '';

  document.getElementById('app').innerHTML = `<div class="login-wrap"><div class="login-card">
    <div class="login-side">
      <div class="flex">
        <div style="width:44px;height:44px;border-radius:12px;background:rgba(255,255,255,.15);display:grid;place-items:center;color:#fff">${ic('heart', 'ic-l')}</div>
        <div><div style="font-size:20px;font-weight:800">ClinicOS</div>
          <small style="color:#a5d8e9">${esc(bi(publicCfg.clinicFa, publicCfg.clinicEn))}</small></div>
      </div>
      <p>${esc(bi('سامانه یکپارچه مدیریت کلینیک، پرونده الکترونیک، پذیرش، صف، آزمایشگاه و مالی.', 'Integrated clinic management: EMR, reception, queue, laboratory and billing.'))}</p>
      <ul>
        <li>${esc(bi('پرونده الکترونیک با نسخه‌بندی و امضای پزشک', 'Versioned EMR with physician sign-off'))}</li>
        <li>${esc(bi('نوبت‌دهی با جلوگیری از تداخل', 'Scheduling with conflict prevention'))}</li>
        <li>${esc(bi('نتایج آزمایش با هشدار مقادیر بحرانی', 'Lab results with critical-value alerts'))}</li>
        <li>${esc(bi('نقش‌ها، مجوزها و ثبت کامل رخدادها', 'Roles, permissions and a full audit trail'))}</li>
      </ul>
      <div style="margin-top:auto;font-size:11px;color:#8fcbe0">${esc(bi('نسخه ۱٫۰ · داده‌ها روی سرور ذخیره می‌شوند', 'v1.0 · data stored server-side'))}</div>
    </div>
    <div class="login-form">
      <div class="flex right">
        <button class="icon-btn" type="button" data-act="ui.lang">${ic('globe')}<b class="fs11">${S.lang === 'fa' ? 'English' : 'فارسی'}</b></button>
      </div>
      <h2>${esc(bi('ورود به سامانه', 'Sign in'))}</h2>
      <p class="muted fs12 mb14">${esc(publicCfg.clinicEn || '')}${publicCfg.clinicEn && publicCfg.clinicFa ? ' · ' : ''}${esc(publicCfg.clinicFa || '')}</p>
      <form data-form="login">
        <div class="form">
          <div class="field f-12"><label>${esc(bi('نام کاربری', 'Username'))}</label>
            <input name="username" id="login-username" autocomplete="username" required autofocus></div>
          <div class="field f-12"><label>${esc(bi('رمز عبور', 'Password'))}</label>
            <input name="password" id="login-password" type="password" autocomplete="current-password" required></div>
        </div>
        ${message ? `<div class="login-err show">${esc(message)}</div>` : ''}
        <button class="btn pri mt14" type="submit" style="width:100%;justify-content:center">${ic('logout')} ${esc(bi('ورود', 'Sign in'))}</button>
      </form>
      ${demoBlock}
    </div>
  </div></div>`;
  const input = document.getElementById('login-username');
  if (input) input.focus();
}

onForm('login', async (form) => {
  const data = new FormData(form);
  const username = String(data.get('username') || '').trim();
  const password = String(data.get('password') || '');
  if (!username || !password) return;
  try {
    S.user = await post('/api/auth/login', { username, password });
    await loadMeta();
    go(homeFor(S.user.role));
    await render();
  } catch (err) {
    if (err.status === 401) {
      renderLogin(bi('نام کاربری یا رمز عبور نادرست است.', 'Invalid username or password.'));
    } else if (err.status === 429) {
      renderLogin(bi('تلاش‌های ناموفق زیاد؛ چند دقیقه دیگر تلاش کنید.', 'Too many attempts; try again in a few minutes.'));
    } else {
      renderLogin(err.payload?.message || err.message);
    }
  }
});

onAction('login.demo', async (el) => {
  const username = el.dataset.user;
  const u = document.getElementById('login-username');
  const p = document.getElementById('login-password');
  if (u) u.value = username;
  if (p && publicCfg.demoPassword) p.value = publicCfg.demoPassword;
  const form = document.querySelector('form[data-form="login"]');
  if (form && publicCfg.demoPassword) form.requestSubmit();
});

/* ------------------------------------------------------------------ shell */

async function loadMeta() {
  S.meta = await get('/api/meta');
  const stored = localStorage.getItem('clinicos_cal');
  if (!stored && S.meta.settings?.calendar) S.cal = S.meta.settings.calendar;
}

function shell(content) {
  const user = S.user;
  const items = (NAV[user.role] || []).map(([href, key, icon]) => {
    const active = S.route === href || (href !== '#/patients' && S.route.startsWith(href + '/'));
    return `<a href="${href}" class="${active ? 'on' : ''}">${ic(icon)}<span>${esc(t(key))}</span></a>`;
  }).join('');
  const clinicName = bi(S.meta?.clinic?.nameFa, S.meta?.clinic?.nameEn);
  const calLabel = S.cal === 'jalali' ? bi('شمسی', 'Jalali') : bi('میلادی', 'Gregorian');
  return `<div class="shell">
    <div class="sidebar" id="sidebar">
      <div class="side-brand"><div class="logo">${ic('heart')}</div>
        <div><b>ClinicOS</b><small>${esc(clinicName)}</small></div></div>
      <div class="side-ws">${esc(roleLabel(user.role))}</div>
      <nav class="nav">${items}</nav>
      <div class="side-foot">v1.0 · ${esc(S.meta?.clinic?.phone || '')}</div>
    </div>
    <div class="main">
      <div class="topbar">
        <button class="burger" type="button" data-act="ui.menu" aria-label="menu">${ic('queue')}</button>
        <div class="brand-mini"><div class="logo">${ic('heart')}</div><span>ClinicOS</span></div>
        <div class="gsearch">
          ${ic('search')}
          <input id="gs-input" autocomplete="off" placeholder="${esc(bi('جستجوی بیمار، کد یا تلفن…', 'Search patient, code or phone…'))}">
          <div id="gs-drop" class="gs-drop hidden"></div>
        </div>
        <div class="top-actions">
          <button class="icon-btn" type="button" data-act="ui.cal" title="${esc(bi('تقویم', 'Calendar'))}">${ic('calendar')}<b class="fs11">${esc(calLabel)}</b></button>
          <button class="icon-btn" type="button" data-act="ui.lang" title="${esc(bi('زبان', 'Language'))}">${ic('globe')}<b class="fs11">${S.lang === 'fa' ? 'EN' : 'فا'}</b></button>
          <button class="icon-btn" type="button" data-act="notif.open" aria-label="${esc(bi('اعلان‌ها', 'Notifications'))}">${ic('bell')}<span class="dot hidden" id="notif-count"></span></button>
          <div class="chip-user" title="${esc(bi(user.nameFa, user.nameEn))}">
            <div class="avatar">${esc((bi(user.nameFa, user.nameEn) || '?').trim()[0] || '?')}</div>
            <div><b>${esc(bi(user.nameFa, user.nameEn))}</b><small>${esc(roleLabel(user.role))}</small></div>
          </div>
          <button class="icon-btn" type="button" data-act="ui.logout" title="${esc(t('common.logout'))}">${ic('logout')}</button>
        </div>
      </div>
      <div class="content">${content}</div>
    </div>
  </div>`;
}

onAction('ui.menu', () => {
  const sidebar = document.getElementById('sidebar');
  if (!sidebar) return;
  sidebar.classList.toggle('open');
  let scrim = document.querySelector('.scrim');
  if (sidebar.classList.contains('open')) {
    if (!scrim) {
      scrim = document.createElement('div');
      scrim.className = 'scrim';
      scrim.dataset.act = 'ui.menu';
      document.body.appendChild(scrim);
    }
  } else if (scrim) {
    scrim.remove();
  }
});

onAction('ui.lang', async () => {
  S.lang = S.lang === 'fa' ? 'en' : 'fa';
  localStorage.setItem('clinicos_lang', S.lang);
  applyDir();
  if (S.user) await render();
  else renderLogin();
});

onAction('ui.cal', async () => {
  S.cal = S.cal === 'jalali' ? 'gregorian' : 'jalali';
  localStorage.setItem('clinicos_cal', S.cal);
  await render();
});

onAction('ui.logout', async () => {
  try { await post('/api/auth/logout', {}); } catch { /* session already gone */ }
  S.user = null;
  S.meta = null;
  location.hash = '#/';
  renderLogin();
});

onAction('notif.open', async () => {
  const data = await get('/api/notifications');
  const items = data.items || [];
  openModal({
    title: bi('اعلان‌ها', 'Notifications'),
    size: '',
    body: items.length ? `<div class="stack">${items.map((n) => `<div class="notif-item ${n.read ? '' : 'unread'} ${n.crit ? 'crit' : ''}" data-act="notif.read" data-id="${esc(n.id)}" data-link="${esc(n.link || '')}">
      <div class="ndot"></div>
      <div><div class="nt">${esc(bi(n.text.fa, n.text.en))}</div><time>${esc(ago(n.at))}</time></div>
    </div>`).join('')}</div>` : emptyState('bell', bi('اعلانی وجود ندارد', 'No notifications')),
    footer: `<button class="btn" data-close type="button">${esc(t('common.close'))}</button>
             <button class="btn pri" type="button" data-act="notif.read-all">${esc(bi('خواندن همه', 'Mark all read'))}</button>`,
  });
});

onAction('notif.read-all', async () => {
  await post('/api/notifications/read-all', {});
  closeModal();
  await updateNotifCount();
  toast(bi('همه اعلان‌ها خوانده شد', 'All notifications marked read'), 'ok');
});

onAction('notif.read', async (el) => {
  const id = el.dataset.id;
  const link = el.dataset.link;
  try { await post(`/api/notifications/${id}/read`, {}); } catch { /* already read */ }
  closeModal();
  await updateNotifCount();
  if (link) go(link);
});

async function updateNotifCount() {
  try {
    const data = await get('/api/notifications');
    const dot = document.getElementById('notif-count');
    if (!dot) return;
    if (data.unread > 0) {
      dot.textContent = faNum(data.unread);
      dot.classList.remove('hidden');
    } else {
      dot.classList.add('hidden');
    }
  } catch { /* signed out */ }
}

/* ---------------------------------------------------------------- search */

let searchTimer = null;
function onSearchInput(event) {
  if (event.target.id !== 'gs-input') return;
  const value = event.target.value.trim();
  const drop = document.getElementById('gs-drop');
  if (!drop) return;
  clearTimeout(searchTimer);
  if (value.length < 2) {
    drop.classList.add('hidden');
    return;
  }
  searchTimer = setTimeout(async () => {
    try {
      const data = await get(`/api/patients?q=${encodeURIComponent(value)}&perPage=7`);
      if (!data.items.length) {
        drop.innerHTML = emptyState('search', bi('بیماری یافت نشد', 'No patient found'));
      } else {
        drop.innerHTML = `<div class="gs-h">${esc(bi('بیماران', 'Patients'))}</div>` + data.items.map((p) => `
          <div class="gs-item" data-link="#/patient/${esc(p.id)}">
            ${ic('user', 'ic-s')}
            <div><b>${esc(patientName(p))}</b><small> · ${esc(p.code)} · ${esc(p.phone || '')} · ${esc(fmtAge(p.dob))}</small></div>
          </div>`).join('');
      }
      drop.classList.remove('hidden');
    } catch (err) {
      drop.classList.add('hidden');
    }
  }, 220);
}

document.addEventListener('click', (event) => {
  const drop = document.getElementById('gs-drop');
  if (drop && !event.target.closest('.gsearch')) drop.classList.add('hidden');
});

/* -------------------------------------------------------------- dashboard */

function kpi(label, value, icon = 'chart', hint = '') {
  return `<div class="stat"><div class="lbl">${ic(icon)}${esc(label)}</div>
    <div class="val">${esc(value)}</div>${hint ? `<div class="hint">${esc(hint)}</div>` : ''}</div>`;
}

async function dashboard() {
  const d = await get('/api/dashboard');
  const k = d.kpis;
  const user = S.user;
  const critical = d.criticalResults || [];
  const overdues = d.overdueFollowUps || [];

  let html = `<div class="page-head"><div class="grow">
      <h2>${esc(bi('داشبورد', 'Dashboard'))}</h2>
      <div class="sub">${esc(fmtDate(d.date))} · ${esc(bi('وضعیت زنده کلینیک', 'Live clinic status'))}</div>
    </div><div class="head-actions">
      ${user.role === 'receptionist' || user.role === 'admin' ? `<button class="btn pri" data-link="#/appointments">${ic('plus')}${esc(bi('نوبت جدید', 'New appointment'))}</button>` : ''}
      ${user.role === 'admin' ? `<a class="btn" href="/api/admin/backup">${ic('db')}${esc(bi('پشتیبان', 'Backup'))}</a>` : ''}
    </div></div>`;

  if (critical.length) {
    html += `<div class="banner-crit">${ic('alert')}<span>${esc(bi('نتایج بحرانی در انتظار بررسی', 'Critical results awaiting review'))}</span>
      <a class="btn sm right" href="#/doctor/labs">${esc(bi('مشاهده', 'Review'))}</a></div>`;
  }

  html += `<div class="kpi-grid">
    ${kpi(bi('نوبت‌های امروز', "Today's appointments"), faNum(k.appointments), 'calendar')}
    ${kpi(bi('ویزیت‌های تکمیل‌شده', 'Completed visits'), faNum(k.completed), 'check')}
    ${kpi(bi('در صف انتظار', 'Waiting now'), faNum(k.waiting), 'queue', k.inProgress ? `${faNum(k.inProgress)} ${bi('در حال ویزیت', 'in progress')}` : '')}
    ${kpi(bi('پیش‌نویس‌های پرونده', 'Open drafts'), faNum(k.drafts), 'notes')}
    ${kpi(bi('آزمایش‌های در جریان', 'Labs pending'), faNum(k.labsPending), 'flask')}
    ${kpi(bi('پیگیری‌های سررسید', 'Overdue follow-ups'), faNum(k.overdueFollowUps), 'clock')}
    ${kpi(bi('بیماران جدید امروز', 'New patients today'), faNum(k.newPatients), 'userplus')}
    ${k.revenueToday !== null && k.revenueToday !== undefined ? kpi(bi('دریافتی امروز', 'Collected today'), money(k.revenueToday), 'money') : ''}
    ${k.outstanding !== null && k.outstanding !== undefined ? kpi(bi('مانده مطالبات', 'Outstanding'), money(k.outstanding), 'receipt') : ''}
  </div>`;

  html += `<div class="split"><div class="stack">`;
  html += `<div class="card"><h3>${ic('calendar')}${esc(bi('نوبت‌های امروز', "Today's appointments"))}</h3>`;
  if (!d.appointments.length) {
    html += emptyState('calendar', bi('امروز نوبتی ثبت نشده است', 'No appointments today'));
  } else {
    html += `<div class="tbl-wrap"><table class="tbl"><thead><tr>
      <th>${esc(bi('زمان', 'Time'))}</th><th>${esc(bi('بیمار', 'Patient'))}</th>
      <th>${esc(bi('پزشک', 'Doctor'))}</th><th>${esc(bi('وضعیت', 'Status'))}</th><th>${esc(bi('علت', 'Reason'))}</th>
    </tr></thead><tbody>${d.appointments.map((a) => `<tr>
      <td class="num">${esc(fmtTime(a.time))}</td>
      <td><a class="rowlink" href="#/patient/${esc(a.patientId)}">${esc(bi(a.patientName.fa, a.patientName.en))}</a>
          <div class="mini-note">${esc(a.patientCode || '')}${a.queueNumber ? ' · ' + esc(bi('صف', 'queue')) + ' ' + faNum(a.queueNumber) : ''}</div></td>
      <td>${esc(bi(a.doctorName.fa, a.doctorName.en))}</td>
      <td>${apptBadge(a.status)}</td>
      <td class="small">${esc(bi(a.reason.fa, a.reason.en))}</td>
    </tr>`).join('')}</tbody></table></div>`;
  }
  html += `</div>`;

  if (d.revenueTrend) {
    const max = Math.max(1, ...d.revenueTrend.map((r) => r.amount));
    html += `<div class="card"><h3>${ic('money')}${esc(bi('دریافتی ۷ روز گذشته', 'Collected · last 7 days'))}</h3>
      <div class="bars">${d.revenueTrend.map((r) => `<div class="bar-row">
        <div class="bl">${esc(fmtDate(r.date, { short: true }))}</div>
        <div class="bar-track"><div class="bar-fill" style="width:${Math.max(2, Math.round(r.amount / max * 100))}%"></div></div>
        <div class="bv">${esc(num(r.amount))}</div></div>`).join('')}</div></div>`;
  }
  html += `</div><div class="stack">`;

  html += `<div class="card"><h3>${ic('queue')}${esc(bi('صف انتظار', 'Waiting queue'))}</h3>`;
  html += d.queue.length ? `<div class="stack">${d.queue.map((q) => `<div class="flex-b" style="border-bottom:1px solid var(--line);padding-bottom:7px">
      <div class="flex"><span class="avatar" style="background:${q.priority ? 'var(--err)' : 'var(--pri)'}">${faNum(q.number)}</span>
        <div><b>${esc(bi(q.patientName.fa, q.patientName.en))}</b><div class="mini-note">${esc(bi(q.doctorName.fa, q.doctorName.en))}</div></div></div>
      <div>${q.status === 'waiting' ? badge(t('status.waiting'), 'b-amber') : q.status === 'in_progress' ? badge(t('status.in_progress'), 'b-violet') : badge(t('q.called_col'), 'b-cyan')}</div>
    </div>`).join('')}</div>` : emptyState('queue', bi('صف خالی است', 'Queue is empty'));
  html += `</div>`;

  if (overdues.length) {
    html += `<div class="card"><h3>${ic('clock')}${esc(bi('پیگیری‌های سررسید گذشته', 'Overdue follow-ups'))}</h3>
      <div class="stack">${overdues.map((f) => `<div class="flex-b" style="border-bottom:1px solid var(--line);padding-bottom:6px">
        <div><a class="rowlink" href="#/patient/${esc(f.patientId)}">${esc(bi(f.patientName.fa, f.patientName.en))}</a>
          <div class="mini-note">${esc(bi(f.reason.fa, f.reason.en))}</div></div>
        <span class="tag ${f.priority === 'urgent' ? 'red' : f.priority === 'high' ? 'amber' : 'gray'}">${esc(fmtDate(f.due))}</span>
      </div>`).join('')}</div></div>`;
  }

  if (user.role === 'admin') {
    html += `<div class="card"><h3>${ic('shield')}${esc(bi('مدیریت', 'Administration'))}</h3>
      <div class="btn-group">
        <button class="btn sm" data-link="#/admin/staff">${ic('userplus')}${esc(t('adm.users'))}</button>
        <button class="btn sm" data-link="#/admin/doctors">${ic('stetho')}${esc(t('nav.doctors'))}</button>
        <button class="btn sm" data-link="#/admin/services">${ic('folder')}${esc(t('adm.services'))}</button>
        <button class="btn sm" data-link="#/admin/settings">${ic('gear')}${esc(t('nav.settings'))}</button>
        <button class="btn sm" data-link="#/admin/audit">${ic('history')}${esc(t('nav.audit'))}</button>
      </div></div>`;
  }
  html += `</div></div>`;
  return html;
}

/* ---------------------------------------------------------------- routing */

function allowed(segs, role) {
  if (!segs.length) return true;
  const head = segs[0];
  const roles = ACCESS[head];
  if (!roles) return false;
  if (head === 'patients' && segs[1] === 'new' && !ACCESS.patients.includes(role)) return false;
  return roles.includes(role);
}

function denied() {
  return `<div class="card">${emptyState('shield', bi('شما به این بخش دسترسی ندارید', 'You do not have access to this section'))}
    <div style="text-align:center"><button class="btn" data-link="${homeFor(S.user.role)}">${esc(bi('بازگشت به خانه', 'Back home'))}</button></div></div>`;
}

async function view(segs) {
  const head = segs[0] || '';
  if (!head) return dashboard();
  switch (head) {
    case 'reception':
      return dashboard();
    case 'admin':
      if (segs[1] === 'staff') return adminView.renderStaff();
      if (segs[1] === 'doctors') return adminView.renderDoctors();
      if (segs[1] === 'services') return adminView.renderServices();
      if (segs[1] === 'settings') return adminView.renderSettings();
      if (segs[1] === 'audit') return adminView.renderAudit();
      return dashboard();
    case 'doctor':
      if (segs[1] === 'waiting') return clinicalView.renderWaiting();
      if (segs[1] === 'consults') return clinicalView.renderConsults();
      if (segs[1] === 'labs') return clinicalView.renderDoctorLabs();
      if (segs[1] === 'followups') return clinicalView.renderFollowUps();
      return clinicalView.renderDoctorHome();
    case 'nurse':
      return schedulingView.renderNurse();
    case 'lab':
      return clinicalView.renderLabWorkspace();
    case 'patients':
      if (segs[1] === 'new') return patientsView.renderRegister();
      return patientsView.renderList();
    case 'patient':
      return patientsView.renderProfile(segs[1]);
    case 'appointments':
      return schedulingView.renderAppointments();
    case 'calendar':
      return schedulingView.renderCalendar();
    case 'queue':
      return schedulingView.renderQueue();
    case 'consult':
      return clinicalView.renderConsult(segs[1]);
    case 'finance':
      if (segs[1] === 'invoices') return financeView.renderInvoices();
      return financeView.renderFinanceHome();
    case 'invoice':
      return financeView.renderInvoice(segs[1]);
    case 'reports':
      return financeView.renderReports();
    default:
      return `<div class="card">${emptyState('search', bi('صفحه یافت نشد', 'Page not found'))}</div>`;
  }
}

async function render() {
  applyDir();
  if (!S.user) {
    renderLogin();
    return;
  }
  if (!S.meta) {
    try { await loadMeta(); } catch (err) { /* retry next navigation */ }
  }
  const hash = location.hash || '#/';
  S.route = hash;
  const [path, qs] = hash.split('?');
  const segs = path.replace(/^#\/?/, '').split('/').filter(Boolean);
  const app = document.getElementById('app');
  if (!allowed(segs, S.user.role)) {
    app.innerHTML = shell(denied());
    return;
  }
  app.innerHTML = shell(spinner());
  let content;
  try {
    content = await view(segs, new URLSearchParams(qs || ''));
  } catch (err) {
    const message = err?.payload?.message || err?.message || String(err);
    content = `<div class="card"><div class="alert red">${ic('alert')}<div>${esc(message)}</div></div>
      <button class="btn" data-link="${homeFor(S.user.role)}">${esc(bi('بازگشت', 'Back'))}</button></div>`;
    if (!err?.status) console.error(err);
  }
  app.innerHTML = shell(content);
  updateNotifCount();
  window.scrollTo({ top: 0, behavior: 'instant' });
}

/* ------------------------------------------------------------------- boot */

async function boot() {
  installHandlers();
  window.addEventListener('hashchange', () => render());
  window.addEventListener('clinicos:refresh', () => render());
  window.addEventListener('clinicos:unauthorized', () => {
    S.user = null;
    S.meta = null;
    renderLogin(bi('نشست شما پایان یافته است؛ دوباره وارد شوید.', 'Your session expired; please sign in again.'));
  });
  document.addEventListener('input', onSearchInput);

  try {
    publicCfg = await get('/api/auth/config');
  } catch {
    publicCfg = { demo: false, demoUsers: [] };
  }
  try {
    S.user = await get('/api/auth/me');
    await loadMeta();
  } catch {
    S.user = null;
  }
  applyDir();
  if (!S.user) {
    renderLogin();
    return;
  }
  if (!location.hash || location.hash === '#/' || location.hash === '#') {
    location.hash = homeFor(S.user.role);
  } else {
    await render();
  }
}

boot();
