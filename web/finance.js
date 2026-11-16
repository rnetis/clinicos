// Finance: cashier dashboard, invoices, payments, refunds and reports.
import {
  S, t, bi, esc, can, faNum, money, fmtDate, fmtDateTime, todayISO, addDays, isoToCal,
  get, post, go, toast, refresh, query, onAction, onForm, onChange, openModal, closeModal,
  confirmDlg, emptyState, pager, textField, textArea, selectField, dateField, readDate,
  formValues, invBadge, badge, printDoc, patientPicker, doctorName,
} from './lib.js';
import { ic } from './icons.js';

const invState = { q: '', status: '', from: '', to: '' };
const repState = { from: addDays(todayISO(), -29), to: todayISO() };

const METHODS = [['cash', 'نقدی', 'Cash'], ['card', 'کارت', 'Card'], ['transfer', 'انتقال بانکی', 'Transfer'], ['insurance', 'بیمه', 'Insurance'], ['wallet', 'کیف پول', 'Wallet']];
const STATUSES = ['unpaid', 'partial', 'paid', 'refunded', 'cancelled'];
const methodLabel = (m) => { const f = METHODS.find(([k]) => k === m); return f ? bi(f[1], f[2]) : m; };
const serviceList = () => S.meta?.services || [];
const insurerList = () => S.meta?.insurers || [];

function totalRow(label, value, cls = '') {
  return `<div class="total-row ${cls}"><span>${esc(label)}</span><b>${esc(money(value))} ${esc(t('common.currency'))}</b></div>`;
}

/* --------------------------------------------------------------- dashboard */

export async function renderFinanceHome() {
  const [dash, invoices] = await Promise.all([
    get('/api/dashboard'),
    get('/api/invoices?perPage=6'),
  ]);
  const k = dash.kpis || {};
  const trend = dash.revenueTrend || [];
  const peak = Math.max(1, ...trend.map((x) => x.amount || 0));
  return `<div class="page-head"><div class="grow"><h2>${esc(bi('میز کار مالی', 'Finance desk'))}</h2>
      <div class="sub">${esc(fmtDate(dash.date))} · ${esc(bi('دریافت‌ها، بدهی‌ها و گزارش روز', 'Collections, receivables and daily performance'))}</div></div>
      <div class="head-actions">
        <button class="btn pri" data-act="inv.new">${ic('plus')}${esc(bi('صورتحساب جدید', 'New invoice'))}</button>
        <button class="btn" data-link="#/finance/invoices">${ic('receipt')}${esc(t('nav.invoices'))}</button>
      </div></div>
    <div class="kpi-grid">
      <div class="stat ok"><div class="lbl">${ic('money')}${esc(bi('دریافتی امروز', 'Collected today'))}</div><div class="val">${esc(money(k.revenueToday || 0))}</div></div>
      <div class="stat warn"><div class="lbl">${ic('receipt')}${esc(bi('مانده مطالبات', 'Outstanding'))}</div><div class="val">${esc(money(k.outstanding || 0))}</div></div>
      <div class="stat"><div class="lbl">${ic('users')}${esc(bi('بیماران جدید', 'New patients'))}</div><div class="val">${faNum(k.newPatients || 0)}</div></div>
      <div class="stat"><div class="lbl">${ic('calendar')}${esc(bi('نوبت‌های امروز', "Today's appointments"))}</div><div class="val">${faNum(k.appointments || 0)}</div></div>
    </div>
    <div class="grid2">
      <div class="card"><h3>${ic('chart')}${esc(bi('دریافتی ۷ روز گذشته', 'Collections — last 7 days'))}</h3>
        <div class="bars">${trend.map((x) => `<div class="bar-col" title="${esc(fmtDate(x.date))}">
          <div class="bar" style="height:${Math.max(4, Math.round((x.amount / peak) * 110))}px"></div>
          <div class="mini-note">${esc(fmtDate(x.date).slice(0, 6))}</div></div>`).join('')}</div>
        <div class="mini-note mt8">${esc(bi('بیشترین روز:', 'Peak:'))} ${esc(money(peak))} ${esc(t('common.currency'))}</div>
      </div>
      <div class="card"><h3>${ic('receipt')}${esc(bi('آخرین صورتحساب‌ها', 'Latest invoices'))}</h3>
        ${(invoices.items || []).length ? `<div class="stack">${invoices.items.map((i) => `<div class="flex-b" style="border-bottom:1px solid var(--line);padding-bottom:7px">
          <div><a class="rowlink" href="#/invoice/${esc(i.id)}">${esc(i.no)}</a>
            <div class="mini-note">${esc(bi(i.patientName.fa, i.patientName.en))} · ${esc(fmtDate(i.date))}</div></div>
          <div style="text-align:end"><b>${esc(money(i.due))}</b><div>${invBadge(i.status)}</div></div>
        </div>`).join('')}</div>` : emptyState('receipt', bi('صورتحسابی ثبت نشده', 'No invoices yet'))}
      </div>
    </div>`;
}

