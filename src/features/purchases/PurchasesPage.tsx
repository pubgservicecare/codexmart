import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { keepPreviousData, useQuery } from '@tanstack/react-query';
import { Banknote, ClipboardList, PackagePlus, Plus } from 'lucide-react';
import { api, type DateRange, type PurchaseOrder } from '@/lib/api';
import { presetRange } from '@/lib/dates';
import { date, methodLabel, money } from '@/lib/format';
import { useDebounced } from '@/lib/utils';
import { Badge, Button, DateRangePicker, EmptyState, ErrorBox, PageHeader, Pager, SearchBox, Select, Spinner, Tabs } from '@/components/ui';
import { PurchaseDetailDialog } from './PurchaseDetailDialog';
import { PO_STATUS, PurchaseOrderDialog } from './PurchaseOrderDialog';
import { SupplierPaymentDialog } from './SupplierPaymentDialog';

type Tab = 'invoices' | 'orders' | 'payments' | 'returns';

export function PurchasesPage() {
  const navigate = useNavigate();
  const [tab, setTab] = useState<Tab>('invoices');
  const [range, setRange] = useState<DateRange>(presetRange('thisMonth'));
  const [poOpen, setPoOpen] = useState<PurchaseOrder | 'new' | null>(null);
  const [paying, setPaying] = useState(false);

  return (
    <div>
      <PageHeader
        title="Purchases"
        description="Goods received from suppliers, purchase orders, payments and returns."
        icon={<ClipboardList className="h-5 w-5" />}
        actions={
          <>
            <Button icon={<Banknote className="h-4 w-4" />} onClick={() => setPaying(true)}>
              Pay supplier
            </Button>
            <Button icon={<Plus className="h-4 w-4" />} onClick={() => setPoOpen('new')}>
              New purchase order
            </Button>
            <Button variant="primary" icon={<PackagePlus className="h-4 w-4" />} onClick={() => navigate('/purchases/receive')}>
              Receive goods
            </Button>
          </>
        }
      />
      <div className="mb-5 flex flex-wrap items-end justify-between gap-4">
        <Tabs
          value={tab}
          onChange={setTab}
          tabs={[
            { key: 'invoices', label: 'Purchase invoices' },
            { key: 'orders', label: 'Purchase orders' },
            { key: 'payments', label: 'Supplier payments' },
            { key: 'returns', label: 'Returns to supplier' },
          ]}
        />
        <DateRangePicker value={range} onChange={setRange} />
      </div>
      {tab === 'invoices' && <Invoices range={range} />}
      {tab === 'orders' && <Orders range={range} onOpen={setPoOpen} />}
      {tab === 'payments' && <Payments range={range} />}
      {tab === 'returns' && <Returns range={range} />}
      {poOpen && <PurchaseOrderDialog po={poOpen === 'new' ? null : poOpen} onClose={() => setPoOpen(null)} />}
      {paying && <SupplierPaymentDialog onClose={() => setPaying(false)} />}
    </div>
  );
}

function Invoices({ range }: { range: DateRange }) {
  const [search, setSearch] = useState('');
  const [offset, setOffset] = useState(0);
  const [open, setOpen] = useState<string | null>(null);
  const q = useDebounced(search, 200);
  const { data, isLoading, error, refetch } = useQuery({
    queryKey: ['purchases', 'list', range, q, offset],
    queryFn: () => api.purchases.list({ search: q, dateFrom: range.from, dateTo: range.to, limit: 50, offset }),
    placeholderData: keepPreviousData,
  });
  return (
    <div className="card">
      <div className="border-b border-slate-100 p-4">
        <SearchBox value={search} onChange={setSearch} placeholder="GRN no, supplier invoice or supplier" className="w-80" />
      </div>
      {error ? (
        <ErrorBox error={error} onRetry={refetch} />
      ) : isLoading ? (
        <Spinner />
      ) : !data?.items.length ? (
        <EmptyState icon={<PackagePlus className="h-6 w-6" />} title="No purchases in this period" description="Use “Receive goods” when stock arrives from a supplier." />
      ) : (
        <>
          <table className="table">
            <thead>
              <tr>
                <th>No.</th>
                <th>Date</th>
                <th>Supplier</th>
                <th>Supplier invoice</th>
                <th className="text-right">Items</th>
                <th className="text-right">Total</th>
                <th className="text-right">Paid</th>
                <th>PO</th>
              </tr>
            </thead>
            <tbody>
              {data.items.map((p) => (
                <tr key={p.id} className="clickable" onClick={() => setOpen(p.id)}>
                  <td className="font-medium text-slate-900">{p.purchaseNo}</td>
                  <td className="text-slate-600">{date(p.businessDate)}</td>
                  <td>{p.supplierName}</td>
                  <td className="text-slate-600">{p.supplierInvoiceNo ?? '—'}</td>
                  <td className="num text-right">{p.itemCount}</td>
                  <td className="num text-right font-semibold">{money(p.total)}</td>
                  <td className="num text-right">{money(p.paidAmount)}</td>
                  <td className="text-slate-600">{p.poNo ?? '—'}</td>
                </tr>
              ))}
            </tbody>
          </table>
          <Pager total={data.total} limit={50} offset={offset} onChange={setOffset} />
        </>
      )}
      {open && <PurchaseDetailDialog id={open} onClose={() => setOpen(null)} />}
    </div>
  );
}

