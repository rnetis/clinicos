// ClinicOS browser runtime: state, API client, bilingual formatting, Jalali calendar,
// modal/toast primitives and the delegated event glue used by every view.
import { I18N } from './i18n.js';
import { ic } from './icons.js';

export const S = {
  lang: localStorage.getItem('clinicos_lang') || 'fa',
  cal: localStorage.getItem('clinicos_cal') || 'jalali',
  user: null,
  meta: null,
  route: '',
  boot: null,
};

const FA_DIGITS = '۰۱۲۳۴۵۶۷۸۹';

export function t(key) {
  const entry = I18N[key];
  if (!entry) return key;
  const idx = S.lang === 'en' ? 1 : 0;
  return Array.isArray(entry) ? entry[idx] : String(entry).split('|')[idx] ?? key;
}

/** Bilingual literal: bi('فارسی','English') */
export function bi(fa, en) {
  const a = fa ?? '';
  const b = en ?? '';
  return S.lang === 'en' ? (b || a) : (a || b);
}

export function esc(value) {
  return String(value ?? '').replace(/[&<>"']/g, (c) => ({
    '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;',
  }[c]));
}

export const can = (perm) => {
  const list = S.user?.permissions || [];
  return list.includes('*') || list.includes(perm);
};

export const roleLabel = (role) => t('adm.role.' + role);

/* ---------------------------------------------------------------- numbers */

export function faNum(value) {
  if (value === null || value === undefined || value === '') return '';
  const s = String(value);
  return S.lang === 'fa' ? s.replace(/\d/g, (d) => FA_DIGITS[+d]) : s;
}

export function num(value) {
  const n = Number(value) || 0;
  return faNum(String(Math.round(n)).replace(/\B(?=(\d{3})+(?!\d))/g, S.lang === 'fa' ? '٬' : ','));
}

export function money(value) {
  const n = Math.round(Number(value) || 0);
  const abs = String(Math.abs(n)).replace(/\B(?=(\d{3})+(?!\d))/g, S.lang === 'fa' ? '٬' : ',');
  return (n < 0 ? '-' : '') + faNum(abs) + ' ' + t('common.currency');
}

export function pct(value) {
  return faNum(Math.round(Number(value) || 0)) + '%';
}

/* ------------------------------------------------------------------ dates */

const div = (a, b) => ~~(a / b);
const mod = (a, b) => a - ~~(a / b) * b;
const JBREAKS = [-61, 9, 38, 199, 426, 686, 756, 818, 1111, 1181, 1210, 1635, 2060, 2097, 2192, 2262, 2324, 2394, 2456, 3178];

function jalCal(jy) {
  const bl = JBREAKS.length;
  let gy = jy + 621;
  let leapJ = -14;
  let jp = JBREAKS[0];
  let jm = 0;
  let jump = 0;
  if (jy < jp || jy >= JBREAKS[bl - 1]) throw new Error('invalid jalali year');
  for (let i = 1; i < bl; i += 1) {
    jm = JBREAKS[i];
    jump = jm - jp;
    if (jy < jm) break;
    leapJ += div(jump, 33) * 8 + div(mod(jump, 33), 4);
    jp = jm;
  }
  let n = jy - jp;
  leapJ += div(n, 33) * 8 + div(mod(n, 33) + 3, 4);
  if (mod(jump, 33) === 4 && jump - n === 4) leapJ += 1;
  const leapG = div(gy, 4) - div((div(gy, 100) + 1) * 3, 4) - 150;
  const march = 20 + leapJ - leapG;
  if (jump - n < 6) n = n - jump + div(jump + 4, 33) * 33;
  let leap = mod(mod(n + 1, 33) - 1, 4);
  if (leap === -1) leap = 4;
  return { leap, gy, march };
}

function g2d(gy, gm, gd) {
  let d = div((gy + div(gm - 8, 6) + 100100) * 1461, 4) + div(153 * mod(gm + 9, 12) + 2, 5) + gd - 34840408;
  d = d - div(div(gy + 100100 + div(gm - 8, 6), 100) * 3, 4) + 752;
  return d;
}