/* ---------------------------------------------------------------- invoices */

export async function renderInvoices() {
  const data = await get(`/api/invoices${query({ q: invState.q, status: invState.status, from: invState.from, to: invState.to, perPage: 100 })}`);
  const rows = data.items || [];
  const sum = (key) => rows.reduce((s, r) => s + (r[key] || 0), 0);
  return `<div class="page-head"><div class="grow"><h2>${esc(t('nav.invoices'))}</h2>
      <div class="sub">${esc(bi('جست‌وجو، وضعیت پرداخت و پیگیری مطالبات', 'Search, payment status and receivables'))}</div></div>
      <div class="head-actions"><button class="btn pri" data-act="inv.new">${ic('plus')}${esc(bi('صورتحساب جدید', 'New invoice'))}</button></div></div>
    <div class="card"><form data-form="inv.filter" class="inline-fields">
      <div class="field grow"><label>${esc(t('common.search'))}</label><input name="q" value="${esc(invState.q)}" placeholder="${esc(bi('شماره یا نام بیمار', 'Number or patient'))}"></div>
      <div class="field"><label>${esc(t('common.status'))}</label><select name="status">
        <option value="">${esc(t('common.all'))}</option>
        ${STATUSES.map((s) => `<option value="${s}" ${invState.status === s ? 'selected' : ''}>${esc(t('inv.status.' + s))}</option>`).join('')}</select></div>
      <div class="field"><label>${esc(bi('از تاریخ', 'From'))}</label><input type="date" name="from" value="${esc(invState.from)}"></div>
      <div class="field"><label>${esc(bi('تا تاریخ', 'To'))}</label><input type="date" name="to" value="${esc(invState.to)}"></div>
      <button class="btn pri" type="submit">${ic('search')}${esc(bi('فیلتر', 'Filter'))}</button>
      <button class="btn" type="button" data-act="inv.clear">${esc(bi('پاک‌کردن', 'Clear'))}</button>
    </form></div>
    <div class="tbl-wrap mt14"><table class="tbl"><thead><tr>
      <th>${esc(bi('شماره', 'No.'))}</th><th>${esc(bi('تاریخ', 'Date'))}</th><th>${esc(bi('بیمار', 'Patient'))}</th>
      <th>${esc(bi('مبلغ', 'Amount'))}</th><th>${esc(bi('پرداخت', 'Paid'))}</th><th>${esc(bi('مانده', 'Balance'))}</th><th>${esc(t('common.status'))}</th><th></th>
    </tr></thead><tbody>${rows.length ? rows.map((i) => `<tr>
      <td class="mono"><a class="rowlink" href="#/invoice/${esc(i.id)}">${esc(i.no)}</a></td>
      <td>${esc(fmtDate(i.date))}</td>
      <td><a class="rowlink" href="#/patient/${esc(i.patientId)}">${esc(bi(i.patientName.fa, i.patientName.en))}</a><div class="mini-note">${esc(i.patientCode)}</div></td>
      <td class="num">${esc(money(i.due))}</td>
      <td class="num">${esc(money(i.paid))}</td>
      <td class="num">${i.outstanding > 0 ? `<b style="color:var(--err)">${esc(money(i.outstanding))}</b>` : '—'}</td>
      <td>${invBadge(i.status)}</td>
      <td><a class="btn xs" href="#/invoice/${esc(i.id)}">${esc(bi('بازکردن', 'Open'))}</a></td>
    </tr>`).join('') : `<tr><td colspan="8">${emptyState('receipt', bi('صورتحسابی یافت نشد', 'No invoices found'))}</td></tr>`}</tbody>
    ${rows.length ? `<tfoot><tr><td colspan="3"><b>${esc(bi('جمع', 'Total'))}</b></td>
      <td class="num"><b>${esc(money(sum('due')))}</b></td><td class="num"><b>${esc(money(sum('paid')))}</b></td>
      <td class="num"><b>${esc(money(sum('outstanding')))}</b></td><td colspan="2"></td></tr></tfoot>` : ''}</table></div>`;
}

onForm('inv.filter', (form) => {
  const v = formValues(form);
  invState.q = v.q || '';
  invState.status = v.status || '';
  invState.from = v.from || '';
  invState.to = v.to || '';
  refresh();
});
onAction('inv.clear', () => { invState.q = ''; invState.status = ''; invState.from = ''; invState.to = ''; refresh(); });

