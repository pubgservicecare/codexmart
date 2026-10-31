// Display and input conversion for money, quantities and dates.
// The backend stores integers only: paisa for money, thousandths for quantities.

export const QTY_SCALE = 1000;

export interface MoneyFormat {
  symbol: string;
  decimals: number;
}

let moneyFormat: MoneyFormat = { symbol: 'Rs', decimals: 2 };

/** Called when settings load so every screen formats consistently. */
export function setMoneyFormat(f: MoneyFormat) {
  moneyFormat = f;
}

function group(n: number): string {
  return n.toLocaleString('en-US');
}

/** 125050 -> "1,250.50" */
export function amount(minor: number, decimals = moneyFormat.decimals): string {
  const negative = minor < 0;
  const abs = Math.abs(minor);
  let body: string;
  if (decimals === 0) {
    body = group(Math.round(abs / 100));
  } else {
    const whole = Math.floor(abs / 100);
    const frac = abs % 100;
    body = `${group(whole)}.${String(frac).padStart(2, '0')}`;
  }
  return negative ? `-${body}` : body;
}

/** 125050 -> "Rs 1,250.50" */
export function money(minor: number | null | undefined): string {
  if (minor === null || minor === undefined) return '—';
  const negative = minor < 0;
  const s = `${moneyFormat.symbol} ${amount(Math.abs(minor))}`;
  return negative ? `-${s}` : s;
}

/** Compact form for chart axes: 1250000 -> "12.5K" */
export function compactMoney(minor: number): string {
  const v = minor / 100;
  const abs = Math.abs(v);
  if (abs >= 1_000_000) return `${+(v / 1_000_000).toFixed(1)}M`;
  if (abs >= 1_000) return `${+(v / 1_000).toFixed(1)}K`;
  return `${Math.round(v)}`;
}

/** Parses typed money ("1,250.5") into paisa; null when empty or invalid. */
export function parseMoney(text: string): number | null {
  const cleaned = text.replace(/[,\s]/g, '').replace(/^Rs\.?/i, '');
  if (cleaned === '' || cleaned === '.' || cleaned === '-') return null;
  if (!/^-?\d*(\.\d{0,2})?$/.test(cleaned)) return null;
  const negative = cleaned.startsWith('-');
  const [whole, frac = ''] = cleaned.replace('-', '').split('.');
  const value = Number(whole || '0') * 100 + Number((frac + '00').slice(0, 2));
  return negative ? -value : value;
}

/** Paisa to an editable string ("1250.5" style, no grouping). */
export function moneyInput(minor: number | null | undefined): string {
  if (minor === null || minor === undefined) return '';
  const negative = minor < 0;
  const abs = Math.abs(minor);
  const whole = Math.floor(abs / 100);
  const frac = abs % 100;
  const s = frac === 0 ? `${whole}` : `${whole}.${String(frac).padStart(2, '0')}`;
  return negative ? `-${s}` : s;
}

/** 2500 -> "2.5", 3000 -> "3" */
export function qty(milli: number | null | undefined): string {
  if (milli === null || milli === undefined) return '—';
  const negative = milli < 0;
  const abs = Math.abs(milli);
  const whole = Math.floor(abs / QTY_SCALE);
  const frac = abs % QTY_SCALE;
  const body = frac === 0 ? group(whole) : `${group(whole)}.${String(frac).padStart(3, '0').replace(/0+$/, '')}`;
  return negative ? `-${body}` : body;
}

/** Parses a typed quantity into thousandths; null when invalid. */
export function parseQty(text: string, allowDecimal = true): number | null {
  const cleaned = text.replace(/[,\s]/g, '');
  if (cleaned === '') return null;
  const re = allowDecimal ? /^-?\d*(\.\d{0,3})?$/ : /^-?\d+$/;
  if (!re.test(cleaned) || cleaned === '.' || cleaned === '-') return null;
  const negative = cleaned.startsWith('-');
  const [whole, frac = ''] = cleaned.replace('-', '').split('.');
  const value = Number(whole || '0') * QTY_SCALE + Number((frac + '000').slice(0, 3));
  return negative ? -value : value;
}

export function qtyInput(milli: number): string {
  return qty(milli).replace(/,/g, '');
}

/** Basis points to "12.5%". */
export function percent(bp: number): string {
  return `${+(bp / 100).toFixed(1)}%`;
}

const MONTHS = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];

/** "2026-11-01" -> "01 Nov 2026" */
export function date(ymd: string | null | undefined): string {
  if (!ymd) return '—';
  const [y, m, d] = ymd.slice(0, 10).split('-');
  if (!y || !m || !d) return ymd;
  return `${d} ${MONTHS[Number(m) - 1]} ${y}`;
}

/** "2026-11-01" -> "01 Nov" */
export function shortDate(ymd: string): string {
  const [, m, d] = ymd.split('-');
  return `${d} ${MONTHS[Number(m) - 1]}`;
}

/** UTC timestamp -> local "01 Nov 2026, 14:05" */
export function dateTime(ts: string | null | undefined): string {
  if (!ts) return '—';
  const d = new Date(ts);
  if (Number.isNaN(d.getTime())) return ts;
  const dd = String(d.getDate()).padStart(2, '0');
  const hh = String(d.getHours()).padStart(2, '0');
  const mm = String(d.getMinutes()).padStart(2, '0');
  return `${dd} ${MONTHS[d.getMonth()]} ${d.getFullYear()}, ${hh}:${mm}`;
}

export function time(ts: string): string {
  const d = new Date(ts);
  return `${String(d.getHours()).padStart(2, '0')}:${String(d.getMinutes()).padStart(2, '0')}`;
}

/** Local calendar date as YYYY-MM-DD. */
export function today(): string {
  return toYmd(new Date());
}

export function toYmd(d: Date): string {
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`;
}

export function addDays(ymd: string, days: number): string {
  const [y, m, d] = ymd.split('-').map(Number);
  const dt = new Date(y, m - 1, d);
  dt.setDate(dt.getDate() + days);
  return toYmd(dt);
}

export function bytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  return `${(n / 1024 / 1024).toFixed(1)} MB`;
}

export function relativeTime(ts: string | null | undefined): string {
  if (!ts) return 'never';
  const diff = Date.now() - new Date(ts).getTime();
  const mins = Math.round(diff / 60000);
  if (mins < 1) return 'just now';
  if (mins < 60) return `${mins} min ago`;
  const hours = Math.round(mins / 60);
  if (hours < 48) return `${hours} h ago`;
  return `${Math.round(hours / 24)} days ago`;
}

export const methodLabels: Record<string, string> = {
  cash: 'Cash',
  card: 'Card',
  bank: 'Bank transfer',
  wallet: 'Mobile wallet',
  cheque: 'Cheque',
  account: 'Customer account',
  credit: 'Credit (udhaar)',
};

export function methodLabel(m: string): string {
  return methodLabels[m] ?? m;
}
