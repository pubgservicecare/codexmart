import { useState } from 'react';
import { keepPreviousData, useQuery } from '@tanstack/react-query';
import { Boxes, CalendarClock, ClipboardList, Plus, ShieldCheck } from 'lucide-react';
import { api, type DateRange, type Product } from '@/lib/api';
import { presetRange } from '@/lib/dates';
import { date, dateTime, money, qty } from '@/lib/format';
import { useSession, useSettings } from '@/lib/session';
import { cx, useDebounced } from '@/lib/utils';
import {
  Badge,
  Button,
  DateRangePicker,
  EmptyState,
  ErrorBox,
  PageHeader,
  Pager,
  SearchBox,
  Select,
  Spinner,
  StatCard,
  Tabs,
  useToast,
} from '@/components/ui';
import { stockTone } from '@/features/products/ProductsPage';
import { AdjustmentDialog, REASONS } from './AdjustmentDialog';

type Tab = 'stock' | 'adjustments' | 'movements' | 'batches';

const MOVEMENT_LABELS: Record<string, string> = {
  opening: 'Opening stock',
  purchase: 'Purchase',
  purchase_return: 'Return to supplier',
  sale: 'Sale',
  sale_return: 'Customer return',
  sale_void: 'Sale voided',
  adjustment_in: 'Adjustment +',
  adjustment_out: 'Adjustment −',
};

export function InventoryPage() {
  const [tab, setTab] = useState<Tab>('stock');
  const [adjustFor, setAdjustFor] = useState<Product | 'new' | null>(null);
  const [historyFor, setHistoryFor] = useState<Product | null>(null);
  const toast = useToast();
  const [checking, setChecking] = useState(false);

  const verify = async () => {
    setChecking(true);
    try {
      const mismatches = await api.inventory.verify();
      if (mismatches.length === 0) toast.success('Stock records are consistent', 'Every product matches its full movement history.');
      else toast.error('Stock records need attention', `${mismatches.length} product(s) differ from their history: ${mismatches.map((m) => m.productName).join(', ')}`);
    } catch (e) {
      toast.error(e);
    } finally {
      setChecking(false);
    }
  };

  return (
    <div>
      <PageHeader
        title="Inventory"
        description="Stock levels, adjustments, full stock history and expiry dates."
        icon={<Boxes className="h-5 w-5" />}
        actions={
          <>
            <Button icon={<ShieldCheck className="h-4 w-4" />} loading={checking} onClick={verify}>
              Verify stock records
            </Button>
            <Button variant="primary" icon={<Plus className="h-4 w-4" />} onClick={() => setAdjustFor('new')}>
              Stock adjustment
            </Button>
          </>
        }
      />
      <Tabs
        className="mb-5"
        value={tab}
        onChange={setTab}
        tabs={[
          { key: 'stock', label: 'Stock levels' },
          { key: 'adjustments', label: 'Adjustments' },
          { key: 'movements', label: 'Stock history' },
          { key: 'batches', label: 'Batches & expiry' },
        ]}
      />
      {tab === 'stock' && (
        <StockLevels
          onAdjust={setAdjustFor}
          onHistory={(p) => {
            setHistoryFor(p);
            setTab('movements');
          }}
        />
      )}
      {tab === 'adjustments' && <Adjustments />}
      {tab === 'movements' && <Movements product={historyFor} onClearProduct={() => setHistoryFor(null)} />}
      {tab === 'batches' && <Batches />}
      {adjustFor && <AdjustmentDialog initialProduct={adjustFor === 'new' ? undefined : adjustFor} onClose={() => setAdjustFor(null)} />}
    </div>
  );
}

