import { useEffect, useMemo, useState, type ReactNode } from 'react';
import { Banknote, CreditCard, Landmark, Plus, Smartphone, Trash, Wallet } from 'lucide-react';
import type { CustomerBrief, PaymentMethod, Tender } from '@/lib/api';
import { money } from '@/lib/format';
import { cx } from '@/lib/utils';
import { Button, Field, Input, Modal, MoneyInput } from '@/components/ui';

const METHODS: { key: PaymentMethod; label: string; icon: ReactNode }[] = [
  { key: 'cash', label: 'Cash', icon: <Banknote className="h-4 w-4" /> },
  { key: 'card', label: 'Card', icon: <CreditCard className="h-4 w-4" /> },
  { key: 'bank', label: 'Bank', icon: <Landmark className="h-4 w-4" /> },
  { key: 'wallet', label: 'Wallet', icon: <Smartphone className="h-4 w-4" /> },
];

interface Row {
  method: PaymentMethod;
  amount: number | null;
  reference: string;
}

/** Suggested cash amounts: exact, then the next round notes. */
function quickCash(total: number): number[] {
  const out = new Set<number>([total]);
  for (const step of [10000, 50000, 100000, 500000]) {
    const v = Math.ceil(total / step) * step;
    if (v > total) out.add(v);
  }
  return [...out].sort((a, b) => a - b).slice(0, 5);
}

