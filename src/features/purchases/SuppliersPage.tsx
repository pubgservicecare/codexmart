import { useState } from 'react';
import { keepPreviousData, useQuery, useQueryClient } from '@tanstack/react-query';
import { Banknote, Building, Pencil, Plus } from 'lucide-react';
import { api, type Supplier, type SupplierInput } from '@/lib/api';
import { csvMoney, exportCsv } from '@/lib/csv';
import { date, money } from '@/lib/format';
import { useDebounced } from '@/lib/utils';
import { Badge, Button, EmptyState, ErrorBox, Field, Input, Modal, MoneyInput, PageHeader, SearchBox, Select, Spinner, Switch, Textarea, useToast } from '@/components/ui';
import { SupplierPaymentDialog } from './SupplierPaymentDialog';

export function SuppliersPage() {
  const [search, setSearch] = useState('');
  const [status, setStatus] = useState('active');
  const [editing, setEditing] = useState<Supplier | 'new' | null>(null);
  const [viewing, setViewing] = useState<string | null>(null);
  const [paying, setPaying] = useState<string | null>(null);
  const q = useDebounced(search, 200);
  const { data, isLoading, error, refetch } = useQuery({
    queryKey: ['suppliers', 'list', q, status],
    queryFn: () => api.suppliers.list({ search: q, status, limit: 500 }),
    placeholderData: keepPreviousData,
  });
  const owed = data?.items.reduce((s, x) => s + Math.max(x.balance, 0), 0) ?? 0;

  return (
    <div>
      <PageHeader
        title="Suppliers"
        description="Who you buy from and how much you owe them."
        icon={<Building className="h-5 w-5" />}
        actions={
          <>
            <Button icon={<Banknote className="h-4 w-4" />} onClick={() => setPaying('')}>
              Pay supplier
            </Button>
            <Button variant="primary" icon={<Plus className="h-4 w-4" />} onClick={() => setEditing('new')}>
              New supplier
            </Button>
          </>
        }
      />
      <div className="card">
        <div className="flex flex-wrap items-center gap-3 border-b border-slate-100 p-4">
          <SearchBox value={search} onChange={setSearch} placeholder="Name, phone or contact" className="w-80" />
          <Select className="w-32" value={status} onChange={(e) => setStatus(e.target.value)}>
            <option value="active">Active</option>
            <option value="inactive">Inactive</option>
            <option value="all">All</option>
          </Select>
          <span className="ml-auto text-sm text-slate-500">
            Total owed: <b className="num text-slate-900">{money(owed)}</b>
          </span>
        </div>
        {error ? (
          <ErrorBox error={error} onRetry={refetch} />
        ) : isLoading ? (
          <Spinner />
        ) : !data?.items.length ? (
          <EmptyState icon={<Building className="h-6 w-6" />} title="No suppliers yet" description="Add the companies and wholesalers you buy stock from." />
        ) : (
          <table className="table">
            <thead>
              <tr>
                <th>Supplier</th>
                <th>Phone</th>
                <th className="text-right">Purchases</th>
                <th className="text-right">Paid</th>
                <th className="text-right">Returns</th>
                <th className="text-right">Balance owed</th>
                <th>Last purchase</th>
                <th />
              </tr>
            </thead>
            <tbody>
              {data.items.map((s) => (
                <tr key={s.id} className="clickable" onClick={() => setViewing(s.id)}>
                  <td>
                    <p className="font-medium text-slate-900">
                      {s.name} {!s.isActive && <Badge>Inactive</Badge>}
                    </p>
                    {s.contactPerson && <p className="text-2xs text-slate-500">{s.contactPerson}</p>}
                  </td>
                  <td className="text-slate-600">{s.phone ?? '—'}</td>
                  <td className="num text-right">{money(s.totalPurchases)}</td>
                  <td className="num text-right">{money(s.totalPayments)}</td>
                  <td className="num text-right">{money(s.totalReturns)}</td>
                  <td className={`num text-right font-semibold ${s.balance > 0 ? 'text-amber-700' : 'text-slate-700'}`}>{money(s.balance)}</td>
                  <td className="text-slate-600">{date(s.lastPurchaseDate)}</td>
                  <td className="text-right" onClick={(e) => e.stopPropagation()}>
                    <Button size="sm" variant="ghost" iconOnly title="Edit" onClick={() => setEditing(s)}>
                      <Pencil className="h-4 w-4" />
                    </Button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>
      {editing && <SupplierForm supplier={editing === 'new' ? null : editing} onClose={() => setEditing(null)} />}
      {viewing && <SupplierLedger id={viewing} onClose={() => setViewing(null)} onPay={() => setPaying(viewing)} />}
      {paying !== null && <SupplierPaymentDialog supplierId={paying || undefined} onClose={() => setPaying(null)} />}
    </div>
  );
}

function SupplierForm({ supplier, onClose }: { supplier: Supplier | null; onClose: () => void }) {
  const toast = useToast();
  const qc = useQueryClient();
  const [form, setForm] = useState<SupplierInput>({
    name: supplier?.name ?? '',
    contactPerson: supplier?.contactPerson ?? '',
    phone: supplier?.phone ?? '',
    email: supplier?.email ?? '',
    address: supplier?.address ?? '',
    openingBalance: supplier?.openingBalance ?? 0,
    notes: supplier?.notes ?? '',
    isActive: supplier?.isActive ?? true,
  });
  const [busy, setBusy] = useState(false);
  const set = (p: Partial<SupplierInput>) => setForm((f) => ({ ...f, ...p }));
  const save = async () => {
    setBusy(true);
    try {
      if (supplier) await api.suppliers.update(supplier.id, form);
      else await api.suppliers.create(form);
      void qc.invalidateQueries({ queryKey: ['suppliers'] });
      toast.success(supplier ? 'Supplier updated' : 'Supplier added', form.name);
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
      title={supplier ? `Edit ${supplier.name}` : 'New supplier'}
      icon={<Building className="h-5 w-5" />}
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
        <Field label="Supplier / company name" required className="col-span-2">
          <Input value={form.name} onChange={(e) => set({ name: e.target.value })} />
        </Field>
        <Field label="Contact person">
          <Input value={form.contactPerson ?? ''} onChange={(e) => set({ contactPerson: e.target.value })} />
        </Field>
        <Field label="Phone">
          <Input value={form.phone ?? ''} onChange={(e) => set({ phone: e.target.value })} />
        </Field>
        <Field label="Email">
          <Input value={form.email ?? ''} onChange={(e) => set({ email: e.target.value })} />
        </Field>
        <Field label="Opening balance (owed)" hint="What you already owed before using CODE X MART.">
          <MoneyInput value={form.openingBalance} onChange={(v) => set({ openingBalance: v ?? 0 })} />
        </Field>
        <Field label="Address" className="col-span-2">
          <Textarea rows={2} value={form.address ?? ''} onChange={(e) => set({ address: e.target.value })} />
        </Field>
        <Field label="Notes" className="col-span-2">
          <Textarea rows={2} value={form.notes ?? ''} onChange={(e) => set({ notes: e.target.value })} />
        </Field>
        {supplier && (
          <div className="col-span-2">
            <Switch checked={form.isActive ?? true} onChange={(v) => set({ isActive: v })} label="Active" />
          </div>
        )}
      </div>
    </Modal>
  );
}

function SupplierLedger({ id, onClose, onPay }: { id: string; onClose: () => void; onPay: () => void }) {
  const { data: s } = useQuery({ queryKey: ['suppliers', 'detail', id], queryFn: () => api.suppliers.get(id) });
  const { data: ledger, isLoading } = useQuery({ queryKey: ['suppliers', 'ledger', id], queryFn: () => api.suppliers.ledger(id) });
  const exportLedger = () =>
    ledger &&
    exportCsv(`supplier_${s?.name ?? id}.csv`, ledger, [
      { header: 'Date', value: (e) => e.date },
      { header: 'Reference', value: (e) => e.reference },
      { header: 'Description', value: (e) => e.description },
      { header: 'Purchased', value: (e) => csvMoney(e.credit) },
      { header: 'Paid / returned', value: (e) => csvMoney(e.debit) },
      { header: 'Balance', value: (e) => csvMoney(e.balance) },
    ]);
  return (
    <Modal
      open
      onClose={onClose}
      size="lg"
      title={s?.name ?? 'Supplier'}
      description={s ? `Balance owed: ${money(s.balance)}` : undefined}
      icon={<Building className="h-5 w-5" />}
      footer={
        <>
          <Button onClick={exportLedger}>Export statement</Button>
          <Button variant="primary" icon={<Banknote className="h-4 w-4" />} onClick={onPay}>
            Pay supplier
          </Button>
        </>
      }
    >
      <div className="max-h-[60vh] overflow-y-auto rounded-xl border border-slate-200">
        {isLoading ? (
          <Spinner />
        ) : (
          <table className="table">
            <thead>
              <tr>
                <th>Date</th>
                <th>Details</th>
                <th className="text-right">Purchased</th>
                <th className="text-right">Paid / returned</th>
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
                  <td className="num text-right">{e.credit ? money(e.credit) : ''}</td>
                  <td className="num text-right text-emerald-700">{e.debit ? money(e.debit) : ''}</td>
                  <td className="num text-right font-medium">{money(e.balance)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>
    </Modal>
  );
}