function d2g(jdn) {
  let j = 4 * jdn + 139361631;
  j += div(div(4 * jdn + 183187720, 146097) * 3, 4) * 4 - 3908;
  const i = div(mod(j, 1461), 4) * 5 + 308;
  const gd = div(mod(i, 153), 5) + 1;
  const gm = mod(div(i, 153), 12) + 1;
  const gy = div(j, 1461) - 100100 + div(8 - gm, 6);
  return { gy, gm, gd };
}

function d2j(jdn) {
  const gy = d2g(jdn).gy;
  let jy = gy - 621;
  const r = jalCal(jy);
  const jdn1f = g2d(gy, 3, r.march);
  let k = jdn - jdn1f;
  let jm;
  let jd;
  if (k >= 0) {
    if (k <= 185) {
      jm = 1 + div(k, 31);
      jd = mod(k, 31) + 1;
      return { jy, jm, jd };
    }
    k -= 186;
  } else {
    jy -= 1;
    k += 179;
    if (r.leap === 1) k += 1;
  }
  jm = 7 + div(k, 30);
  jd = mod(k, 30) + 1;
  return { jy, jm, jd };
}

function j2d(jy, jm, jd) {
  const r = jalCal(jy);
  return g2d(r.gy, 3, r.march) + (jm - 1) * 31 - div(jm, 7) * (jm - 7) + jd - 1;
}

export const toJalali = (gy, gm, gd) => d2j(g2d(gy, gm, gd));
export const toGregorian = (jy, jm, jd) => d2g(j2d(jy, jm, jd));

export function jMonthLen(jy, jm) {
  if (jm <= 6) return 31;
  if (jm <= 11) return 30;
  return jalCal(jy).leap === 0 ? 30 : 29;
}

export function calMonthLen(y, m) {
  if (S.cal === 'jalali') return jMonthLen(y, m);
  if (m === 2) return (y % 4 === 0 && (y % 100 !== 0 || y % 400 === 0)) ? 29 : 28;
  return [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31][m - 1];
}

const J_MONTHS = ['فروردین', 'اردیبهشت', 'خرداد', 'تیر', 'مرداد', 'شهریور', 'مهر', 'آبان', 'آذر', 'دی', 'بهمن', 'اسفند'];
const J_MONTHS_EN = ['Farvardin', 'Ordibehesht', 'Khordad', 'Tir', 'Mordad', 'Shahrivar', 'Mehr', 'Aban', 'Azar', 'Dey', 'Bahman', 'Esfand'];
const G_MONTHS_FA = ['ژانویه', 'فوریه', 'مارس', 'آوریل', 'مه', 'ژوئن', 'ژوئیه', 'اوت', 'سپتامبر', 'اکتبر', 'نوامبر', 'دسامبر'];
const G_MONTHS_EN = ['January', 'February', 'March', 'April', 'May', 'June', 'July', 'August', 'September', 'October', 'November', 'December'];

export function calMonths() {
  if (S.cal === 'jalali') return S.lang === 'fa' ? J_MONTHS : J_MONTHS_EN;
  return S.lang === 'fa' ? G_MONTHS_FA : G_MONTHS_EN;
}

export const p2 = (n) => String(n).padStart(2, '0');
export const isoOf = (y, m, d) => `${y}-${p2(m)}-${p2(d)}`;

export function todayISO() {
  const n = new Date();
  return isoOf(n.getFullYear(), n.getMonth() + 1, n.getDate());
}

export function parseISO(iso) {
  return new Date(String(iso).slice(0, 10) + 'T12:00:00');
}

export function addDays(iso, n) {
  const d = parseISO(iso);
  d.setDate(d.getDate() + n);
  return isoOf(d.getFullYear(), d.getMonth() + 1, d.getDate());
}