export function PaymentDialog({
  open,
  onClose,
  total,
  customer,
  creditAllowed,
  busy,
  onConfirm,
}: {
  open: boolean;
  onClose: () => void;
  total: number;
  customer: CustomerBrief | null;
  creditAllowed: boolean;
  busy: boolean;
  onConfirm: (tenders: Tender[]) => void;
}) {
  const [rows, setRows] = useState<Row[]>([{ method: 'cash', amount: null, reference: '' }]);

  useEffect(() => {
    if (open) setRows([{ method: 'cash', amount: null, reference: '' }]);
  }, [open]);

  // Preview only; the backend settles the payment authoritatively.
  const preview = useMemo(() => {
    const amounts = rows.map((r, i) => (r.amount === null && i === 0 && rows.length === 1 ? total : (r.amount ?? 0)));
    const nonCash = rows.reduce((s, r, i) => s + (r.method === 'cash' ? 0 : amounts[i]), 0);
    const cash = rows.reduce((s, r, i) => s + (r.method === 'cash' ? amounts[i] : 0), 0);
    const cashApplied = Math.min(cash, Math.max(total - nonCash, 0));
    const paid = nonCash + cashApplied;
    return { amounts, nonCash, change: cash - cashApplied, remaining: total - paid, overNonCash: nonCash > total };
  }, [rows, total]);

  const canCredit = !!customer && creditAllowed;
  const invalid = preview.overNonCash || (preview.remaining > 0 && !canCredit);

  const submit = () => {
    if (invalid || busy) return;
    onConfirm(
      rows
        .map((r, i) => ({ method: r.method, amount: preview.amounts[i], reference: r.reference.trim() || null }))
        .filter((t) => t.amount > 0),
    );
  };

  const setRow = (i: number, patch: Partial<Row>) => setRows((rs) => rs.map((r, idx) => (idx === i ? { ...r, ...patch } : r)));

  return (
    <Modal
      open={open}
      onClose={onClose}
      closeOnBackdrop={false}
      title="Take payment"
      size="md"
      icon={<Wallet className="h-5 w-5" />}
      footer={
        <>
          <Button onClick={onClose}>Back to bill</Button>
          <Button variant="success" size="lg" loading={busy} disabled={invalid} onClick={submit}>
            Complete sale <span className="kbd ml-1 border-emerald-400 bg-emerald-500 text-white">Enter</span>
          </Button>
        </>
      }
    >
      <form
        onSubmit={(e) => {
          e.preventDefault();
          submit();
        }}
      >
        <div className="mb-5 rounded-xl bg-ink-950 px-5 py-4 text-white">
          <p className="text-xs uppercase tracking-wider text-slate-400">Amount due</p>
          <p className="num mt-1 text-4xl font-semibold tracking-tight">{money(total)}</p>
          {customer && <p className="mt-1 text-sm text-slate-300">Customer: {customer.name}</p>}
        </div>

        <div className="space-y-3">
          {rows.map((row, i) => (
            <div key={i} className="rounded-xl border border-slate-200 p-3">
              <div className="flex items-center gap-2">
                <div className="grid flex-1 grid-cols-4 gap-1 rounded-lg bg-slate-100 p-1">
                  {METHODS.map((m) => (
                    <button
                      key={m.key}
                      type="button"
                      onClick={() => setRow(i, { method: m.key })}
                      className={cx(
                        'flex items-center justify-center gap-1.5 rounded-md py-1.5 text-xs font-medium transition',
                        row.method === m.key ? 'bg-white text-slate-900 shadow-sm' : 'text-slate-500 hover:text-slate-700',
                      )}
                    >
                      {m.icon}
                      {m.label}
                    </button>
                  ))}
                </div>
                {rows.length > 1 && (
                  <Button size="sm" variant="ghost" iconOnly aria-label="Remove payment" onClick={() => setRows((rs) => rs.filter((_, idx) => idx !== i))}>
                    <Trash className="h-4 w-4" />
                  </Button>
                )}
              </div>
              <div className="mt-3 grid grid-cols-5 gap-2">
                <Field label={row.method === 'cash' ? 'Cash received' : 'Amount'} className="col-span-3">
                  <MoneyInput
                    value={row.amount}
                    onChange={(v) => setRow(i, { amount: v })}
                    placeholder={rows.length === 1 ? money(total) : '0'}
                    className="h-11 text-lg"
                    data-autofocus={i === 0 ? true : undefined}
                  />
                </Field>
                {row.method !== 'cash' && (
                  <Field label="Reference" className="col-span-2">
                    <Input className="h-11" value={row.reference} onChange={(e) => setRow(i, { reference: e.target.value })} placeholder="Last 4 / Txn ID" />
                  </Field>
                )}
              </div>
              {row.method === 'cash' && (
                <div className="mt-2 flex flex-wrap gap-1.5">
                  {quickCash(Math.max(total - preview.nonCash, 0)).map((v) => (
                    <button
                      key={v}
                      type="button"
                      className="num rounded-md border border-slate-200 bg-white px-2.5 py-1 text-xs font-medium text-slate-700 hover:border-brand-300 hover:bg-brand-50"
                      onClick={() => setRow(i, { amount: v })}
                    >
                      {money(v)}
                    </button>
                  ))}
                </div>
              )}
            </div>
          ))}
          {rows.length < 4 && (
            <Button size="sm" variant="ghost" icon={<Plus className="h-4 w-4" />} onClick={() => setRows((rs) => [...rs, { method: 'card', amount: null, reference: '' }])}>
              Split payment
            </Button>
          )}
        </div>

        <div className="mt-5 space-y-2 rounded-xl bg-slate-50 p-4">
          {preview.change > 0 && (
            <div className="flex items-center justify-between">
              <span className="text-sm font-medium text-slate-600">Change to return</span>
              <span className="num text-2xl font-semibold text-emerald-700">{money(preview.change)}</span>
            </div>
          )}
          {preview.remaining > 0 && (
            <div className="flex items-center justify-between">
              <span className="text-sm font-medium text-slate-600">{canCredit ? `On credit to ${customer?.name}` : 'Still to pay'}</span>
              <span className={cx('num text-xl font-semibold', canCredit ? 'text-amber-700' : 'text-rose-700')}>{money(preview.remaining)}</span>
            </div>
          )}
          {preview.change === 0 && preview.remaining === 0 && <p className="text-sm font-medium text-emerald-700">Paid in full</p>}
          {preview.overNonCash && <p className="text-sm text-rose-700">Card, bank and wallet payments cannot be more than the bill.</p>}
          {preview.remaining > 0 && !canCredit && (
            <p className="text-sm text-slate-500">
              {customer ? 'Credit sales are turned off in Settings.' : 'Select a customer on the bill to sell on credit (udhaar).'}
            </p>
          )}
        </div>
        <button type="submit" className="hidden" />
      </form>
    </Modal>
  );
}