function Orders({ range, onOpen }: { range: DateRange; onOpen: (po: PurchaseOrder) => void }) {
  const [status, setStatus] = useState('');
  const { data, isLoading, error, refetch } = useQuery({
    queryKey: ['po', 'list', range, status],
    queryFn: () => api.purchases.orders({ status: status || undefined, dateFrom: range.from, dateTo: range.to, limit: 200 }),
  });
  const openPo = async (id: string) => onOpen(await api.purchases.getOrder(id));
  return (
    <div className="card">
      <div className="border-b border-slate-100 p-4">
        <Select className="w-48" value={status} onChange={(e) => setStatus(e.target.value)}>
          <option value="">All statuses</option>
          {Object.entries(PO_STATUS).map(([k, v]) => (
            <option key={k} value={k}>
              {v.label}
            </option>
          ))}
        </Select>
      </div>
      {error ? (
        <ErrorBox error={error} onRetry={refetch} />
      ) : isLoading ? (
        <Spinner />
      ) : !data?.items.length ? (
        <EmptyState icon={<ClipboardList className="h-6 w-6" />} title="No purchase orders" description="Purchase orders are optional; you can receive goods without one." />
      ) : (
        <table className="table">
          <thead>
            <tr>
              <th>PO</th>
              <th>Order date</th>
              <th>Supplier</th>
              <th>Expected</th>
              <th className="text-right">Lines</th>
              <th className="text-right">Estimate</th>
              <th>Status</th>
            </tr>
          </thead>
          <tbody>
            {data.items.map((po) => (
              <tr key={po.id} className="clickable" onClick={() => void openPo(po.id)}>
                <td className="font-medium text-slate-900">{po.poNo}</td>
                <td className="text-slate-600">{date(po.orderDate)}</td>
                <td>{po.supplierName}</td>
                <td className="text-slate-600">{date(po.expectedDate)}</td>
                <td className="num text-right">{po.itemCount}</td>
                <td className="num text-right">{money(po.totalEstimate)}</td>
                <td>
                  <Badge tone={PO_STATUS[po.status].tone}>{PO_STATUS[po.status].label}</Badge>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </div>
  );
}

function Payments({ range }: { range: DateRange }) {
  const { data, isLoading, error, refetch } = useQuery({
    queryKey: ['purchases', 'payments', range],
    queryFn: () => api.purchases.payments({ dateFrom: range.from, dateTo: range.to, limit: 500 }),
  });
  const total = data?.items.reduce((s, p) => s + p.amount, 0) ?? 0;
  return (
    <div className="card">
      {error ? (
        <ErrorBox error={error} onRetry={refetch} />
      ) : isLoading ? (
        <Spinner />
      ) : !data?.items.length ? (
        <EmptyState icon={<Banknote className="h-6 w-6" />} title="No supplier payments in this period" />
      ) : (
        <table className="table">
          <thead>
            <tr>
              <th>No.</th>
              <th>Date</th>
              <th>Supplier</th>
              <th>Method</th>
              <th>Reference</th>
              <th>Against</th>
              <th className="text-right">Amount</th>
            </tr>
          </thead>
          <tbody>
            {data.items.map((p) => (
              <tr key={p.id}>
                <td className="font-medium text-slate-900">{p.paymentNo}</td>
                <td className="text-slate-600">{date(p.businessDate)}</td>
                <td>{p.supplierName}</td>
                <td>{methodLabel(p.method)}</td>
                <td className="text-slate-600">{p.reference ?? '—'}</td>
                <td className="text-slate-600">{p.purchaseNo ?? 'Account'}</td>
                <td className="num text-right font-semibold">{money(p.amount)}</td>
              </tr>
            ))}
          </tbody>
          <tfoot>
            <tr>
              <td colSpan={6} className="text-right">
                Total paid
              </td>
              <td className="num text-right">{money(total)}</td>
            </tr>
          </tfoot>
        </table>
      )}
    </div>
  );
}

function Returns({ range }: { range: DateRange }) {
  const { data, isLoading, error, refetch } = useQuery({
    queryKey: ['purchases', 'returns', range],
    queryFn: () => api.purchases.returns({ dateFrom: range.from, dateTo: range.to, limit: 500 }),
  });
  return (
    <div className="card">
      {error ? (
        <ErrorBox error={error} onRetry={refetch} />
      ) : isLoading ? (
        <Spinner />
      ) : !data?.items.length ? (
        <EmptyState title="No returns to suppliers in this period" description="Open a purchase invoice and choose “Return to supplier”." />
      ) : (
        <table className="table">
          <thead>
            <tr>
              <th>No.</th>
              <th>Date</th>
              <th>Supplier</th>
              <th>Purchase</th>
              <th>Items</th>
              <th>Reason</th>
              <th className="text-right">Value</th>
            </tr>
          </thead>
          <tbody>
            {data.items.map((r) => (
              <tr key={r.id}>
                <td className="font-medium text-slate-900">{r.returnNo}</td>
                <td className="text-slate-600">{date(r.businessDate)}</td>
                <td>{r.supplierName}</td>
                <td>{r.purchaseNo}</td>
                <td className="text-xs">{r.lines.map((l) => `${l.productName} ×${l.qty / 1000}`).join(', ')}</td>
                <td className="text-slate-600">{r.reason ?? '—'}</td>
                <td className="num text-right font-semibold">{money(r.total)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </div>
  );
}