function StockLevels({ onAdjust, onHistory }: { onAdjust: (p: Product) => void; onHistory: (p: Product) => void }) {
  const { can } = useSession();
  const [search, setSearch] = useState('');
  const [stock, setStock] = useState('');
  const [offset, setOffset] = useState(0);
  const q = useDebounced(search, 200);
  const { data, isLoading, error, refetch } = useQuery({
    queryKey: ['products', 'stock', q, stock, offset],
    queryFn: () => api.products.list({ search: q, stock: (stock || undefined) as never, sort: 'stock', limit: 100, offset }),
    placeholderData: keepPreviousData,
  });
  const valuation = useQuery({ queryKey: ['inventory', 'valuation'], queryFn: api.reports.inventoryValuation, enabled: can('viewReports') });

  return (
    <div className="space-y-5">
      {valuation.data && (
        <div className="grid grid-cols-4 gap-4">
          <StatCard label="Stock value at cost" value={money(valuation.data.totalCostValue)} tone="brand" />
          <StatCard label="Stock value at selling price" value={money(valuation.data.totalRetailValue)} tone="green" />
          <StatCard label="Products in stock" value={valuation.data.productsInStock} tone="gray" />
          <StatCard label="Potential gross profit" value={money(valuation.data.totalRetailValue - valuation.data.totalCostValue)} tone="blue" />
        </div>
      )}
      <div className="card">
        <div className="flex flex-wrap gap-3 border-b border-slate-100 p-4">
          <SearchBox value={search} onChange={(v) => { setSearch(v); setOffset(0); }} placeholder="Search products" className="w-80" />
          <Select className="w-44" value={stock} onChange={(e) => { setStock(e.target.value); setOffset(0); }}>
            <option value="">All products</option>
            <option value="attention">Low or out of stock</option>
            <option value="low">Low stock</option>
            <option value="out">Out of stock</option>
          </Select>
        </div>
        {error ? (
          <ErrorBox error={error} onRetry={refetch} />
        ) : isLoading ? (
          <Spinner />
        ) : !data?.items.length ? (
          <EmptyState icon={<Boxes className="h-6 w-6" />} title="No products" />
        ) : (
          <>
            <table className="table">
              <thead>
                <tr>
                  <th>Product</th>
                  <th className="text-right">On hand</th>
                  <th className="text-right">Reorder at</th>
                  {can('viewCosts') && <th className="text-right">Avg. cost</th>}
                  {can('viewCosts') && <th className="text-right">Stock value</th>}
                  <th className="w-56" />
                </tr>
              </thead>
              <tbody>
                {data.items.map((p) => (
                  <tr key={p.id}>
                    <td>
                      <p className="font-medium text-slate-900">{p.displayName}</p>
                      <p className="text-2xs text-slate-500">{p.sku}</p>
                    </td>
                    <td className="text-right">
                      <Badge tone={stockTone(p)}>
                        {qty(p.stockQty)} {p.unitShort}
                      </Badge>
                    </td>
                    <td className="num text-right text-slate-600">{p.reorderLevel ? qty(p.reorderLevel) : '—'}</td>
                    {can('viewCosts') && <td className="num text-right text-slate-600">{money(p.costPrice)}</td>}
                    {can('viewCosts') && <td className="num text-right font-medium">{money(p.stockValue)}</td>}
                    <td className="text-right">
                      <Button size="sm" variant="ghost" onClick={() => onHistory(p)}>
                        History
                      </Button>
                      <Button size="sm" onClick={() => onAdjust(p)}>
                        Adjust
                      </Button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
            <Pager total={data.total} limit={100} offset={offset} onChange={setOffset} />
          </>
        )}
      </div>
    </div>
  );
}

function Adjustments() {
  const [range, setRange] = useState<DateRange>(presetRange('last30'));
  const [offset, setOffset] = useState(0);
  const { data, isLoading, error, refetch } = useQuery({
    queryKey: ['inventory', 'adjustments', range, offset],
    queryFn: () => api.inventory.adjustments({ dateFrom: range.from, dateTo: range.to, limit: 50, offset }),
    placeholderData: keepPreviousData,
  });
  const reasonLabel = (r: string) => REASONS.find((x) => x.value === r)?.label ?? r;
  return (
    <div className="card">
      <div className="border-b border-slate-100 p-4">
        <DateRangePicker value={range} onChange={(r) => { setRange(r); setOffset(0); }} />
      </div>
      {error ? (
        <ErrorBox error={error} onRetry={refetch} />
      ) : isLoading ? (
        <Spinner />
      ) : !data?.items.length ? (
        <EmptyState icon={<ClipboardList className="h-6 w-6" />} title="No adjustments in this period" />
      ) : (
        <>
          <table className="table">
            <thead>
              <tr>
                <th>No.</th>
                <th>Date</th>
                <th>Reason</th>
                <th>Products</th>
                <th className="text-right">Value change</th>
                <th>By</th>
              </tr>
            </thead>
            <tbody>
              {data.items.map((a) => (
                <tr key={a.id}>
                  <td className="font-medium text-slate-900">{a.adjustmentNo}</td>
                  <td className="text-slate-600">{dateTime(a.createdAt)}</td>
                  <td>
                    {reasonLabel(a.reason)}
                    {a.note && <p className="text-2xs text-slate-500">{a.note}</p>}
                  </td>
                  <td className="text-sm">
                    {a.lines.map((l) => (
                      <p key={l.productId}>
                        {l.productName}{' '}
                        <span className={cx('num font-semibold', l.qtyChange > 0 ? 'text-emerald-700' : 'text-rose-700')}>
                          {l.qtyChange > 0 ? '+' : ''}
                          {qty(l.qtyChange)}
                        </span>
                      </p>
                    ))}
                  </td>
                  <td className={cx('num text-right font-medium', (a.valueChange ?? 0) < 0 ? 'text-rose-700' : 'text-emerald-700')}>{money(a.valueChange)}</td>
                  <td className="text-slate-600">{a.userName}</td>
                </tr>
              ))}
            </tbody>
          </table>
          <Pager total={data.total} limit={50} offset={offset} onChange={setOffset} />
        </>
      )}
    </div>
  );
}

function Movements({ product, onClearProduct }: { product: Product | null; onClearProduct: () => void }) {
  const { can } = useSession();
  const [range, setRange] = useState<DateRange>(presetRange('last30'));
  const [type, setType] = useState('');
  const [offset, setOffset] = useState(0);
  const filter = { productId: product?.id, movementType: type || undefined, dateFrom: range.from, dateTo: range.to };
  const { data, isLoading, error, refetch } = useQuery({
    queryKey: ['inventory', 'movements', filter, offset],
    queryFn: () => api.inventory.movements({ ...filter, limit: 100, offset }),
    placeholderData: keepPreviousData,
  });
  return (
    <div className="card">
      <div className="flex flex-wrap items-center gap-3 border-b border-slate-100 p-4">
        {product && (
          <span className="inline-flex items-center gap-2 rounded-lg bg-brand-50 px-3 py-1.5 text-sm text-brand-800">
            {product.displayName}
            <button className="text-brand-500 hover:text-brand-800" onClick={onClearProduct}>
              ✕
            </button>
          </span>
        )}
        <DateRangePicker value={range} onChange={(r) => { setRange(r); setOffset(0); }} />
        <Select className="w-48" value={type} onChange={(e) => { setType(e.target.value); setOffset(0); }}>
          <option value="">All movement types</option>
          {Object.entries(MOVEMENT_LABELS).map(([k, v]) => (
            <option key={k} value={k}>
              {v}
            </option>
          ))}
        </Select>
      </div>
      {error ? (
        <ErrorBox error={error} onRetry={refetch} />
      ) : isLoading ? (
        <Spinner />
      ) : !data?.items.length ? (
        <EmptyState title="No stock movements" description="Every sale, purchase, return and adjustment is listed here." />
      ) : (
        <>
          <table className="table">
            <thead>
              <tr>
                <th>When</th>
                <th>Product</th>
                <th>Type</th>
                <th>Reference</th>
                <th className="text-right">Change</th>
                <th className="text-right">Balance</th>
                {can('viewCosts') && <th className="text-right">Value</th>}
                <th>By</th>
              </tr>
            </thead>
            <tbody>
              {data.items.map((m) => (
                <tr key={m.id}>
                  <td className="whitespace-nowrap text-slate-600">{dateTime(m.createdAt)}</td>
                  <td>
                    <p className="font-medium text-slate-900">{m.productName}</p>
                    {m.batchNo && <p className="text-2xs text-slate-500">Batch {m.batchNo}</p>}
                  </td>
                  <td>{MOVEMENT_LABELS[m.movementType] ?? m.movementType}</td>
                  <td className="text-slate-600">
                    {m.refNo}
                    {m.reason && <p className="text-2xs">{m.reason}</p>}
                  </td>
                  <td className={cx('num text-right font-semibold', m.qtyChange > 0 ? 'text-emerald-700' : 'text-rose-700')}>
                    {m.qtyChange > 0 ? '+' : ''}
                    {qty(m.qtyChange)}
                  </td>
                  <td className="num text-right">{qty(m.qtyAfter)}</td>
                  {can('viewCosts') && <td className="num text-right text-slate-500">{money(m.valueChange)}</td>}
                  <td className="text-slate-600">{m.userName ?? 'System'}</td>
                </tr>
              ))}
            </tbody>
          </table>
          <Pager total={data.total} limit={100} offset={offset} onChange={setOffset} />
        </>
      )}
    </div>
  );
}

function Batches() {
  const settings = useSettings();
  const { can } = useSession();
  const [days, setDays] = useState<number | ''>(settings.expiryWarningDays);
  const { data, isLoading, error, refetch } = useQuery({
    queryKey: ['batches', 'list', days],
    queryFn: () => api.inventory.batches({ onlyAvailable: true, expiringWithinDays: days === '' ? undefined : days }),
  });
  return (
    <div className="card">
      <div className="flex flex-wrap items-center gap-3 border-b border-slate-100 p-4">
        <span className="text-sm text-slate-600">Show batches with stock</span>
        <Select className="w-56" value={String(days)} onChange={(e) => setDays(e.target.value === '' ? '' : Number(e.target.value))}>
          <option value="0">already expired</option>
          <option value="30">expiring within 30 days</option>
          <option value="60">expiring within 60 days</option>
          <option value="90">expiring within 90 days</option>
          <option value="180">expiring within 6 months</option>
          <option value="">all batches</option>
        </Select>
      </div>
      {error ? (
        <ErrorBox error={error} onRetry={refetch} />
      ) : isLoading ? (
        <Spinner />
      ) : !data?.length ? (
        <EmptyState icon={<CalendarClock className="h-6 w-6" />} title="Nothing to show" description="Turn on “Track batches and expiry dates” for a product, then record batches when receiving goods." />
      ) : (
        <table className="table">
          <thead>
            <tr>
              <th>Product</th>
              <th>Batch</th>
              <th>Expiry</th>
              <th className="text-right">Remaining</th>
              <th className="text-right">Received</th>
              {can('viewCosts') && <th className="text-right">Unit cost</th>}
              <th>Received on</th>
            </tr>
          </thead>
          <tbody>
            {data.map((b) => (
              <tr key={b.id}>
                <td className="font-medium text-slate-900">{b.productName}</td>
                <td>{b.batchNo ?? '—'}</td>
                <td>
                  {b.expiryDate ? (
                    <span className="inline-flex items-center gap-2">
                      {date(b.expiryDate)}
                      {b.daysToExpiry !== null &&
                        (b.daysToExpiry < 0 ? (
                          <Badge tone="red">Expired</Badge>
                        ) : b.daysToExpiry <= 30 ? (
                          <Badge tone="amber">{b.daysToExpiry} days</Badge>
                        ) : (
                          <Badge tone="gray">{b.daysToExpiry} days</Badge>
                        ))}
                    </span>
                  ) : (
                    '—'
                  )}
                </td>
                <td className="num text-right font-semibold">
                  {qty(b.qtyRemaining)} {b.unitShort}
                </td>
                <td className="num text-right text-slate-500">{qty(b.qtyReceived)}</td>
                {can('viewCosts') && <td className="num text-right text-slate-500">{money(b.unitCost)}</td>}
                <td className="text-slate-600">{date(b.receivedDate)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </div>
  );
}