export function isoToCal(iso) {
  if (!iso) return null;
  const [y, m, d] = String(iso).slice(0, 10).split('-').map(Number);
  if (!y || !m || !d) return null;
  if (S.cal === 'jalali') {
    const j = toJalali(y, m, d);
    return { y: j.jy, m: j.jm, d: j.jd };
  }
  return { y, m, d };
}

export function calToISO(c) {
  if (!c || !c.y || !c.m || !c.d) return '';
  if (S.cal === 'jalali') {
    const g = toGregorian(+c.y, +c.m, +c.d);
    return isoOf(g.gy, g.gm, g.gd);
  }
  return isoOf(+c.y, +c.m, +c.d);
}

export function fmtDate(iso, opts = {}) {
  if (!iso) return '—';
  const c = isoToCal(iso);
  if (!c) return esc(iso);
  if (opts.numeric) return faNum(`${c.y}/${p2(c.m)}/${p2(c.d)}`);
  if (opts.short) return `${faNum(c.d)} ${calMonths()[c.m - 1]}`;
  return `${faNum(c.d)} ${calMonths()[c.m - 1]} ${faNum(c.y)}`;
}

export function fmtTime(hm) {
  return faNum(String(hm || '').slice(0, 5));
}

export function fmtDateTime(ts) {
  if (!ts) return '—';
  const d = new Date(ts);
  if (Number.isNaN(d.getTime())) return esc(ts);
  return `${fmtDate(isoOf(d.getFullYear(), d.getMonth() + 1, d.getDate()))} · ${faNum(p2(d.getHours()) + ':' + p2(d.getMinutes()))}`;
}

export function ageOf(dob) {
  if (!dob) return null;
  const b = parseISO(dob);
  const n = new Date();
  let a = n.getFullYear() - b.getFullYear();
  const m = n.getMonth() - b.getMonth();
  if (m < 0 || (m === 0 && n.getDate() < b.getDate())) a -= 1;
  return a >= 0 && a < 130 ? a : null;
}

export function fmtAge(dob) {
  const a = ageOf(dob);
  return a === null ? t('common.na') : `${faNum(a)} ${t('common.years')}`;
}

export function weekday(iso) {
  return parseISO(iso).getDay();
}

export function ago(ts) {
  const mins = Math.max(0, Math.round((Date.now() - new Date(ts).getTime()) / 60000));
  if (mins < 1) return bi('همین حالا', 'just now');
  if (mins < 60) return `${faNum(mins)} ${t('common.minutes')}`;
  if (mins < 1440) return `${faNum(Math.round(mins / 60))} ${t('common.hours')}`;
  return `${faNum(Math.round(mins / 1440))} ${t('common.days')}`;
}

/* ------------------------------------------------------------------- names */

export const patientName = (p) => p ? bi(
  `${p.firstNameFa || ''} ${p.lastNameFa || ''}`.trim(),
  `${p.firstNameEn || p.firstNameFa || ''} ${p.lastNameEn || p.lastNameFa || ''}`.trim(),
) : '—';

export const doctorName = (d) => d ? bi(d.nameFa, d.nameEn) : '—';

export const personName = (u) => u ? bi(u.nameFa, u.nameEn) : '—';

export const initials = (name) => (String(name || '?').trim()[0] || '?');

/* -------------------------------------------------------------- api client */

export class ApiError extends Error {
  constructor(status, payload) {
    super(payload?.message || `HTTP ${status}`);
    this.status = status;
    this.payload = payload;
  }
}