export async function renderInvoice(id) {
  const inv = await get(`/api/invoices/${id}`);
  const tt = inv.totals || {};
  const open = !['cancelled', 'refunded'].includes(inv.status);
  return `<div class="page-head">
      <div class="grow"><h2>${esc(bi('صورتحساب', 'Invoice'))} ${esc(inv.no)}</h2>
        <div class="sub">${esc(fmtDate(inv.date))} · ${esc(bi(inv.patient.name.fa, inv.patient.name.en))} · ${esc(inv.patient.code)}</div></div>
      <div class="head-actions">
        <button class="btn" data-act="inv.print" data-id="${esc(inv.id)}">${ic('print')}${esc(t('common.print'))}</button>
        ${open && can('payment.process') ? `<button class="btn pri" data-act="inv.pay" data-id="${esc(inv.id)}" data-due="${tt.outstanding}">${ic('money')}${esc(bi('ثبت پرداخت', 'Add payment'))}</button>` : ''}
        ${open && can('refund.manage') && tt.paid > 0 ? `<button class="btn warn" data-act="inv.refund" data-id="${esc(inv.id)}" data-paid="${tt.paid}">${esc(bi('بازگشت وجه', 'Refund'))}</button>` : ''}
        ${open && can('payment.process') && tt.paid === 0 ? `<button class="btn err" data-act="inv.cancel" data-id="${esc(inv.id)}">${esc(bi('لغو صورتحساب', 'Cancel invoice'))}</button>` : ''}
        <a class="btn" href="#/patient/${esc(inv.patientId)}">${ic('user')}${esc(bi('پرونده', 'Record'))}</a>
      </div></div>
    <div class="grid2">
      <div class="card">
        <div class="flex-b mb8"><h3 style="margin:0">${ic('receipt')}${esc(bi('اقلام', 'Line items'))}</h3>${invBadge(inv.status)}</div>
        <div class="tbl-wrap"><table class="tbl"><thead><tr><th>${esc(bi('شرح', 'Description'))}</th><th>${esc(bi('تعداد', 'Qty'))}</th><th>${esc(bi('مبلغ واحد', 'Unit'))}</th><th>${esc(bi('جمع', 'Total'))}</th></tr></thead>
          <tbody>${inv.items.map((x) => `<tr><td>${esc(bi(x.desc.fa, x.desc.en))}</td><td class="num">${faNum(x.qty)}</td>
            <td class="num">${esc(money(x.price))}</td><td class="num">${esc(money(x.total))}</td></tr>`).join('')}</tbody></table></div>
        <div class="totals mt14">
          ${totalRow(bi('جمع کل', 'Subtotal'), tt.subtotal)}
          ${tt.discount ? totalRow(bi('تخفیف', 'Discount'), -tt.discount) : ''}
          ${tt.insurance ? totalRow(`${bi('سهم بیمه', 'Insurance')}${inv.insurerName?.fa ? ' · ' + bi(inv.insurerName.fa, inv.insurerName.en) : ''}`, -tt.insurance) : ''}
          ${totalRow(bi('قابل پرداخت', 'Amount due'), tt.due, 'strong')}
          ${totalRow(bi('پرداخت‌شده', 'Paid'), tt.paid)}
          ${totalRow(bi('مانده', 'Outstanding'), tt.outstanding, tt.outstanding > 0 ? 'warn' : '')}
        </div>
        ${inv.status === 'cancelled' ? `<div class="alert red mt14">${ic('alert')}<div>${esc(bi('لغو شده', 'Cancelled'))}: ${esc(inv.cancelReason || '')}</div></div>` : ''}
      </div>
      <div class="stack">
        <div class="card"><h3>${ic('user')}${esc(bi('مشخصات', 'Details'))}</h3>
          <div class="kv"><span>${esc(bi('بیمار', 'Patient'))}</span><b>${esc(bi(inv.patient.name.fa, inv.patient.name.en))}</b></div>
          <div class="kv"><span>${esc(bi('کد ملی', 'National ID'))}</span><b class="mono">${esc(inv.patient.nationalId || '—')}</b></div>
          <div class="kv"><span>${esc(bi('تلفن', 'Phone'))}</span><b class="mono">${esc(inv.patient.phone || '—')}</b></div>
          <div class="kv"><span>${esc(bi('پزشک', 'Physician'))}</span><b>${esc(bi(inv.doctorName.fa, inv.doctorName.en))}</b></div>
          <div class="kv"><span>${esc(bi('بیمه', 'Insurer'))}</span><b>${esc(bi(inv.insurerName.fa, inv.insurerName.en) || '—')}</b></div>
        </div>
        <div class="card"><h3>${ic('money')}${esc(bi('پرداخت‌ها', 'Payments'))}</h3>
          ${inv.payments.length ? `<div class="stack">${inv.payments.map((p) => `<div class="flex-b" style="border-bottom:1px solid var(--line);padding-bottom:6px">
            <div><b style="color:${p.kind === 'refund' ? 'var(--err)' : 'var(--ok)'}">${p.kind === 'refund' ? '−' : '+'}${esc(money(p.amount))}</b>
              <div class="mini-note">${esc(methodLabel(p.method))} · ${esc(fmtDateTime(p.at))}${p.ref ? ' · ' + esc(p.ref) : ''}</div></div>
            <div class="mini-note">${esc(bi(p.by.fa, p.by.en))}</div>
          </div>`).join('')}</div>` : emptyState('money', bi('پرداختی ثبت نشده', 'No payments recorded'))}
        </div>
      </div>
    </div>`;
}

