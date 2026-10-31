import { useEffect, useState } from 'react';
import { keepPreviousData, useQuery, useQueryClient } from '@tanstack/react-query';
import { HandCoins, Pencil, Plus, UserPlus, Users } from 'lucide-react';
import { api, type Customer, type CustomerInput, type PaymentMethod } from '@/lib/api';
import { csvMoney, exportCsv } from '@/lib/csv';
import { date, dateTime, money } from '@/lib/format';
import { useSession } from '@/lib/session';
import { newTxnId, useDebounced } from '@/lib/utils';
import {
  Badge,
  Button,
  Checkbox,
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
  Switch,
  Textarea,
  useToast,
} from '@/components/ui';
import { SaleDetailDialog } from '@/features/sales/SaleDetailDialog';

const LIMIT = 50;

export function CustomersPage() {
  const { can } = useSession();
  const [search, setSearch] = useState('');
  const [withBalance, setWithBalance] = useState(false);
  const [offset, setOffset] = useState(0);
  const [editing, setEditing] = useState<Customer | 'new' | null>(null);
  const [viewing, setViewing] = useState<string | null>(null);
  const q = useDebounced(search, 200);
  const { data, isLoading, error, refetch } = useQuery({
    queryKey: ['customers', 'list', q, withBalance, offset],
    queryFn: () => api.customers.list({ search: q, withBalance, limit: LIMIT, offset }),
    placeholderData: keepPreviousData,
  });

  return (
    <div>
      <PageHeader
        title="Customers"
        description="Contact details, purchase history and credit (udhaar) balances."
        icon={<Users className="h-5 w-5" />}
        actions={
          can('manageCustomers') && (
            <Button variant="primary" icon={<UserPlus className="h-4 w-4" />} onClick={() => setEditing('new')}>
              New customer
            </Button>
          )
        }
      />
      <div className="card">
        <div className="flex flex-wrap items-center gap-4 border-b border-slate-100 p-4">
          <SearchBox value={search} onChange={(v) => { setSearch(v); setOffset(0); }} placeholder="Name or phone" className="w-80" />
          <Checkbox checked={withBalance} onChange={(v) => { setWithBalance(v); setOffset(0); }} label="Only with a balance" />
        </div>
        {error ? (
          <ErrorBox error={error} onRetry={refetch} />
        ) : isLoading ? (
          <Spinner />
        ) : !data?.items.length ? (
          <EmptyState icon={<Users className="h-6 w-6" />} title="No customers yet" description="Customers can also be added from the billing screen (F4)." />
        ) : (
          <>
            <table className="table">
              <thead>
                <tr>
                  <th>Name</th>
                  <th>Phone</th>
                  <th className="text-right">Total purchases</th>
                  <th>Last purchase</th>
                  <th className="text-right">Credit limit</th>
                  <th className="text-right">Balance</th>
                  <th />
                </tr>
              </thead>
              <tbody>
                {data.items.map((c) => (
                  <tr key={c.id} className="clickable" onClick={() => setViewing(c.id)}>
                    <td className="font-medium text-slate-900">
                      {c.name} {!c.isActive && <Badge>Inactive</Badge>}
                    </td>
                    <td className="text-slate-600">{c.phone ?? '—'}</td>
                    <td className="num text-right">{money(c.totalPurchases)}</td>
                    <td className="text-slate-600">{date(c.lastPurchaseDate)}</td>
                    <td className="num text-right text-slate-600">{c.creditLimit !== null ? money(c.creditLimit) : '—'}</td>
                    <td className="num text-right">
                      {c.balance > 0 ? (
                        <span className="font-semibold text-amber-700">{money(c.balance)}</span>
                      ) : c.balance < 0 ? (
                        <span className="font-semibold text-emerald-700">Advance {money(-c.balance)}</span>
                      ) : (
                        <span className="text-slate-400">—</span>
                      )}
                    </td>
                    <td className="text-right">
                      {can('manageCustomers') && (
                        <Button size="sm" variant="ghost" iconOnly aria-label="Edit" onClick={(e) => { e.stopPropagation(); setEditing(c); }}>
                          <Pencil className="h-4 w-4" />
                        </Button>
                      )}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
            <Pager total={data.total} limit={LIMIT} offset={offset} onChange={setOffset} />
          </>
        )}
      </div>
      {editing && <CustomerForm customer={editing === 'new' ? null : editing} onClose={() => setEditing(null)} />}
      {viewing && <CustomerAccount id={viewing} onClose={() => setViewing(null)} onEdit={(c) => setEditing(c)} />}
    </div>
  );
}

function CustomerForm({ customer, onClose }: { customer: Customer | null; onClose: () => void }) {
  const toast = useToast();
  const qc = useQueryClient();
  const { can } = useSession();
  const [form, setForm] = useState<CustomerInput>({
    name: customer?.name ?? '',
    phone: customer?.phone ?? '',
    email: customer?.email ?? '',
    address: customer?.address ?? '',
    openingBalance: customer?.openingBalance ?? 0,
    creditLimit: customer?.creditLimit ?? null,
    notes: customer?.notes ?? '',
    isActive: customer?.isActive ?? true,
  });
  const [busy, setBusy] = useState(false);
  const set = (patch: Partial<CustomerInput>) => setForm((f) => ({ ...f, ...patch }));

  const save = async () => {
    setBusy(true);
    try {
      if (customer) await api.customers.update(customer.id, form);
      else await api.customers.create(form);
      void qc.invalidateQueries({ queryKey: ['customers'] });
      toast.success(customer ? 'Customer updated' : 'Customer added', form.name);
      onClose();
    } catch (e) {
      toast.error(e);
    } finally {
      setBusy(false);
    }
  };

  return (
    <Modal
      open
      onClose={onClose}
      title={customer ? `Edit ${customer.name}` : 'New customer'}
      icon={<UserPlus className="h-5 w-5" />}
      footer={
        <>
          <Button onClick={onClose}>Cancel</Button>
          <Button variant="primary" loading={busy} disabled={!form.name.trim()} onClick={save}>
            Save
          </Button>
        </>
      }
    >
      <div className="grid grid-cols-2 gap-4">
        <Field label="Name" required className="col-span-2">
          <Input value={form.name} onChange={(e) => set({ name: e.target.value })} />
        </Field>
        <Field label="Phone">
          <Input value={form.phone ?? ''} onChange={(e) => set({ phone: e.target.value })} />
        </Field>
        <Field label="Email">
          <Input value={form.email ?? ''} onChange={(e) => set({ email: e.target.value })} />
        </Field>
        <Field label="Address" className="col-span-2">
          <Textarea rows={2} value={form.address ?? ''} onChange={(e) => set({ address: e.target.value })} />
        </Field>
        <Field label="Credit limit" hint="Leave empty for no limit.">
          <MoneyInput value={form.creditLimit ?? null} onChange={(v) => set({ creditLimit: v })} />
        </Field>
        <Field label="Opening balance" hint={can('managePurchasing') ? 'Amount already owed before using CODE X MART.' : 'Managers only.'}>
          <MoneyInput value={form.openingBalance} disabled={!can('managePurchasing')} onChange={(v) => set({ openingBalance: v ?? 0 })} />
        </Field>
        <Field label="Notes" className="col-span-2">
          <Textarea rows={2} value={form.notes ?? ''} onChange={(e) => set({ notes: e.target.value })} />
        </Field>
        {customer && (
          <div className="col-span-2">
            <Switch checked={form.isActive ?? true} onChange={(v) => set({ isActive: v })} label="Active" description="Inactive customers cannot be selected for new sales." />
          </div>
        )}
      </div>
    </Modal>
  );
}

function CustomerAccount({ id, onClose, onEdit }: { id: string; onClose: () => void; onEdit: (c: Customer) => void }) {
  const { can } = useSession();
  const [payOpen, setPayOpen] = useState(false);
  const [saleId, setSaleId] = useState<string | null>(null);
  const { data: c } = useQuery({ queryKey: ['customers', 'detail', id], queryFn: () => api.customers.get(id) });
  const { data: ledger, isLoading } = useQuery({ queryKey: ['customers', 'ledger', id], queryFn: () => api.customers.ledger(id) });
  const { data: sales } = useQuery({ queryKey: ['sales', 'customer', id], queryFn: () => api.sales.list({ customerId: id, limit: 20 }) });

  const exportLedger = () =>
    ledger &&
    exportCsv(`statement_${c?.name ?? id}.csv`, ledger, [
      { header: 'Date', value: (e) => e.date },
      { header: 'Reference', value: (e) => e.reference },
      { header: 'Description', value: (e) => e.description },
      { header: 'Charged', value: (e) => csvMoney(e.debit) },
      { header: 'Paid', value: (e) => csvMoney(e.credit) },
      { header: 'Balance', value: (e) => csvMoney(e.balance) },
    ]);

  return (
    <Modal
      open
      onClose={onClose}
      size="xl"
      title={c?.name ?? 'Customer'}
      description={c ? [c.phone, c.email].filter(Boolean).join(' · ') || 'No contact details' : undefined}
      icon={<Users className="h-5 w-5" />}
      footer={
        <>
          {c && can('manageCustomers') && (
            <Button className="mr-auto" icon={<Pencil className="h-4 w-4" />} onClick={() => onEdit(c)}>
              Edit
            </Button>
          )}
          <Button onClick={exportLedger}>Export statement</Button>
          {can('receiveCustomerPayment') && (
            <Button variant="primary" icon={<HandCoins className="h-4 w-4" />} onClick={() => setPayOpen(true)}>
              Receive payment
            </Button>
          )}
        </>
      }
    >
      {c && (
        <div className="mb-5 grid grid-cols-3 gap-3">
          <div className="rounded-xl bg-slate-50 p-4">
            <p className="text-xs text-slate-500">Balance owed</p>
            <p className={`num mt-1 text-2xl font-semibold ${c.balance > 0 ? 'text-amber-700' : 'text-slate-900'}`}>{money(c.balance)}</p>
          </div>
          <div className="rounded-xl bg-slate-50 p-4">
            <p className="text-xs text-slate-500">Total purchases</p>
            <p className="num mt-1 text-2xl font-semibold text-slate-900">{money(c.totalPurchases)}</p>
          </div>
          <div className="rounded-xl bg-slate-50 p-4">
            <p className="text-xs text-slate-500">Credit limit</p>
            <p className="num mt-1 text-2xl font-semibold text-slate-900">{c.creditLimit !== null ? money(c.creditLimit) : 'No limit'}</p>
          </div>
        </div>
      )}
      <div className="grid grid-cols-5 gap-5">
        <div className="col-span-3">
          <p className="mb-2 text-xs font-semibold uppercase text-slate-500">Account statement</p>
          <div className="max-h-96 overflow-y-auto rounded-xl border border-slate-200">
            {isLoading ? (
              <Spinner />
            ) : (
              <table className="table">
                <thead>
                  <tr>
                    <th>Date</th>
                    <th>Details</th>
                    <th className="text-right">Charged</th>
                    <th className="text-right">Paid</th>
                    <th className="text-right">Balance</th>
                  </tr>
                </thead>
                <tbody>
                  {ledger?.map((e, i) => (
                    <tr key={i}>
                      <td className="whitespace-nowrap text-slate-600">{date(e.date)}</td>
                      <td>
                        <p className="text-slate-900">{e.description}</p>
                        <p className="text-2xs text-slate-500">{e.reference}</p>
                      </td>
                      <td className="num text-right">{e.debit ? money(e.debit) : ''}</td>
                      <td className="num text-right text-emerald-700">{e.credit ? money(e.credit) : ''}</td>
                      <td className="num text-right font-medium">{money(e.balance)}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            )}
          </div>
        </div>
        <div className="col-span-2">
          <p className="mb-2 text-xs font-semibold uppercase text-slate-500">Recent purchases</p>
          <div className="max-h-96 overflow-y-auto rounded-xl border border-slate-200">
            {!sales?.items.length ? (
              <EmptyState title="No purchases yet" />
            ) : (
              sales.items.map((s) => (
                <button key={s.id} onClick={() => setSaleId(s.id)} className="flex w-full items-center justify-between border-b border-slate-100 px-3 py-2.5 text-left last:border-0 hover:bg-slate-50">
                  <span>
                    <span className="block text-sm font-medium text-slate-900">{s.invoiceNo}</span>
                    <span className="text-2xs text-slate-500">{dateTime(s.createdAt)}</span>
                  </span>
                  <span className="num text-sm font-semibold">{money(s.total)}</span>
                </button>
              ))
            )}
          </div>
        </div>
      </div>
      {payOpen && c && <ReceivePaymentDialog customer={c} onClose={() => setPayOpen(false)} />}
      <SaleDetailDialog saleId={saleId} onClose={() => setSaleId(null)} />
    </Modal>
  );
}

function ReceivePaymentDialog({ customer, onClose }: { customer: Customer; onClose: () => void }) {
  const toast = useToast();
  const qc = useQueryClient();
  const [txnId] = useState(newTxnId);
  const [amount, setAmount] = useState<number | null>(customer.balance > 0 ? customer.balance : null);
  const [method, setMethod] = useState<PaymentMethod>('cash');
  const [note, setNote] = useState('');
  const [busy, setBusy] = useState(false);
  useEffect(() => setAmount(customer.balance > 0 ? customer.balance : null), [customer.balance]);

  const save = async () => {
    if (!amount) return;
    setBusy(true);
    try {
      const p = await api.customers.receivePayment({ clientTxnId: txnId, customerId: customer.id, amount, method, note: note || undefined });
      void qc.invalidateQueries({ queryKey: ['customers'] });
      void qc.invalidateQueries({ queryKey: ['dashboard'] });
      toast.success(`Payment ${p.receiptNo} recorded`, `${money(p.amount)} from ${customer.name}`);
      onClose();
    } catch (e) {
      toast.error(e);
    } finally {
      setBusy(false);
    }
  };

  return (
    <Modal
      open
      onClose={onClose}
      size="sm"
      title={`Receive payment from ${customer.name}`}
      description={`Current balance: ${money(customer.balance)}`}
      icon={<HandCoins className="h-5 w-5" />}
      footer={
        <>
          <Button onClick={onClose}>Cancel</Button>
          <Button variant="primary" icon={<Plus className="h-4 w-4" />} loading={busy} disabled={!amount || amount <= 0} onClick={save}>
            Record payment
          </Button>
        </>
      }
    >
      <div className="space-y-4">
        <Field label="Amount received" required>
          <MoneyInput value={amount} onChange={setAmount} className="h-11 text-lg" />
        </Field>
        <Field label="Method">
          <Select value={method} onChange={(e) => setMethod(e.target.value as PaymentMethod)}>
            <option value="cash">Cash</option>
            <option value="card">Card</option>
            <option value="bank">Bank transfer</option>
            <option value="wallet">Mobile wallet</option>
            <option value="cheque">Cheque</option>
          </Select>
        </Field>
        <Field label="Note">
          <Input value={note} onChange={(e) => setNote(e.target.value)} />
        </Field>
        {amount !== null && amount > customer.balance && customer.balance >= 0 && (
          <p className="text-xs text-slate-500">The extra {money(amount - customer.balance)} will be kept as an advance for future purchases.</p>
        )}
      </div>
    </Modal>
  );
}