export async function api(path, { method = 'GET', body } = {}) {
  const res = await fetch(path, {
    method,
    credentials: 'same-origin',
    headers: { 'content-type': 'application/json', 'x-clinicos-csrf': '1' },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  const type = res.headers.get('content-type') || '';
  const payload = type.includes('application/json') ? await res.json() : await res.text();
  if (res.status === 401 && S.user) {
    S.user = null;
    window.dispatchEvent(new CustomEvent('clinicos:unauthorized'));
  }
  if (!res.ok) throw new ApiError(res.status, payload);
  return payload;
}

export const get = (path) => api(path);
export const post = (path, body) => api(path, { method: 'POST', body });
export const patch = (path, body) => api(path, { method: 'PATCH', body });
export const put = (path, body) => api(path, { method: 'PUT', body });
export const del = (path) => api(path, { method: 'DELETE' });

export function query(params) {
  const q = new URLSearchParams();
  Object.entries(params || {}).forEach(([k, v]) => {
    if (v !== undefined && v !== null && v !== '') q.set(k, v);
  });
  const s = q.toString();
  return s ? `?${s}` : '';
}

/* --------------------------------------------------------------- feedback */

export function toast(message, type = 'ok') {
  const root = document.getElementById('toast-root');
  if (!root) return;
  const node = document.createElement('div');
  node.className = `toast ${type}`;
  node.textContent = message;
  root.appendChild(node);
  setTimeout(() => node.remove(), 4200);
}

export function errorToast(err) {
  const msg = err?.payload?.message || err?.message || String(err);
  toast(msg, 'err');
  if (!err?.payload) console.error(err);
}

export function emptyState(icon, message) {
  return `<div class="empty">${ic(icon, 'ic-l')}<p>${esc(message)}</p></div>`;
}

export function spinner(label = '') {
  return `<div class="empty"><div class="spinner"></div><p>${esc(label || bi('در حال بارگذاری…', 'Loading…'))}</p></div>`;
}

/* ----------------------------------------------------------------- modals */

export const ACTIONS = {};
export const FORMS = {};
export const CHANGES = {};

export const onAction = (name, fn) => { ACTIONS[name] = fn; };
export const onForm = (name, fn) => { FORMS[name] = fn; };
export const onChange = (name, fn) => { CHANGES[name] = fn; };

export function openModal({ title = '', body = '', footer = '', size = '', onMount = null }) {
  const root = document.getElementById('modal-root');
  root.innerHTML = `<div class="modal-back" data-back>
    <div class="modal ${size}" role="dialog" aria-modal="true">
      <div class="modal-head"><h3>${esc(title)}</h3><button class="x-btn" data-close type="button" aria-label="${esc(t('common.close'))}">${ic('x')}</button></div>
      <div class="modal-body">${body}</div>
      ${footer ? `<div class="modal-foot">${footer}</div>` : ''}
    </div></div>`;
  if (onMount) onMount(root.querySelector('.modal'));
}

export function closeModal() {
  const root = document.getElementById('modal-root');
  if (root) root.innerHTML = '';
}

export function confirmDlg(title, message, onOk, { danger = false, okText = '' } = {}) {
  openModal({
    title,
    size: 'narrow',
    body: `<div class="alert ${danger ? 'red' : 'amber'}">${ic('alert')}<div>${esc(message)}</div></div>`,
    footer: `<button class="btn" data-close type="button">${t('common.cancel')}</button>
             <button class="btn ${danger ? 'err' : 'pri'}" type="button" data-act="modal.ok">${esc(okText || t('common.confirm_action'))}</button>`,
  });
  ACTIONS['modal.ok'] = async () => {
    closeModal();
    await onOk();
  };
}

export function installHandlers() {
  document.addEventListener('click', async (event) => {
    const closer = event.target.closest('[data-close]');
    if (closer) {
      const back = event.target.closest('[data-back]');
      if (back) back.remove();
      closeModal();
      return;
    }
    const backdrop = event.target.closest('[data-back]');
    if (backdrop && event.target === backdrop) {
      closeModal();
      return;
    }
    const actionEl = event.target.closest('[data-act]');
    if (actionEl) {
      const fn = ACTIONS[actionEl.dataset.act];
      if (fn) {
        event.preventDefault();
        try {
          await fn(actionEl, event);
        } catch (err) {
          errorToast(err);
        }
      }
      return;
    }
    const linkEl = event.target.closest('[data-link]');
    if (linkEl) {
      event.preventDefault();
      go(linkEl.dataset.link);
    }
  });

  document.addEventListener('submit', async (event) => {
    const form = event.target.closest('form[data-form]');
    if (!form) return;
    event.preventDefault();
    const fn = FORMS[form.dataset.form];
    if (!fn) return;
    const button = form.querySelector('button[type="submit"]');
    if (button) button.disabled = true;
    try {
      await fn(form);
    } catch (err) {
      errorToast(err);
    } finally {
      if (button) button.disabled = false;
    }
  });

  document.addEventListener('change', async (event) => {
    const el = event.target.closest('[data-change]');
    if (!el) return;
    const fn = CHANGES[el.dataset.change];
    if (!fn) return;
    try {
      await fn(el, event);
    } catch (err) {
      errorToast(err);
    }
  });

  document.addEventListener('keydown', (event) => {
    if (event.key === 'Escape') closeModal();
  });
}

/* -------------------------------------------------------------- navigation */

export function go(hash) {
  if (location.hash === hash) {
    window.dispatchEvent(new HashChangeEvent('hashchange'));
  } else {
    location.hash = hash;
  }
}

/* ---------------------------------------------------------------- markup */

export function field(label, input, cls = 'f-4', { note = '', required = false } = {}) {
  return `<div class="field ${cls}"><label>${esc(label)}${required ? ' <span class="req">*</span>' : ''}</label>${input}${note ? `<div class="fnote">${esc(note)}</div>` : ''}</div>`;
}

export function textField(label, name, value = '', cls = 'f-4', opts = {}) {
  return field(label, `<input name="${name}" value="${esc(value)}" ${opts.type ? `type="${opts.type}"` : ''} ${opts.required ? 'required' : ''} ${opts.placeholder ? `placeholder="${esc(opts.placeholder)}"` : ''} ${opts.dir ? `dir="${opts.dir}"` : ''} ${opts.min !== undefined ? `min="${opts.min}"` : ''} ${opts.max !== undefined ? `max="${opts.max}"` : ''}>`, cls, opts);
}

export function textArea(label, name, value = '', cls = 'f-6', opts = {}) {
  return field(label, `<textarea name="${name}" rows="${opts.rows || 3}" ${opts.required ? 'required' : ''}>${esc(value)}</textarea>`, cls, opts);
}

export function selectField(label, name, options, cls = 'f-4', value = '', opts = {}) {
  return field(label, `<select name="${name}" ${opts.required ? 'required' : ''}>${options.map(([val, text]) => `<option value="${esc(val)}" ${String(value) === String(val) ? 'selected' : ''}>${esc(text)}</option>`).join('')}</select>`, cls, opts);
}

export function dateField(name, value = '', cls = 'f-4', opts = {}) {
  const c = value ? isoToCal(value) : null;
  const now = new Date();
  const curY = S.cal === 'jalali'
    ? toJalali(now.getFullYear(), now.getMonth() + 1, now.getDate()).jy
    : now.getFullYear();
  const [lo, hi] = opts.range || [curY - 1, curY + 2];
  const years = [];
  for (let y = hi; y >= lo; y -= 1) years.push(y);
  const months = calMonths();
  const cy = c?.y || '';
  const cm = c?.m || '';
  const len = c ? calMonthLen(c.y, c.m) : 31;
  const days = [];
  for (let d = 1; d <= len; d += 1) days.push(d);
  const inner = `<div class="datefield" data-datefield="${name}">
    <select name="${name}__y" data-change="date.rebuild"><option value="">—</option>${years.map((y) => `<option value="${y}" ${String(cy) === String(y) ? 'selected' : ''}>${faNum(y)}</option>`).join('')}</select>
    <select name="${name}__m" data-change="date.rebuild"><option value="">—</option>${months.map((m, i) => `<option value="${i + 1}" ${String(cm) === String(i + 1) ? 'selected' : ''}>${esc(m)}</option>`).join('')}</select>
    <select class="df-y" name="${name}__d">${days.map((d) => `<option value="${d}" ${String(c?.d) === String(d) ? 'selected' : ''}>${faNum(d)}</option>`).join('')}</select>
  </div>`;
  return field(opts.label || t('common.date'), inner, cls, opts);
}

export function readDate(form, name) {
  const y = form.querySelector(`[name="${name}__y"]`);
  const m = form.querySelector(`[name="${name}__m"]`);
  const d = form.querySelector(`[name="${name}__d"]`);
  if (!y || !m || !d) {
    const plain = form.querySelector(`[name="${name}"]`);
    return plain ? plain.value : '';
  }
  if (!y.value || !m.value || !d.value) return '';
  return calToISO({ y: +y.value, m: +m.value, d: +d.value });
}

onChange('date.rebuild', (el) => {
  const box = el.closest('[data-datefield]');
  if (!box) return;
  const y = box.querySelector('select[name$="__y"]');
  const m = box.querySelector('select[name$="__m"]');
  const d = box.querySelector('select[name$="__d"]');
  const keep = +d.value || 1;
  if (!y.value || !m.value) {
    d.innerHTML = Array.from({ length: 31 }, (_, i) => i + 1)
      .map((n) => `<option value="${n}">${faNum(n)}</option>`).join('');
    return;
  }
  const len = calMonthLen(+y.value, +m.value);
  d.innerHTML = Array.from({ length: len }, (_, i) => i + 1)
    .map((n) => `<option value="${n}" ${n === Math.min(keep, len) ? 'selected' : ''}>${faNum(n)}</option>`).join('');
});

export function formValues(form) {
  const out = {};
  new FormData(form).forEach((value, key) => {
    if (key.endsWith('__y') || key.endsWith('__m') || key.endsWith('__d')) return;
    out[key] = typeof value === 'string' ? value.trim() : value;
  });
  return out;
}

export const bool = (v) => v === true || v === 'on' || v === 'true' || v === '1';

/* --------------------------------------------------------------- badges */

const APPT_COLORS = {
  requested: 'b-gray', confirmed: 'b-blue', arrived: 'b-cyan', checked_in: 'b-cyan',
  waiting: 'b-amber', in_progress: 'b-violet', completed: 'b-green', cancelled: 'b-red',
  no_show: 'b-dark', rescheduled: 'b-gray',
};
const INV_COLORS = {
  unpaid: 'b-red', partial: 'b-amber', paid: 'b-green', refunded: 'b-violet',
  cancelled: 'b-gray', pending_verify: 'b-blue',
};
const LAB_COLORS = {
  ordered: 'b-blue', collected: 'b-cyan', in_progress: 'b-amber', completed: 'b-violet',
  reviewed: 'b-green', cancelled: 'b-gray',
};
const QUEUE_COLORS = { waiting: 'b-amber', called: 'b-cyan', in_progress: 'b-violet', done: 'b-green', cancelled: 'b-gray' };
const ENC_COLORS = { draft: 'b-amber', signed: 'b-green', amended: 'b-violet', entered_in_error: 'b-red' };
const FLAG_COLORS = { normal: 'b-green', abnormal: 'b-amber', critical: 'b-red' };

export function badge(text, color = 'b-gray') {
  return `<span class="badge ${color}"><i></i>${esc(text)}</span>`;
}

export function apptBadge(status) {
  return badge(t('status.' + status), APPT_COLORS[status] || 'b-gray');
}
export function invBadge(status) {
  return badge(t('inv.status.' + status), INV_COLORS[status] || 'b-gray');
}
export function labBadge(status) {
  return badge(t('lab.status.' + status), LAB_COLORS[status] || 'b-gray');
}
export function queueBadge(status) {
  const key = status === 'done' ? 'status.completed' : status === 'cancelled' ? 'status.cancelled' : 'status.' + status;
  return badge(t(key), QUEUE_COLORS[status] || 'b-gray');
}
export function encBadge(status) {
  return badge(t('enc.v_' + status), ENC_COLORS[status] || 'b-gray');
}
export function flagBadge(flag) {
  const label = flag === 'critical' ? t('common.critical') : flag === 'abnormal' ? t('common.abnormal') : t('common.normal');
  return badge(label, FLAG_COLORS[flag] || 'b-green');
}

export function pager(meta, action) {
  if (!meta || meta.pages <= 1) return '';
  const page = meta.page;
  const pages = meta.pages;
  const from = Math.max(1, page - 2);
  const to = Math.min(pages, from + 4);
  let html = `<div class="tbl-foot"><span>${faNum(meta.total)} ${bi('مورد', 'items')}</span><div class="pager">`;
  html += `<button data-act="${action}" data-page="${page - 1}" ${page <= 1 ? 'disabled' : ''}>‹</button>`;
  for (let p = from; p <= to; p += 1) {
    html += `<button data-act="${action}" data-page="${p}" class="${p === page ? 'on' : ''}">${faNum(p)}</button>`;
  }
  html += `<button data-act="${action}" data-page="${page + 1}" ${page >= pages ? 'disabled' : ''}>›</button></div></div>`;
  return html;
}

/* --------------------------------------------------------- patient picker */

export function patientPicker(name, label = bi('بیمار', 'Patient'), cls = 'f-6') {
  const id = `pp-${name}`;
  return `<div class="field ${cls}"><label>${esc(label)} <span class="req">*</span></label>
    <input id="${id}-search" placeholder="${esc(bi('نام، کد یا تلفن بیمار…', 'Patient name, code or phone…'))}" autocomplete="off" data-change="picker.search" data-target="${id}">
    <input type="hidden" name="${name}" id="${id}">
    <div id="${id}-chosen" class="mini-note"></div>
    <div id="${id}-results" class="stack" style="margin-top:6px"></div>
  </div>`;
}

onChange('picker.search', async (el) => {
  const target = el.dataset.target;
  const box = document.getElementById(`${target}-results`);
  const term = el.value.trim();
  if (!box) return;
  if (term.length < 2) {
    box.innerHTML = '';
    return;
  }
  const data = await get(`/api/patients?q=${encodeURIComponent(term)}&perPage=6`);
  box.innerHTML = data.items.length ? data.items.map((p) => `<button type="button" class="btn sm" style="justify-content:flex-start" data-act="picker.select" data-target="${target}" data-id="${esc(p.id)}" data-label="${esc(patientName(p) + ' · ' + p.code)}">
      ${ic('user', 'ic-s')} ${esc(patientName(p))} <span class="mini-note">${esc(p.code)}</span></button>`).join('')
    : `<div class="mini-note">${esc(bi('بیماری یافت نشد', 'No patient found'))}</div>`;
});

onAction('picker.select', (el) => {
  const target = el.dataset.target;
  const hidden = document.getElementById(target);
  const chosen = document.getElementById(`${target}-chosen`);
  const results = document.getElementById(`${target}-results`);
  if (hidden) hidden.value = el.dataset.id;
  if (chosen) chosen.textContent = el.dataset.label;
  if (results) results.innerHTML = '';
  const search = document.getElementById(`${target}-search`);
  if (search) search.value = '';
});

/* ------------------------------------------------------------------ print */

/* -------------------------------------------------------------- lifecycle */

let afterRenderHook = null;
export function after(fn) { afterRenderHook = fn; }
export function runAfter() {
  const fn = afterRenderHook;
  afterRenderHook = null;
  if (fn) fn();
}
export function resetAfter() { afterRenderHook = null; }

export const refresh = () => window.dispatchEvent(new CustomEvent('clinicos:refresh'));

export function printDoc(html) {
  const root = document.getElementById('print-root');
  if (!root) return;
  root.innerHTML = html;
  window.print();
}