/* ------------------------------------------------------------ new invoice */

function itemRow() {
  return `<div class="inv-row">
    <select class="item-service">${['', ...serviceList().map((s) => s.id)].map((id) => {
      const s = serviceList().find((x) => x.id === id);
      return `<option value="${esc(id)}" data-price="${s ? s.price : 0}">${esc(id ? `${s.code} · ${bi(s.nameFa, s.nameEn)}` : bi('انتخاب خدمت', 'Select service'))}</option>`;
    }).join('')}</select>
    <input class="item-qty" type="number" min="1" max="100" value="1" title="${esc(bi('تعداد', 'Qty'))}">
    <input class="item-price" type="number" min="0" step="1000" value="0" title="${esc(bi('مبلغ', 'Price'))}">
    <button class="btn xs err" type="button" data-act="inv.delitem">${ic('x', 'ic-s')}</button>
  </div>`;
}

function recalc(modal) {
  const rows = [...modal.querySelectorAll('.inv-row')];
  const subtotal = rows.reduce((sum, row) => {
    const qty = Number(row.querySelector('.item-qty').value) || 0;
    const price = Number(row.querySelector('.item-price').value) || 0;
    return sum + qty * price;
  }, 0);
  const discount = Number(modal.querySelector('[name="discount"]')?.value) || 0;
  const cover = Number(modal.querySelector('[name="cover"]')?.value) || 0;
  const due = Math.max(0, Math.round((subtotal - discount) * (1 - cover / 100)));
  const out = modal.querySelector('#inv-preview');
  if (out) out.innerHTML = `<span>${esc(bi('جمع', 'Subtotal'))}: <b>${esc(money(subtotal))}</b></span>
    <span>${esc(bi('تخفیف', 'Discount'))}: <b>${esc(money(discount))}</b></span>
    <span>${esc(bi('سهم بیمه', 'Cover'))}: <b>${esc(faNum(cover))}%</b></span>
    <span>${esc(bi('قابل پرداخت', 'Due'))}: <b style="color:var(--pri)">${esc(money(due))}</b></span>`;
}

function invoiceModal(presetPatient = '') {
  openModal({
    title: bi('صورتحساب جدید', 'New invoice'),
    size: 'wide',
    body: `<form data-form="inv.create">
      <div class="form">
        ${patientPicker('patientId', bi('بیمار', 'Patient'), 'f-12')}
        ${selectField(bi('پزشک', 'Physician'), 'doctorId', [['', bi('بدون پزشک', 'None')]].concat((S.meta?.doctors || []).map((d) => [d.id, doctorName(d)])), 'f-4', '')}
        ${selectField(bi('بیمه', 'Insurer'), 'insurerId', [['', bi('بدون بیمه', 'None')]].concat(insurerList().map((i) => [i.id, `${bi(i.nameFa, i.nameEn)} (${faNum(i.cover)}%)`])), 'f-4', '')}
        ${selectField(bi('درصد پوشش', 'Cover %'), 'cover', Array.from({ length: 11 }, (_, i) => [String(i * 10), faNum(i * 10) + '%']), 'f-4', '0')}
        <div class="field f-12"><label>${esc(bi('اقلام', 'Line items'))}</label>
          <div id="inv-items">${itemRow()}</div>
          <button class="btn sm mt8" type="button" data-act="inv.additem">${ic('plus')}${esc(bi('افزودن قلم', 'Add item'))}</button></div>
        <div class="field f-4"><label>${esc(bi('تخفیف (ریال)', 'Discount (IRR)'))}</label><input type="number" name="discount" min="0" step="1000" value="0"></div>
        <div class="f-8" id="inv-preview"></div>
      </div>
      <div class="flex-b mt14">
        <button class="btn" type="button" data-close>${esc(t('common.cancel'))}</button>
        <button class="btn pri" type="submit">${esc(bi('صدور صورتحساب', 'Issue invoice'))}</button>
      </div>
    </form>`,
    onMount: (modal) => {
      const form = modal.querySelector('form');
      if (presetPatient) {
        const hidden = modal.querySelector('#pp-patientId');
        if (hidden) hidden.value = presetPatient;
      }
      form.addEventListener('change', (event) => {
        if (event.target.classList.contains('item-service')) {
          const price = Number(event.target.selectedOptions[0]?.dataset.price) || 0;
          const row = event.target.closest('.inv-row');
          row.querySelector('.item-price').value = price;
        }
        if (event.target.name === 'insurerId') {
          const ins = insurerList().find((i) => i.id === event.target.value);
          if (ins) form.querySelector('[name="cover"]').value = String(Math.round(ins.cover / 10) * 10);
        }
        recalc(modal);
      });
      form.addEventListener('input', () => recalc(modal));
      recalc(modal);
    },
  });
}

