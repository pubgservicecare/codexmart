import { useState } from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { Calculator, Lock, Printer } from 'lucide-react';
import { api, type CashClosing, type CashDay } from '@/lib/api';
import { addDays, date, dateTime, methodLabel, money, today } from '@/lib/format';
import { usePrint } from '@/lib/print';
import { useSettings } from '@/lib/session';
import { cx } from '@/lib/utils';
import { Button, EmptyState, Field, Input, KeyValue, MoneyInput, PageHeader, Spinner, Textarea, useToast } from '@/components/ui';

function ClosingSlip({ c, storeName }: { c: CashClosing; storeName: string }) {
  const row = (l: string, v: string, bold = false) => (
    <div style={{ display: 'flex', justifyContent: 'space-between', fontWeight: bold ? 700 : 400 }}>
      <span>{l}</span>
      <span>{v}</span>
    </div>
  );
  return (
    <div style={{ width: '72mm', margin: '0 auto', padding: '3mm 0', fontFamily: '"Inter Variable", Arial, sans-serif', fontSize: 12, color: '#000' }}>
      <div style={{ textAlign: 'center', fontWeight: 800, fontSize: 15 }}>{storeName}</div>
      <div style={{ textAlign: 'center', fontWeight: 700 }}>CASH CLOSING {c.closingNo}</div>
      <div style={{ textAlign: 'center' }}>{date(c.businessDate)}</div>
      <div style={{ borderTop: '1px dashed #000', margin: '6px 0' }} />
      {row('Opening float', money(c.openingFloat))}
      {row('+ Cash sales', money(c.cashSales))}
      {row('+ Customer payments', money(c.cashCustomerReceipts))}
      {row('− Cash refunds', money(c.cashRefunds))}
      {row('− Cash expenses', money(c.cashExpenses))}
      {row('− Supplier payments', money(c.cashSupplierPayments))}
      <div style={{ borderTop: '1px dashed #000', margin: '6px 0' }} />
      {row('Expected in drawer', money(c.expectedCash), true)}
      {row('Counted', money(c.countedCash), true)}
      {row(c.difference === 0 ? 'Difference' : c.difference > 0 ? 'Over' : 'Short', money(c.difference), true)}
      {c.note && <div style={{ marginTop: 6 }}>Note: {c.note}</div>}
      <div style={{ marginTop: 6 }}>
        Closed by {c.userName} · {dateTime(c.createdAt)}
      </div>
    </div>
  );
}

