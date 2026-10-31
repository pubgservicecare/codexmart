import { useState } from 'react';
import { keepPreviousData, useQuery, useQueryClient } from '@tanstack/react-query';
import { Ban, Download, Plus, Wallet } from 'lucide-react';
import { api, type DateRange, type Expense, type PaymentMethod } from '@/lib/api';
import { csvMoney, exportCsv } from '@/lib/csv';
import { presetRange } from '@/lib/dates';
import { date, methodLabel, money, today } from '@/lib/format';
import { newTxnId, useDebounced } from '@/lib/utils';
import {
  Badge,
  Button,
  ConfirmDialog,
  DateRangePicker,
  EmptyState,
  ErrorBox,
  Field,
  Input,
  Modal,
  MoneyInput,
  PageHeader,
  Pager,
  SearchBox,
  Select,
  Spinner,
  Textarea,
  useToast,
} from '@/components/ui';
import { LookupSelect } from '@/features/products/LookupSelect';

export function ExpensesPage() {
  const toast = useToast();
  const qc = useQueryClient();
  const [range, setRange] = useState<DateRange>(presetRange('thisMonth'));
  const [category, setCategory] = useState<string | null>(null);
  const [search, setSearch] = useState('');
  const [offset, setOffset] = useState(0);
  const [adding, setAdding] = useState(false);
  const [voiding, setVoiding] = useState<Expense | null>(null);
  const [busy, setBusy] = useState(false);
  const q = useDebounced(search, 200);
  const filter = { dateFrom: range.from, dateTo: range.to, categoryId: category ?? undefined, search: q };
  const { data, isLoading, error, refetch } = useQuery({
    queryKey: ['expenses', 'list', filter, offset],
    queryFn: () => api.expenses.list({ ...filter, limit: 50, offset }),
    placeholderData: keepPreviousData,
  });
  const summary = useQuery({ queryKey: ['expenses', 'summary', range], queryFn: () => api.reports.expenses(range) });

  const doVoid = async (reason: string) => {
    if (!voiding) return;
    setBusy(true);
    try {
      await api.expenses.void(voiding.id, reason);
      void qc.invalidateQueries({ queryKey: ['expenses'] });
      void qc.invalidateQueries({ queryKey: ['dashboard'] });
      toast.success('Expense voided', voiding.expenseNo);
      setVoiding(null);
    } catch (e) {
      toast.error(e);
    } finally {
      setBusy(false);
    }
  };

  const exportRows = async () => {
    const all = await api.expenses.list({ ...filter, limit: 1000 });
    const ok = await exportCsv(`expenses_${range.from}_${range.to}.csv`, all.items, [
      { header: 'No.', value: (e) => e.expenseNo },
      { header: 'Date', value: (e) => e.businessDate },
      { header: 'Category', value: (e) => e.categoryName },
      { header: 'Paid to', value: (e) => e.payee },
      { header: 'Method', value: (e) => methodLabel(e.method) },
      { header: 'Amount', value: (e) => csvMoney(e.amount) },
      { header: 'Status', value: (e) => e.status },
      { header: 'Note', value: (e) => e.note },
    ]);
    if (ok) toast.success('Exported');
  };

  return (
    <div>
      <PageHeader
        title="Expenses"
        description="Rent, electricity, salaries and other running costs of the shop."
        icon={<Wallet className="h-5 w-5" />}
        actions={
          <>
            <Button icon={<Download className="h-4 w-4" />} onClick={() => void exportRows().catch(toast.error)}>
              Export CSV
            </Button>
            <Button variant="primary" icon={<Plus className="h-4 w-4" />} onClick={() => setAdding(true)}>
              Record expense
            </Button>
          </>
        }
      />
      <div className="grid grid-cols-4 gap-5">
        <div className="card col-span-3">
          <div className="flex flex-wrap items-center gap-3 border-b border-slate-100 p-4">
            <DateRangePicker value={range} onChange={(r) => { setRange(r); setOffset(0); }} />
            <div className="w-48">
              <LookupSelect kind="expenseCategory" value={category} onChange={(v) => { setCategory(v); setOffset(0); }} emptyLabel="All categories" canAdd={false} />
            </div>
            <SearchBox value={search} onChange={setSearch} placeholder="Payee or note" className="w-56" />
          </div>
          {error ? (
            <ErrorBox error={error} onRetry={refetch} />
          ) : isLoading ? (
            <Spinner />
          ) : !data?.items.length ? (
            <EmptyState icon={<Wallet className="h-6 w-6" />} title="No expenses in this period" />
          ) : (
            <>
              <table className="table">
                <thead>
                  <tr>
                    <th>Date</th>
                    <th>Category</th>
                    <th>Paid to / note</th>
                    <th>Method</th>
                    <th className="text-right">Amount</th>
                    <th />
                  </tr>
                </thead>
                <tbody>
                  {data.items.map((e) => (
                    <tr key={e.id} className={e.status === 'void' ? 'opacity-60' : ''}>
                      <td className="whitespace-nowrap text-slate-600">
                        {date(e.businessDate)}
                        <p className="text-2xs text-slate-400">{e.expenseNo}</p>
                      </td>
                      <td>{e.categoryName}</td>
                      <td>
                        <p className="text-slate-900">{e.payee ?? '—'}</p>
                        {e.note && <p className="text-2xs text-slate-500">{e.note}</p>}
                        {e.status === 'void' && <p className="text-2xs text-rose-600">Void: {e.voidReason}</p>}
                      </td>
                      <td>{methodLabel(e.method)}</td>
                      <td className={`num text-right font-semibold ${e.status === 'void' ? 'line-through' : ''}`}>{money(e.amount)}</td>
                      <td className="text-right">
                        {e.status === 'void' ? (
                          <Badge tone="red">Void</Badge>
                        ) : (
                          <Button size="sm" variant="ghost" iconOnly title="Void" onClick={() => setVoiding(e)}>
                            <Ban className="h-4 w-4" />
                          </Button>
                        )}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
              <Pager total={data.total} limit={50} offset={offset} onChange={setOffset} />
            </>
          )}
        </div>
        <div className="card h-fit p-5">
          <p className="text-xs font-semibold uppercase text-slate-500">Total for period</p>
          <p className="num mt-1 text-2xl font-semibold text-slate-900">{money(summary.data?.total ?? 0)}</p>
          <div className="mt-4 space-y-2.5">
            {summary.data?.byCategory.map((c) => {
              const share = summary.data.total ? (c.amount / summary.data.total) * 100 : 0;
              return (
                <div key={c.name}>
                  <div className="flex justify-between text-sm">
                    <span className="text-slate-700">{c.name}</span>
                    <span className="num font-medium text-slate-900">{money(c.amount)}</span>
                  </div>
                  <div className="mt-1 h-1.5 rounded-full bg-slate-100">
                    <div className="h-1.5 rounded-full bg-brand-500" style={{ width: `${share}%` }} />
                  </div>
                </div>
              );
            })}
          </div>
        </div>
      </div>
      {adding && <ExpenseForm onClose={() => setAdding(false)} />}
      <ConfirmDialog
        open={!!voiding}
        onClose={() => setVoiding(null)}
        title={`Void expense ${voiding?.expenseNo}?`}
        message={`${voiding ? money(voiding.amount) : ''} will be removed from expense totals. The record stays in history marked as void.`}
        confirmLabel="Void expense"
        danger
        requireReason
        busy={busy}
        onConfirm={doVoid}
      />
    </div>
  );
}

function ExpenseForm({ onClose }: { onClose: () => void }) {
  const toast = useToast();
  const qc = useQueryClient();
  const [txnId] = useState(newTxnId);
  const [categoryId, setCategoryId] = useState<string | null>(null);
  const [amount, setAmount] = useState<number | null>(null);
  const [method, setMethod] = useState<PaymentMethod>('cash');
  const [payee, setPayee] = useState('');
  const [note, setNote] = useState('');
  const [day, setDay] = useState(today());
  const [busy, setBusy] = useState(false);

  const save = async () => {
    if (!categoryId || !amount) return;
    setBusy(true);
    try {
      const e = await api.expenses.create({ clientTxnId: txnId, categoryId, amount, method, payee: payee || null, note: note || null, businessDate: day });
      void qc.invalidateQueries({ queryKey: ['expenses'] });
      void qc.invalidateQueries({ queryKey: ['dashboard'] });
      toast.success(`Expense ${e.expenseNo} recorded`, `${money(e.amount)} · ${e.categoryName}`);
      onClose();
    } catch (err) {
      toast.error(err);
    } finally {
      setBusy(false);
    }
  };

  return (
    <Modal
      open
      onClose={onClose}
      title="Record expense"
      icon={<Wallet className="h-5 w-5" />}
      footer={
        <>
          <Button onClick={onClose}>Cancel</Button>
          <Button variant="primary" loading={busy} disabled={!categoryId || !amount || amount <= 0} onClick={save}>
            Save expense
          </Button>
        </>
      }
    >
      <div className="grid grid-cols-2 gap-4">
        <Field label="Category" required>
          <LookupSelect kind="expenseCategory" value={categoryId} onChange={setCategoryId} allowEmpty emptyLabel="Choose…" />
        </Field>
        <Field label="Amount" required>
          <MoneyInput value={amount} onChange={setAmount} />
        </Field>
        <Field label="Date">
          <Input type="date" value={day} max={today()} onChange={(e) => setDay(e.target.value)} />
        </Field>
        <Field label="Paid by">
          <Select value={method} onChange={(e) => setMethod(e.target.value as PaymentMethod)}>
            <option value="cash">Cash (from the drawer)</option>
            <option value="bank">Bank transfer</option>
            <option value="cheque">Cheque</option>
            <option value="wallet">Mobile wallet</option>
            <option value="card">Card</option>
          </Select>
        </Field>
        <Field label="Paid to" className="col-span-2">
          <Input value={payee} onChange={(e) => setPayee(e.target.value)} placeholder="e.g. Landlord, LESCO, staff name" />
        </Field>
        <Field label="Note" className="col-span-2">
          <Textarea rows={2} value={note} onChange={(e) => setNote(e.target.value)} />
        </Field>
      </div>
    </Modal>
  );
}