onAction('inv.new', (el) => invoiceModal(el?.dataset?.patient || ''));
onAction('inv.additem', () => {
  document.getElementById('inv-items')?.insertAdjacentHTML('beforeend', itemRow());
});
onAction('inv.delitem', (el) => {
  el.closest('.inv-row')?.remove();
  const modal = el.closest('.modal');
  if (modal) recalc(modal);
});

onForm('inv.create', async (form) => {
  const v = formValues(form);
  const modal = form.closest('.modal');
  const items = [...form.querySelectorAll('.inv-row')].map((row) => {
    const serviceId = row.querySelector('.item-service').value;
    const service = serviceList().find((s) => s.id === serviceId);
    return {
      serviceId: serviceId || null,
      descFa: service ? service.nameFa : '',
      descEn: service ? service.nameEn : '',
      qty: Number(row.querySelector('.item-qty').value) || 1,
      price: Number(row.querySelector('.item-price').value) || 0,
    };
  }).filter((item) => item.serviceId || item.price > 0);
  if (!v.patientId) throw new Error(bi('بیمار را انتخاب کنید', 'Select a patient'));
  if (!items.length) throw new Error(bi('حداقل یک قلم اضافه کنید', 'Add at least one line item'));
  const res = await post('/api/invoices', {
    patientId: v.patientId,
    doctorId: v.doctorId || null,
    insurerId: v.insurerId || null,
    cover: Number(v.cover) || 0,
    discount: Number(v.discount) || 0,
    items,
  });
  closeModal();
  toast(bi('صورتحساب صادر شد: ', 'Invoice issued: ') + res.no, 'ok');
  go(`#/invoice/${res.id}`);
});

/* ---------------------------------------------------------------- payments */

onAction('inv.pay', (el) => {
  openModal({
    title: bi('ثبت پرداخت', 'Add payment'),
    size: 'narrow',
    body: `<form data-form="inv.pay" data-id="${esc(el.dataset.id)}">
      <div class="form">
        ${textField(bi('مبلغ (ریال)', 'Amount (IRR)'), 'amount', el.dataset.due || '', 'f-12', { type: 'number', required: true, dir: 'ltr' })}
        ${selectField(bi('روش پرداخت', 'Payment method'), 'method', METHODS.map(([k, fa, en]) => [k, bi(fa, en)]), 'f-6', 'cash')}
        ${textField(bi('شماره پیگیری', 'Reference'), 'reference', '', 'f-6', { dir: 'ltr' })}
      </div>
      <div class="mini-note mt8">${esc(bi('مانده قابل پرداخت:', 'Outstanding:'))} <b>${esc(money(Number(el.dataset.due) || 0))}</b></div>
      <div class="flex-b mt14"><button class="btn" type="button" data-close>${esc(t('common.cancel'))}</button>
        <button class="btn pri" type="submit">${esc(bi('ثبت', 'Save'))}</button></div>
    </form>`,
  });
});

onForm('inv.pay', async (form) => {
  const v = formValues(form);
  const amount = Number(v.amount);
  if (!amount || amount <= 0) throw new Error(bi('مبلغ معتبر وارد کنید', 'Enter a valid amount'));
  await post(`/api/invoices/${form.dataset.id}/payments`, { amount, method: v.method, reference: v.reference || '' });
  closeModal();
  toast(bi('پرداخت ثبت شد', 'Payment recorded'), 'ok');
  refresh();
});