export function CashPage() {
  const settings = useSettings();
  const print = usePrint();
  const toast = useToast();
  const qc = useQueryClient();
  const [day, setDay] = useState(today());
  const [openingFloat, setOpeningFloat] = useState<number | null>(null);
  const [counted, setCounted] = useState<number | null>(null);
  const [note, setNote] = useState('');
  const [busy, setBusy] = useState(false);
  const { data, isLoading } = useQuery({ queryKey: ['cash', day], queryFn: () => api.cash.preview(day) });
  const history = useQuery({ queryKey: ['cash', 'history'], queryFn: () => api.cash.closings(addDays(today(), -60), today()) });

  const expected = (openingFloat ?? 0) + (data?.netCash ?? 0);
  const difference = counted === null ? null : counted - expected;

  const close = async () => {
    if (counted === null) return;
    setBusy(true);
    try {
      const c = await api.cash.close({ businessDate: day, openingFloat: openingFloat ?? 0, countedCash: counted, note: note || undefined });
      void qc.invalidateQueries({ queryKey: ['cash'] });
      toast.success(`Day closed (${c.closingNo})`, c.difference === 0 ? 'Cash matches exactly.' : `Difference ${money(c.difference)}`);
      void print(<ClosingSlip c={c} storeName={settings.storeName} />, '80mm');
    } catch (e) {
      toast.error(e);
    } finally {
      setBusy(false);
    }
  };

  return (
    <div>
      <PageHeader
        title="Cash closing"
        description="Count the drawer at the end of the day and compare it with what the system expects."
        icon={<Calculator className="h-5 w-5" />}
        actions={<Input type="date" className="w-44" value={day} max={today()} onChange={(e) => e.target.value && setDay(e.target.value)} />}
      />
      {isLoading || !data ? (
        <Spinner />
      ) : (
        <div className="grid grid-cols-3 gap-5">
          <div className="card col-span-2 p-6">
            <h2 className="text-sm font-semibold text-slate-900">Cash movements on {date(day)}</h2>
            <p className="text-xs text-slate-500">{data.invoices} sale(s)</p>
            <div className="mt-4 grid grid-cols-2 gap-x-10">
              <div>
                <KeyValue label="Cash sales" value={money(data.cashSales)} />
                <KeyValue label="Cash received from customers" value={money(data.cashCustomerReceipts)} />
                <KeyValue label="Cash refunds" value={`−${money(data.cashRefunds)}`} />
                <KeyValue label="Expenses paid in cash" value={`−${money(data.cashExpenses)}`} />
                <KeyValue label="Suppliers paid in cash" value={`−${money(data.cashSupplierPayments)}`} />
                <div className="mt-2 border-t border-slate-200 pt-2">
                  <KeyValue label="Net cash for the day" value={money(data.netCash)} strong />
                </div>
              </div>
              <div>
                <p className="mb-1 text-xs font-semibold uppercase text-slate-500">Sales by payment method</p>
                {data.salesByMethod.length === 0 && <p className="text-sm text-slate-500">No sales</p>}
                {data.salesByMethod.map((m) => (
                  <KeyValue key={m.method} label={methodLabel(m.method)} value={money(m.amount)} />
                ))}
              </div>
            </div>
          </div>
          <ClosePanel
            day={data}
            busy={busy}
            openingFloat={openingFloat}
            setOpeningFloat={setOpeningFloat}
            counted={counted}
            setCounted={setCounted}
            note={note}
            setNote={setNote}
            expected={expected}
            difference={difference}
            onClose={close}
            onReprint={(c) => void print(<ClosingSlip c={c} storeName={settings.storeName} />, '80mm')}
          />
        </div>
      )}
      <div className="card mt-5">
        <div className="card-header">
          <h2 className="card-title">Recent closings</h2>
        </div>
        {!history.data?.length ? (
          <EmptyState title="No closings yet" />
        ) : (
          <table className="table">
            <thead>
              <tr>
                <th>Date</th>
                <th className="text-right">Expected</th>
                <th className="text-right">Counted</th>
                <th className="text-right">Difference</th>
                <th>Closed by</th>
                <th />
              </tr>
            </thead>
            <tbody>
              {history.data.map((c) => (
                <tr key={c.id}>
                  <td>{date(c.businessDate)}</td>
                  <td className="num text-right">{money(c.expectedCash)}</td>
                  <td className="num text-right">{money(c.countedCash)}</td>
                  <td className={cx('num text-right font-semibold', c.difference < 0 ? 'text-rose-700' : c.difference > 0 ? 'text-amber-700' : 'text-emerald-700')}>
                    {money(c.difference)}
                  </td>
                  <td className="text-slate-600">{c.userName}</td>
                  <td className="text-right">
                    <Button size="sm" variant="ghost" icon={<Printer className="h-4 w-4" />} onClick={() => void print(<ClosingSlip c={c} storeName={settings.storeName} />, '80mm')}>
                      Print
                    </Button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>
    </div>
  );
}

function ClosePanel(props: {
  day: CashDay;
  busy: boolean;
  openingFloat: number | null;
  setOpeningFloat: (v: number | null) => void;
  counted: number | null;
  setCounted: (v: number | null) => void;
  note: string;
  setNote: (v: string) => void;
  expected: number;
  difference: number | null;
  onClose: () => void;
  onReprint: (c: CashClosing) => void;
}) {
  const c = props.day.closing;
  if (c) {
    return (
      <div className="card p-6">
        <div className="flex items-center gap-2 text-emerald-700">
          <Lock className="h-4 w-4" />
          <h2 className="text-sm font-semibold">Day closed ({c.closingNo})</h2>
        </div>
        <div className="mt-4">
          <KeyValue label="Opening float" value={money(c.openingFloat)} />
          <KeyValue label="Expected in drawer" value={money(c.expectedCash)} />
          <KeyValue label="Counted" value={money(c.countedCash)} />
          <KeyValue label="Difference" value={money(c.difference)} strong />
        </div>
        <p className="mt-3 text-xs text-slate-500">
          Closed by {c.userName}, {dateTime(c.createdAt)}
        </p>
        <Button className="mt-4 w-full" icon={<Printer className="h-4 w-4" />} onClick={() => props.onReprint(c)}>
          Print closing slip
        </Button>
      </div>
    );
  }
  return (
    <div className="card space-y-4 p-6">
      <h2 className="text-sm font-semibold text-slate-900">Close the day</h2>
      <Field label="Opening float" hint="Cash that was in the drawer when the day started.">
        <MoneyInput value={props.openingFloat} onChange={props.setOpeningFloat} placeholder="0" />
      </Field>
      <div className="rounded-lg bg-slate-50 px-3 py-2">
        <KeyValue label="Expected in drawer" value={money(props.expected)} strong />
      </div>
      <Field label="Counted cash" required>
        <MoneyInput value={props.counted} onChange={props.setCounted} className="h-11 text-lg" />
      </Field>
      {props.difference !== null && (
        <div
          className={cx(
            'rounded-lg px-3 py-2 text-sm font-medium',
            props.difference === 0 ? 'bg-emerald-50 text-emerald-800' : props.difference > 0 ? 'bg-amber-50 text-amber-800' : 'bg-rose-50 text-rose-800',
          )}
        >
          {props.difference === 0 ? 'Cash matches exactly.' : props.difference > 0 ? `Over by ${money(props.difference)}` : `Short by ${money(-props.difference)}`}
        </div>
      )}
      <Field label="Note">
        <Textarea rows={2} value={props.note} onChange={(e) => props.setNote(e.target.value)} />
      </Field>
      <Button variant="primary" className="w-full" icon={<Lock className="h-4 w-4" />} loading={props.busy} disabled={props.counted === null} onClick={props.onClose}>
        Close day & print slip
      </Button>
      <p className="text-xs text-slate-500">A closing is permanent. Sales can still be made after closing; they will show in this day's figures.</p>
    </div>
  );
}