onAction('inv.refund', (el) => {
  openModal({
    title: bi('بازگشت وجه', 'Refund payment'),
    size: 'narrow',
    body: `<form data-form="inv.refund" data-id="${esc(el.dataset.id)}">
      <div class="alert amber">${ic('alert')}<div>${esc(bi('بازگشت وجه به‌صورت یک تراکنش منفی ثبت می‌شود و قابل حذف نیست.', 'A refund is recorded as a negative transaction and cannot be deleted.'))}</div></div>
      <div class="form">
        ${textField(bi('مبلغ (ریال)', 'Amount (IRR)'), 'amount', el.dataset.paid || '', 'f-12', { type: 'number', required: true, dir: 'ltr' })}
        ${textField(bi('دلیل', 'Reason'), 'reason', '', 'f-12', { required: true })}
      </div>
      <div class="flex-b mt14"><button class="btn" type="button" data-close>${esc(t('common.cancel'))}</button>
        <button class="btn warn" type="submit">${esc(bi('ثبت بازگشت', 'Refund'))}</button></div>
    </form>`,
  });
});

onForm('inv.refund', async (form) => {
  const v = formValues(form);
  await post(`/api/invoices/${form.dataset.id}/refund`, { amount: Number(v.amount), reason: v.reason });
  closeModal();
  toast(bi('بازگشت وجه ثبت شد', 'Refund recorded'), 'warn');
  refresh();
});

onAction('inv.cancel', (el) => {
  openModal({
    title: bi('لغو صورتحساب', 'Cancel invoice'),
    size: 'narrow',
    body: `<form data-form="inv.cancel" data-id="${esc(el.dataset.id)}">
      <div class="alert red">${ic('alert')}<div>${esc(bi('صورتحساب لغو می‌شود؛ این عمل قابل بازگشت نیست.', 'The invoice will be cancelled; this cannot be undone.'))}</div></div>
      <div class="field"><label>${esc(bi('دلیل لغو', 'Reason'))}</label><textarea name="reason" rows="3"></textarea></div>
      <div class="flex-b mt14"><button class="btn" type="button" data-close>${esc(t('common.cancel'))}</button>
        <button class="btn err" type="submit">${esc(bi('لغو صورتحساب', 'Cancel invoice'))}</button></div>
    </form>`,
  });
});

onForm('inv.cancel', async (form) => {
  await post(`/api/invoices/${form.dataset.id}/cancel`, { reason: formValues(form).reason || '' });
  closeModal();
  toast(bi('صورتحساب لغو شد', 'Invoice cancelled'), 'warn');
  refresh();
});

/* ------------------------------------------------------------------- print */

onAction('inv.print', async (el) => {
  const inv = await get(`/api/invoices/${el.dataset.id}`);
  const clinic = S.meta?.clinic || {};
  const tt = inv.totals;
  printDoc(`<div class="pr-doc">
    <div class="pr-head"><div><h2>${esc(clinic.nameFa || 'ClinicOS')}</h2><small>${esc(clinic.nameEn || '')} · ${esc(clinic.phone || '')}</small></div>
      <div style="text-align:end"><b>${esc(bi('صورتحساب', 'Invoice'))} ${esc(inv.no)}</b><div><small>${esc(fmtDate(inv.date))}</small></div></div></div>
    <div class="pr-meta"><div><span>${esc(bi('بیمار', 'Patient'))}:</span> ${esc(bi(inv.patient.name.fa, inv.patient.name.en))}</div>
      <div><span>${esc(bi('کد', 'Code'))}:</span> ${esc(inv.patient.code)}</div>
      <div><span>${esc(bi('تلفن', 'Phone'))}:</span> ${esc(inv.patient.phone || '')}</div>
      <div><span>${esc(bi('پزشک', 'Physician'))}:</span> ${esc(bi(inv.doctorName.fa, inv.doctorName.en))}</div></div>
    <table class="pr-tbl"><thead><tr><th>${esc(bi('شرح', 'Description'))}</th><th>${esc(bi('تعداد', 'Qty'))}</th><th>${esc(bi('مبلغ واحد', 'Unit'))}</th><th>${esc(bi('جمع', 'Total'))}</th></tr></thead>
      <tbody>${inv.items.map((x) => `<tr><td>${esc(bi(x.desc.fa, x.desc.en))}</td><td>${faNum(x.qty)}</td><td>${esc(money(x.price))}</td><td>${esc(money(x.total))}</td></tr>`).join('')}</tbody></table>
    <div class="pr-totals">
      <div><span>${esc(bi('جمع کل', 'Subtotal'))}</span><b>${esc(money(tt.subtotal))}</b></div>
      ${tt.discount ? `<div><span>${esc(bi('تخفیف', 'Discount'))}</span><b>−${esc(money(tt.discount))}</b></div>` : ''}
      ${tt.insurance ? `<div><span>${esc(bi('سهم بیمه', 'Insurance cover'))}</span><b>−${esc(money(tt.insurance))}</b></div>` : ''}
      <div class="strong"><span>${esc(bi('قابل پرداخت', 'Amount due'))}</span><b>${esc(money(tt.due))}</b></div>
      <div><span>${esc(bi('پرداخت‌شده', 'Paid'))}</span><b>${esc(money(tt.paid))}</b></div>
      <div><span>${esc(bi('مانده', 'Outstanding'))}</span><b>${esc(money(tt.outstanding))}</b></div>
    </div>
    <div class="pr-foot">${esc(bi('این صورتحساب توسط ClinicOS صادر شده است.', 'Issued by ClinicOS'))}</div>
  </div>`);
});

/* ----------------------------------------------------------------- reports */

export async function renderReports() {
  const data = await get(`/api/reports/summary${query({ from: repState.from, to: repState.to })}`);
  const tt = data.totals || {};
  const cur = t('common.currency');
  const csv = (kind) => `/api/reports/export${query({ type: kind, from: data.from, to: data.to })}`;
  const moneyCell = (v) => (v === null || v === undefined ? '—' : `${money(v)} ${cur}`);
  return `<div class="page-head"><div class="grow"><h2>${esc(t('nav.reports'))}</h2>
      <div class="sub">${esc(fmtDate(data.from))} — ${esc(fmtDate(data.to))}</div></div>
      <div class="head-actions">
        <a class="btn" href="${csv('invoices')}" download>${ic('download')}${esc(bi('صورتحساب‌ها (CSV)', 'Invoices (CSV)'))}</a>
        <a class="btn" href="${csv('payments')}" download>${ic('download')}${esc(bi('پرداخت‌ها (CSV)', 'Payments (CSV)'))}</a>
        <a class="btn" href="${csv('appointments')}" download>${ic('download')}${esc(bi('نوبت‌ها (CSV)', 'Appointments (CSV)'))}</a>
        <a class="btn" href="${csv('patients')}" download>${ic('download')}${esc(bi('بیماران (CSV)', 'Patients (CSV)'))}</a>
      </div></div>
    <div class="card"><form data-form="rep.range" class="inline-fields">
      <div class="field"><label>${esc(bi('از تاریخ', 'From'))}</label><input type="date" name="from" value="${esc(data.from)}"></div>
      <div class="field"><label>${esc(bi('تا تاریخ', 'To'))}</label><input type="date" name="to" value="${esc(data.to)}"></div>
      <button class="btn pri" type="submit">${ic('chart')}${esc(bi('به‌روزرسانی', 'Update'))}</button>
      <div class="right flex">
        <button class="btn sm" type="button" data-act="rep.preset" data-days="6">${esc(bi('۷ روز', '7 days'))}</button>
        <button class="btn sm" type="button" data-act="rep.preset" data-days="29">${esc(bi('۳۰ روز', '30 days'))}</button>
        <button class="btn sm" type="button" data-act="rep.preset" data-days="89">${esc(bi('۹۰ روز', '90 days'))}</button>
      </div>
    </form></div>
    <div class="kpi-grid mt14">
      <div class="stat"><div class="lbl">${esc(bi('ویزیت‌ها', 'Visits'))}</div><div class="val">${faNum(tt.visits || 0)}</div><div class="mini-note">${faNum(tt.uniquePatients || 0)} ${esc(bi('بیمار یکتا', 'unique patients'))}</div></div>
      <div class="stat"><div class="lbl">${esc(bi('بیماران جدید', 'New patients'))}</div><div class="val">${faNum(tt.newPatients || 0)}</div></div>
      <div class="stat"><div class="lbl">${esc(bi('نوبت‌ها', 'Appointments'))}</div><div class="val">${faNum(tt.appointments || 0)}</div><div class="mini-note">${faNum(tt.noShows || 0)} ${esc(bi('عدم حضور', 'no-show'))} (${faNum(tt.noShowRate || 0)}%)</div></div>
      <div class="stat ok"><div class="lbl">${esc(bi('درآمد', 'Revenue'))}</div><div class="val">${moneyCell(tt.revenue)}</div></div>
      <div class="stat"><div class="lbl">${esc(bi('صورتحساب‌شده', 'Billed'))}</div><div class="val">${moneyCell(tt.billed)}</div><div class="mini-note">${esc(bi('میانگین:', 'Average:'))} ${moneyCell(tt.averageInvoice)}</div></div>
      <div class="stat warn"><div class="lbl">${esc(bi('مطالبات', 'Outstanding'))}</div><div class="val">${moneyCell(tt.outstanding)}</div></div>
    </div>
    <div class="grid2 mt14">
      <div class="card"><h3>${ic('stetho')}${esc(bi('عملکرد پزشکان', 'By physician'))}</h3>
        <div class="tbl-wrap"><table class="tbl"><thead><tr><th>${esc(bi('پزشک', 'Physician'))}</th><th>${esc(bi('ویزیت', 'Visits'))}</th><th>${esc(bi('درآمد', 'Revenue'))}</th></tr></thead>
          <tbody>${(data.byDoctor || []).map((d) => `<tr><td>${esc(bi(d.name.fa, d.name.en))}</td><td class="num">${faNum(d.visits)}</td><td class="num">${moneyCell(d.revenue)}</td></tr>`).join('') || `<tr><td colspan="3">${emptyState('stetho', bi('داده‌ای نیست', 'No data'))}</td></tr>`}</tbody></table></div>
      </div>
      <div class="card"><h3>${ic('folder')}${esc(bi('درآمد بر اساس دسته خدمت', 'By service category'))}</h3>
        <div class="tbl-wrap"><table class="tbl"><thead><tr><th>${esc(bi('دسته', 'Category'))}</th><th>${esc(bi('تعداد', 'Count'))}</th><th>${esc(bi('مبلغ', 'Amount'))}</th></tr></thead>
          <tbody>${(data.byServiceCategory || []).map((s) => `<tr><td>${esc(s.category)}</td><td class="num">${faNum(s.count)}</td><td class="num">${moneyCell(s.amount)}</td></tr>`).join('') || `<tr><td colspan="3">${emptyState('folder', bi('داده‌ای نیست', 'No data'))}</td></tr>`}</tbody></table></div>
      </div>
      <div class="card"><h3>${ic('money')}${esc(bi('روش‌های پرداخت', 'Payment methods'))}</h3>
        <div class="tbl-wrap"><table class="tbl"><thead><tr><th>${esc(bi('روش', 'Method'))}</th><th>${esc(bi('تعداد', 'Count'))}</th><th>${esc(bi('مبلغ', 'Amount'))}</th></tr></thead>
          <tbody>${(data.byPaymentMethod || []).map((m) => `<tr><td>${esc(methodLabel(m.method))}</td><td class="num">${faNum(m.count)}</td><td class="num">${moneyCell(m.amount)}</td></tr>`).join('') || `<tr><td colspan="3">${emptyState('money', bi('داده‌ای نیست', 'No data'))}</td></tr>`}</tbody></table></div>
      </div>
      <div class="card"><h3>${ic('clipboard')}${esc(bi('شایع‌ترین تشخیص‌ها', 'Top diagnoses'))}</h3>
        <div class="tbl-wrap"><table class="tbl"><thead><tr><th>${esc(bi('کد', 'Code'))}</th><th>${esc(bi('عنوان', 'Title'))}</th><th>${esc(bi('تعداد', 'Count'))}</th></tr></thead>
          <tbody>${(data.topDiagnoses || []).map((d) => `<tr><td class="mono">${esc(d.code)}</td><td class="small">${esc(d.title ? bi(d.title.fa, d.title.en) : '')}</td><td class="num">${faNum(d.count)}</td></tr>`).join('') || `<tr><td colspan="3">${emptyState('clipboard', bi('داده‌ای نیست', 'No data'))}</td></tr>`}</tbody></table></div>
      </div>
    </div>
    <div class="card mt14"><h3>${ic('chart')}${esc(bi('روند ویزیت روزانه', 'Daily visits'))}</h3>
      <div class="bars">${(data.visitsByDay || []).map((d) => {
        const max = Math.max(1, ...data.visitsByDay.map((x) => x.visits));
        return `<div class="bar-col" title="${esc(fmtDate(d.date))}: ${faNum(d.visits)}">
          <div class="bar" style="height:${Math.max(4, Math.round((d.visits / max) * 120))}px"></div>
          <div class="mini-note">${esc(fmtDate(d.date).slice(0, 6))}</div></div>`;
      }).join('') || emptyState('chart', bi('داده‌ای نیست', 'No data'))}</div>
    </div>`;
}

onForm('rep.range', (form) => {
  const v = formValues(form);
  repState.from = v.from || repState.from;
  repState.to = v.to || repState.to;
  refresh();
});
onAction('rep.preset', (el) => {
  repState.to = todayISO();
  repState.from = addDays(repState.to, -Number(el.dataset.days || 29));
  refresh();
});
