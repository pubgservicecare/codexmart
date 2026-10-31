import { useEffect, useRef, useState, type ReactNode } from 'react';
import { useQuery } from '@tanstack/react-query';
import { ChartColumn, Download, Printer } from 'lucide-react';
import { api, type DateRange } from '@/lib/api';
import { csvMoney, csvQty, exportCsv, type Column } from '@/lib/csv';
import { presetRange } from '@/lib/dates';
import { date, dateTime, methodLabel, money, percent, qty, shortDate } from '@/lib/format';
import { usePrint } from '@/lib/print';
import { useSettings } from '@/lib/session';
import { cx } from '@/lib/utils';
import { Button, DateRangePicker, EmptyState, ErrorBox, KeyValue, PageHeader, Select, Spinner, useToast } from '@/components/ui';

type ReportKey =
  | 'sales'
  | 'pl'
  | 'products'
  | 'categories'
  | 'payments'
  | 'returns'
  | 'purchases'
  | 'expenses'
  | 'customers'
  | 'suppliers'
  | 'valuation'
  | 'lowstock'
  | 'expiry';

const MENU: { group: string; items: { key: ReportKey; label: string; ranged: boolean }[] }[] = [
  {
    group: 'Sales & profit',
    items: [
      { key: 'sales', label: 'Sales summary', ranged: true },
      { key: 'pl', label: 'Profit & loss', ranged: true },
      { key: 'products', label: 'Sales by product', ranged: true },
      { key: 'categories', label: 'Sales by category', ranged: true },
      { key: 'payments', label: 'Payment methods', ranged: true },
      { key: 'returns', label: 'Returns & refunds', ranged: true },
    ],
  },
  {
    group: 'Buying & costs',
    items: [
      { key: 'purchases', label: 'Purchases', ranged: true },
      { key: 'expenses', label: 'Expenses', ranged: true },
    ],
  },
  {
    group: 'Balances',
    items: [
      { key: 'customers', label: 'Customer balances', ranged: false },
      { key: 'suppliers', label: 'Supplier balances', ranged: false },
    ],
  },
  {
    group: 'Stock',
    items: [
      { key: 'valuation', label: 'Inventory valuation', ranged: false },
      { key: 'lowstock', label: 'Low stock', ranged: false },
      { key: 'expiry', label: 'Expiry', ranged: false },
    ],
  },
];

/** Exposes CSV export of the visible report to the page header. */
type Exporter = { name: string; run: () => Promise<boolean> } | null;
const noop = () => undefined;

export function ReportsPage() {
  const toast = useToast();
  const print = usePrint();
  const settings = useSettings();
  const [key, setKey] = useState<ReportKey>('sales');
  const [range, setRange] = useState<DateRange>(presetRange('thisMonth'));
  const [exporter, setExporter] = useState<Exporter>(null);
  const item = MENU.flatMap((g) => g.items).find((i) => i.key === key)!;

  const body = <Report k={key} range={range} setExporter={setExporter} />;

  const doPrint = () =>
    print(
      <div style={{ fontFamily: '"Inter Variable", Arial, sans-serif', color: '#000' }}>
        <div style={{ display: 'flex', justifyContent: 'space-between', borderBottom: '1.5px solid #000', paddingBottom: 8, marginBottom: 12 }}>
          <div>
            <div style={{ fontSize: 18, fontWeight: 800 }}>{settings.storeName}</div>
            <div style={{ fontSize: 14, fontWeight: 600 }}>{item.label}</div>
          </div>
          <div style={{ textAlign: 'right', fontSize: 11 }}>
            {item.ranged ? `${date(range.from)} – ${date(range.to)}` : `As of ${date(new Date().toISOString())}`}
            <div>Printed {dateTime(new Date().toISOString())}</div>
          </div>
        </div>
        <Report k={key} range={range} setExporter={noop} />
      </div>,
      'a4',
    );

  return (
    <div>
      <PageHeader
        title="Reports"
        description="Figures are calculated from recorded sales, costs, returns and expenses. Voided records are excluded."
        icon={<ChartColumn className="h-5 w-5" />}
        actions={
          <>
            {item.ranged && <DateRangePicker value={range} onChange={setRange} />}
            <Button
              icon={<Download className="h-4 w-4" />}
              disabled={!exporter}
              onClick={async () => {
                try {
                  if (exporter && (await exporter.run())) toast.success('Exported', exporter.name);
                } catch (e) {
                  toast.error(e);
                }
              }}
            >
              CSV
            </Button>
            <Button icon={<Printer className="h-4 w-4" />} onClick={doPrint}>
              Print
            </Button>
          </>
        }
      />
      <div className="grid grid-cols-[220px_1fr] gap-5">
        <nav className="card h-fit p-2">
          {MENU.map((g) => (
            <div key={g.group} className="mb-2">
              <p className="px-3 pb-1 pt-2 text-2xs font-semibold uppercase tracking-wide text-slate-400">{g.group}</p>
              {g.items.map((i) => (
                <button
                  key={i.key}
                  onClick={() => {
                    setExporter(null);
                    setKey(i.key);
                  }}
                  className={cx('block w-full rounded-lg px-3 py-2 text-left text-sm', key === i.key ? 'bg-brand-50 font-semibold text-brand-700' : 'text-slate-600 hover:bg-slate-50')}
                >
                  {i.label}
                </button>
              ))}
            </div>
          ))}
        </nav>
        <div className="min-w-0">{body}</div>
      </div>
    </div>
  );
}

function Report({ k, range, setExporter }: { k: ReportKey; range: DateRange; setExporter: (e: Exporter) => void }) {
  switch (k) {
    case 'sales':
      return <SalesSummaryReport range={range} setExporter={setExporter} />;
    case 'pl':
      return <ProfitLossReport range={range} />;
    case 'products':
      return <ProductSalesReport range={range} setExporter={setExporter} />;
    case 'categories':
      return <CategoryReport range={range} setExporter={setExporter} />;
    case 'payments':
      return <PaymentMethodsReport range={range} />;
    case 'returns':
      return <ReturnsReport range={range} />;
    case 'purchases':
      return <PurchasesReport range={range} setExporter={setExporter} />;
    case 'expenses':
      return <ExpensesReport range={range} />;
    case 'customers':
      return <CustomerBalances setExporter={setExporter} />;
    case 'suppliers':
      return <SupplierBalances setExporter={setExporter} />;
    case 'valuation':
      return <ValuationReport setExporter={setExporter} />;
    case 'lowstock':
      return <LowStockReport />;
    case 'expiry':
      return <ExpiryReport />;
  }
}

function Box({ children, title, actions }: { children: ReactNode; title?: string; actions?: ReactNode }) {
  return (
    <div className="card overflow-hidden">
      {(title || actions) && (
        <div className="card-header">
          {title && <h2 className="card-title">{title}</h2>}
          {actions}
        </div>
      )}
      {children}
    </div>
  );
}

function useReport<T>(key: unknown[], fn: () => Promise<T>) {
  return useQuery({ queryKey: ['report', ...key], queryFn: fn });
}

function Loading({ q, children }: { q: { isLoading: boolean; error: unknown; refetch: () => void }; children: ReactNode }) {
  if (q.isLoading) return <Spinner />;
  if (q.error) return <ErrorBox error={q.error} onRetry={q.refetch} />;
  return <>{children}</>;
}

function useExport<T>(setExporter: (e: Exporter) => void, name: string, rows: T[] | undefined, columns: Column<T>[]) {
  const cols = useRef(columns);
  cols.current = columns;
  useEffect(() => {
    setExporter(rows ? { name, run: () => exportCsv(name, rows, cols.current) } : null);
  }, [rows, name, setExporter]);
}

// ------------------------------------------------------------------ sales

function SalesSummaryReport({ range, setExporter }: { range: DateRange; setExporter: (e: Exporter) => void }) {
  const [group, setGroup] = useState<'day' | 'week' | 'month'>('day');
  const q = useReport(['sales', range, group], () => api.reports.salesSummary(range, group));
  useExport(setExporter, `sales_${group}_${range.from}_${range.to}.csv`, q.data?.rows, [
    { header: 'Period', value: (r) => r.period },
    { header: 'Bills', value: (r) => r.invoices },
    { header: 'Gross sales', value: (r) => csvMoney(r.grossSales) },
    { header: 'Discounts', value: (r) => csvMoney(r.discounts) },
    { header: 'Returns', value: (r) => csvMoney(r.returnsTotal) },
    { header: 'Net sales', value: (r) => csvMoney(r.netSales) },
    { header: 'Cost of goods', value: (r) => csvMoney(r.cogs) },
    { header: 'Gross profit', value: (r) => csvMoney(r.grossProfit) },
  ]);
  const label = (p: string) => (group === 'month' ? p : group === 'week' ? `Week of ${shortDate(p)}` : date(p));
  return (
    <Box
      title="Sales summary"
      actions={
        <Select className="w-32" value={group} onChange={(e) => setGroup(e.target.value as typeof group)}>
          <option value="day">Daily</option>
          <option value="week">Weekly</option>
          <option value="month">Monthly</option>
        </Select>
      }
    >
      <Loading q={q}>
        {!q.data?.rows.length ? (
          <EmptyState title="No sales in this period" />
        ) : (
          <table className="table">
            <thead>
              <tr>
                <th>Period</th>
                <th className="text-right">Bills</th>
                <th className="text-right">Gross sales</th>
                <th className="text-right">Discounts</th>
                <th className="text-right">Returns</th>
                <th className="text-right">Net sales</th>
                <th className="text-right">Cost of goods</th>
                <th className="text-right">Gross profit</th>
                <th className="text-right">Margin</th>
              </tr>
            </thead>
            <tbody>
              {q.data.rows.map((r) => (
                <tr key={r.period}>
                  <td className="font-medium text-slate-900">{label(r.period)}</td>
                  <td className="num text-right">{r.invoices}</td>
                  <td className="num text-right">{money(r.grossSales)}</td>
                  <td className="num text-right">{money(r.discounts)}</td>
                  <td className="num text-right">{money(r.returnsTotal)}</td>
                  <td className="num text-right font-semibold">{money(r.netSales)}</td>
                  <td className="num text-right">{money(r.cogs)}</td>
                  <td className="num text-right font-semibold">{money(r.grossProfit)}</td>
                  <td className="num text-right">{percent(r.marginBp)}</td>
                </tr>
              ))}
            </tbody>
            <tfoot>
              <tr>
                <td>Total</td>
                <td className="num text-right">{q.data.totals.invoices}</td>
                <td className="num text-right">{money(q.data.totals.grossSales)}</td>
                <td className="num text-right">{money(q.data.totals.discounts)}</td>
                <td className="num text-right">{money(q.data.totals.returnsTotal)}</td>
                <td className="num text-right">{money(q.data.totals.netSales)}</td>
                <td className="num text-right">{money(q.data.totals.cogs)}</td>
                <td className="num text-right">{money(q.data.totals.grossProfit)}</td>
                <td className="num text-right">{percent(q.data.totals.marginBp)}</td>
              </tr>
            </tfoot>
          </table>
        )}
      </Loading>
    </Box>
  );
}

function ProfitLossReport({ range }: { range: DateRange }) {
  const q = useReport(['pl', range], () => api.reports.profitLoss(range));
  const d = q.data;
  return (
    <Box title="Profit & loss statement">
      <Loading q={q}>
        {d && (
          <div className="grid grid-cols-2 gap-8 p-6">
            <div>
              <p className="mb-1 text-xs font-semibold uppercase text-slate-500">Trading</p>
              <KeyValue label="Gross sales (before discounts)" value={money(d.grossSales)} />
              <KeyValue label="Discounts given" value={`−${money(d.discounts)}`} />
              <KeyValue label="Returns & refunds" value={`−${money(d.returns)}`} />
              <div className="my-1 border-t border-slate-200" />
              <KeyValue label="Net sales" value={money(d.netSales)} strong />
              <KeyValue label="Cost of goods sold" value={`−${money(d.cogsSold)}`} />
              <KeyValue label="Cost of goods returned to stock" value={money(d.cogsReturned)} />
              <div className="my-1 border-t border-slate-200" />
              <KeyValue label={`Gross profit (margin ${percent(d.grossMarginBp)})`} value={money(d.grossProfit)} strong />
              <p className="mt-3 text-2xs text-slate-500">
                Cost of goods is the weighted-average cost recorded on each sale line at the time of sale. Purchases themselves are not
                expenses.
              </p>
            </div>
            <div>
              <p className="mb-1 text-xs font-semibold uppercase text-slate-500">Operating expenses</p>
              {d.expenses.length === 0 && <p className="py-1.5 text-sm text-slate-500">No expenses recorded</p>}
              {d.expenses.map((e) => (
                <KeyValue key={e.name} label={`${e.name} (${e.count})`} value={`−${money(e.amount)}`} />
              ))}
              <div className="my-1 border-t border-slate-200" />
              <KeyValue label="Total expenses" value={`−${money(d.expensesTotal)}`} strong />
              <KeyValue label="Stock adjustments (gains − write-offs)" value={money(d.inventoryAdjustments)} />
              <div className="mt-3 rounded-xl bg-slate-50 p-4">
                <KeyValue
                  label="Net operating result"
                  value={<span className={cx('text-lg', d.netOperatingResult < 0 ? 'text-rose-700' : 'text-emerald-700')}>{money(d.netOperatingResult)}</span>}
                  strong
                />
              </div>
              <p className="mt-3 text-2xs text-slate-500">
                {d.invoices} bills · average bill {money(d.averageBill)}. Owner withdrawals, taxes and loan repayments are not included.
              </p>
            </div>
          </div>
        )}
      </Loading>
    </Box>
  );
}

function ProductSalesReport({ range, setExporter }: { range: DateRange; setExporter: (e: Exporter) => void }) {
  const q = useReport(['products', range], () => api.reports.productSales(range));
  useExport(setExporter, `product_sales_${range.from}_${range.to}.csv`, q.data, [
    { header: 'Product', value: (r) => r.name },
    { header: 'SKU', value: (r) => r.sku },
    { header: 'Category', value: (r) => r.category },
    { header: 'Brand', value: (r) => r.brand },
    { header: 'Qty sold', value: (r) => csvQty(r.qtySold) },
    { header: 'Qty returned', value: (r) => csvQty(r.qtyReturned) },
    { header: 'Net sales', value: (r) => csvMoney(r.netSales) },
    { header: 'Cost', value: (r) => csvMoney(r.cogs) },
    { header: 'Gross profit', value: (r) => csvMoney(r.grossProfit) },
  ]);
  return (
    <Box title="Sales by product (best sellers first)">
      <Loading q={q}>
        {!q.data?.length ? (
          <EmptyState title="No products sold in this period" />
        ) : (
          <table className="table">
            <thead>
              <tr>
                <th>Product</th>
                <th>Category</th>
                <th className="text-right">Sold</th>
                <th className="text-right">Returned</th>
                <th className="text-right">Net sales</th>
                <th className="text-right">Cost</th>
                <th className="text-right">Gross profit</th>
                <th className="text-right">Margin</th>
              </tr>
            </thead>
            <tbody>
              {q.data.map((r) => (
                <tr key={r.productId}>
                  <td>
                    <p className="font-medium text-slate-900">{r.name}</p>
                    <p className="text-2xs text-slate-500">{r.sku}</p>
                  </td>
                  <td className="text-slate-600">{r.category}</td>
                  <td className="num text-right">{qty(r.qtySold)}</td>
                  <td className="num text-right">{r.qtyReturned ? qty(r.qtyReturned) : '—'}</td>
                  <td className="num text-right font-semibold">{money(r.netSales)}</td>
                  <td className="num text-right">{money(r.cogs)}</td>
                  <td className="num text-right font-semibold">{money(r.grossProfit)}</td>
                  <td className="num text-right">{percent(r.marginBp)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </Loading>
    </Box>
  );
}

function CategoryReport({ range, setExporter }: { range: DateRange; setExporter: (e: Exporter) => void }) {
  const q = useReport(['categories', range], () => api.reports.categorySales(range));
  useExport(setExporter, `category_sales_${range.from}_${range.to}.csv`, q.data, [
    { header: 'Category', value: (r) => r.category },
    { header: 'Products', value: (r) => r.products },
    { header: 'Qty', value: (r) => csvQty(r.qtySold) },
    { header: 'Net sales', value: (r) => csvMoney(r.netSales) },
    { header: 'Cost', value: (r) => csvMoney(r.cogs) },
    { header: 'Gross profit', value: (r) => csvMoney(r.grossProfit) },
  ]);
  return (
    <Box title="Sales by category">
      <Loading q={q}>
        {!q.data?.length ? (
          <EmptyState title="No sales in this period" />
        ) : (
          <table className="table">
            <thead>
              <tr>
                <th>Category</th>
                <th className="text-right">Products</th>
                <th className="text-right">Qty</th>
                <th className="text-right">Net sales</th>
                <th className="w-48">Share of sales</th>
                <th className="text-right">Gross profit</th>
                <th className="text-right">Margin</th>
              </tr>
            </thead>
            <tbody>
              {q.data.map((r) => (
                <tr key={r.category}>
                  <td className="font-medium text-slate-900">{r.category}</td>
                  <td className="num text-right">{r.products}</td>
                  <td className="num text-right">{qty(r.qtySold)}</td>
                  <td className="num text-right font-semibold">{money(r.netSales)}</td>
                  <td>
                    <div className="flex items-center gap-2">
                      <div className="h-1.5 flex-1 rounded-full bg-slate-100">
                        <div className="h-1.5 rounded-full bg-brand-500" style={{ width: `${Math.max(r.shareBp / 100, 0)}%` }} />
                      </div>
                      <span className="num w-12 text-right text-xs text-slate-600">{percent(r.shareBp)}</span>
                    </div>
                  </td>
                  <td className="num text-right">{money(r.grossProfit)}</td>
                  <td className="num text-right">{percent(r.marginBp)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </Loading>
    </Box>
  );
}

function PaymentMethodsReport({ range }: { range: DateRange }) {
  const q = useReport(['payments', range], () => api.reports.paymentMethods(range));
  const total = q.data?.reduce((s, r) => s + r.amount, 0) ?? 0;
  return (
    <Box title="How customers paid">
      <Loading q={q}>
        {!q.data?.length ? (
          <EmptyState title="No payments in this period" />
        ) : (
          <table className="table">
            <thead>
              <tr>
                <th>Method</th>
                <th className="text-right">Bills</th>
                <th className="text-right">Amount</th>
                <th className="text-right">Share</th>
              </tr>
            </thead>
            <tbody>
              {q.data.map((r) => (
                <tr key={r.name}>
                  <td className="font-medium text-slate-900">{methodLabel(r.name)}</td>
                  <td className="num text-right">{r.count}</td>
                  <td className="num text-right font-semibold">{money(r.amount)}</td>
                  <td className="num text-right">{total ? percent(Math.round((r.amount * 10000) / total)) : '—'}</td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </Loading>
    </Box>
  );
}

function ReturnsReport({ range }: { range: DateRange }) {
  const q = useReport(['returns', range], () => api.reports.returns(range));
  const d = q.data;
  return (
    <Loading q={q}>
      {d && (
        <div className="space-y-5">
          <div className="grid grid-cols-3 gap-4">
            <div className="card p-4">
              <p className="text-xs text-slate-500">Returns</p>
              <p className="num mt-1 text-2xl font-semibold">{d.count}</p>
            </div>
            <div className="card p-4">
              <p className="text-xs text-slate-500">Total refunded</p>
              <p className="num mt-1 text-2xl font-semibold">{money(d.totalRefund)}</p>
            </div>
            <div className="card p-4">
              <p className="mb-1 text-xs text-slate-500">By refund method</p>
              {d.byMethod.map((m) => (
                <KeyValue key={m.name} label={methodLabel(m.name)} value={money(m.amount)} />
              ))}
            </div>
          </div>
          <Box title="Most returned products">
            {!d.topProducts.length ? (
              <EmptyState title="No returns in this period" />
            ) : (
              <table className="table">
                <thead>
                  <tr>
                    <th>Product</th>
                    <th className="text-right">Sold</th>
                    <th className="text-right">Returned</th>
                    <th className="text-right">Refunded</th>
                  </tr>
                </thead>
                <tbody>
                  {d.topProducts.map((p) => (
                    <tr key={p.productId}>
                      <td className="font-medium text-slate-900">{p.name}</td>
                      <td className="num text-right">{qty(p.qtySold)}</td>
                      <td className="num text-right">{qty(p.qtyReturned)}</td>
                      <td className="num text-right font-semibold">{money(p.returnsAmount)}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            )}
          </Box>
        </div>
      )}
    </Loading>
  );
}

// ---------------------------------------------------------------- buying

function PurchasesReport({ range, setExporter }: { range: DateRange; setExporter: (e: Exporter) => void }) {
  const q = useReport(['purchases', range], () => api.reports.purchases(range));
  useExport(setExporter, `purchases_${range.from}_${range.to}.csv`, q.data?.documents, [
    { header: 'No.', value: (r) => r.purchaseNo },
    { header: 'Date', value: (r) => r.businessDate },
    { header: 'Supplier', value: (r) => r.supplierName },
    { header: 'Supplier invoice', value: (r) => r.supplierInvoiceNo },
    { header: 'Subtotal', value: (r) => csvMoney(r.subtotal) },
    { header: 'Discount', value: (r) => csvMoney(r.discount) },
    { header: 'Total', value: (r) => csvMoney(r.total) },
    { header: 'Paid', value: (r) => csvMoney(r.paidAmount) },
  ]);
  const d = q.data;
  return (
    <Loading q={q}>
      {d && (
        <div className="space-y-5">
          <div className="grid grid-cols-4 gap-4">
            <div className="card p-4">
              <p className="text-xs text-slate-500">Purchases ({d.invoices} invoices)</p>
              <p className="num mt-1 text-xl font-semibold">{money(d.purchasesTotal)}</p>
            </div>
            <div className="card p-4">
              <p className="text-xs text-slate-500">Returned to suppliers</p>
              <p className="num mt-1 text-xl font-semibold">{money(d.returnsTotal)}</p>
            </div>
            <div className="card p-4">
              <p className="text-xs text-slate-500">Net purchases</p>
              <p className="num mt-1 text-xl font-semibold">{money(d.netPurchases)}</p>
            </div>
            <div className="card p-4">
              <p className="text-xs text-slate-500">Paid to suppliers</p>
              <p className="num mt-1 text-xl font-semibold">{money(d.paymentsTotal)}</p>
            </div>
          </div>
          <Box title="By supplier">
            {!d.bySupplier.length ? (
              <EmptyState title="No purchases in this period" />
            ) : (
              <table className="table">
                <thead>
                  <tr>
                    <th>Supplier</th>
                    <th className="text-right">Invoices</th>
                    <th className="text-right">Purchases</th>
                    <th className="text-right">Returns</th>
                    <th className="text-right">Payments</th>
                  </tr>
                </thead>
                <tbody>
                  {d.bySupplier.map((s) => (
                    <tr key={s.supplierId}>
                      <td className="font-medium text-slate-900">{s.supplierName}</td>
                      <td className="num text-right">{s.invoices}</td>
                      <td className="num text-right">{money(s.purchases)}</td>
                      <td className="num text-right">{money(s.returns)}</td>
                      <td className="num text-right">{money(s.payments)}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            )}
          </Box>
        </div>
      )}
    </Loading>
  );
}

function ExpensesReport({ range }: { range: DateRange }) {
  const q = useReport(['expenses', range], () => api.reports.expenses(range));
  const d = q.data;
  return (
    <Loading q={q}>
      {d && (
        <div className="grid grid-cols-2 gap-5">
          <Box title={`By category · ${money(d.total)} in ${d.count} entries`}>
            {!d.byCategory.length ? (
              <EmptyState title="No expenses in this period" />
            ) : (
              <table className="table">
                <tbody>
                  {d.byCategory.map((c) => (
                    <tr key={c.name}>
                      <td className="font-medium text-slate-900">{c.name}</td>
                      <td className="num text-right text-slate-500">{c.count}×</td>
                      <td className="num text-right font-semibold">{money(c.amount)}</td>
                      <td className="num w-20 text-right text-slate-500">{d.total ? percent(Math.round((c.amount * 10000) / d.total)) : ''}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            )}
          </Box>
          <Box title="By payment method">
            {!d.byMethod.length ? (
              <EmptyState title="No expenses in this period" />
            ) : (
              <table className="table">
                <tbody>
                  {d.byMethod.map((c) => (
                    <tr key={c.name}>
                      <td className="font-medium text-slate-900">{methodLabel(c.name)}</td>
                      <td className="num text-right text-slate-500">{c.count}×</td>
                      <td className="num text-right font-semibold">{money(c.amount)}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            )}
          </Box>
        </div>
      )}
    </Loading>
  );
}

// -------------------------------------------------------------- balances

function CustomerBalances({ setExporter }: { setExporter: (e: Exporter) => void }) {
  const q = useReport(['customer-balances'], api.reports.customerBalances);
  useExport(setExporter, 'customer_balances.csv', q.data, [
    { header: 'Customer', value: (c) => c.name },
    { header: 'Phone', value: (c) => c.phone },
    { header: 'Balance', value: (c) => csvMoney(c.balance) },
    { header: 'Credit limit', value: (c) => csvMoney(c.creditLimit) },
    { header: 'Last purchase', value: (c) => c.lastPurchaseDate },
  ]);
  const total = q.data?.reduce((s, c) => s + c.balance, 0) ?? 0;
  return (
    <Box title={`Customer balances · total ${money(total)}`}>
      <Loading q={q}>
        {!q.data?.length ? (
          <EmptyState title="No customer owes anything" />
        ) : (
          <table className="table">
            <thead>
              <tr>
                <th>Customer</th>
                <th>Phone</th>
                <th>Last purchase</th>
                <th className="text-right">Credit limit</th>
                <th className="text-right">Balance</th>
              </tr>
            </thead>
            <tbody>
              {q.data.map((c) => (
                <tr key={c.id}>
                  <td className="font-medium text-slate-900">{c.name}</td>
                  <td className="text-slate-600">{c.phone ?? '—'}</td>
                  <td className="text-slate-600">{date(c.lastPurchaseDate)}</td>
                  <td className="num text-right">{c.creditLimit !== null ? money(c.creditLimit) : '—'}</td>
                  <td className={cx('num text-right font-semibold', c.balance > 0 ? 'text-amber-700' : 'text-emerald-700')}>{money(c.balance)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </Loading>
    </Box>
  );
}

function SupplierBalances({ setExporter }: { setExporter: (e: Exporter) => void }) {
  const q = useReport(['supplier-balances'], api.reports.supplierBalances);
  useExport(setExporter, 'supplier_balances.csv', q.data, [
    { header: 'Supplier', value: (s) => s.name },
    { header: 'Opening', value: (s) => csvMoney(s.openingBalance) },
    { header: 'Purchases', value: (s) => csvMoney(s.totalPurchases) },
    { header: 'Returns', value: (s) => csvMoney(s.totalReturns) },
    { header: 'Payments', value: (s) => csvMoney(s.totalPayments) },
    { header: 'Balance', value: (s) => csvMoney(s.balance) },
  ]);
  const total = q.data?.reduce((s, x) => s + x.balance, 0) ?? 0;
  return (
    <Box title={`Supplier balances · total owed ${money(total)}`}>
      <Loading q={q}>
        {!q.data?.length ? (
          <EmptyState title="No suppliers" />
        ) : (
          <table className="table">
            <thead>
              <tr>
                <th>Supplier</th>
                <th className="text-right">Opening</th>
                <th className="text-right">Purchases</th>
                <th className="text-right">Returns</th>
                <th className="text-right">Payments</th>
                <th className="text-right">Balance owed</th>
              </tr>
            </thead>
            <tbody>
              {q.data.map((s) => (
                <tr key={s.id}>
                  <td className="font-medium text-slate-900">{s.name}</td>
                  <td className="num text-right">{money(s.openingBalance)}</td>
                  <td className="num text-right">{money(s.totalPurchases)}</td>
                  <td className="num text-right">{money(s.totalReturns)}</td>
                  <td className="num text-right">{money(s.totalPayments)}</td>
                  <td className="num text-right font-semibold">{money(s.balance)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </Loading>
    </Box>
  );
}

// ------------------------------------------------------------------ stock

function ValuationReport({ setExporter }: { setExporter: (e: Exporter) => void }) {
  const q = useReport(['valuation'], api.reports.inventoryValuation);
  useExport(setExporter, 'inventory_valuation.csv', q.data?.rows, [
    { header: 'Product', value: (r) => r.name },
    { header: 'SKU', value: (r) => r.sku },
    { header: 'Category', value: (r) => r.category },
    { header: 'Qty', value: (r) => csvQty(r.qty) },
    { header: 'Avg cost', value: (r) => csvMoney(r.unitCost) },
    { header: 'Cost value', value: (r) => csvMoney(r.costValue) },
    { header: 'Sale price', value: (r) => csvMoney(r.salePrice) },
    { header: 'Retail value', value: (r) => csvMoney(r.retailValue) },
  ]);
  const d = q.data;
  return (
    <Loading q={q}>
      {d && (
        <div className="space-y-5">
          <div className="grid grid-cols-3 gap-4">
            <div className="card p-4">
              <p className="text-xs text-slate-500">Value at cost</p>
              <p className="num mt-1 text-xl font-semibold">{money(d.totalCostValue)}</p>
            </div>
            <div className="card p-4">
              <p className="text-xs text-slate-500">Value at selling price</p>
              <p className="num mt-1 text-xl font-semibold">{money(d.totalRetailValue)}</p>
            </div>
            <div className="card p-4">
              <p className="text-xs text-slate-500">Products in stock</p>
              <p className="num mt-1 text-xl font-semibold">{d.productsInStock}</p>
            </div>
          </div>
          <Box title="Stock on hand">
            {!d.rows.length ? (
              <EmptyState title="No stock on hand" />
            ) : (
              <table className="table">
                <thead>
                  <tr>
                    <th>Product</th>
                    <th>Category</th>
                    <th className="text-right">Qty</th>
                    <th className="text-right">Avg. cost</th>
                    <th className="text-right">Cost value</th>
                    <th className="text-right">Retail value</th>
                  </tr>
                </thead>
                <tbody>
                  {d.rows.map((r) => (
                    <tr key={r.productId}>
                      <td>
                        <p className="font-medium text-slate-900">{r.name}</p>
                        <p className="text-2xs text-slate-500">{r.sku}</p>
                      </td>
                      <td className="text-slate-600">{r.category}</td>
                      <td className={cx('num text-right', r.qty < 0 && 'text-rose-700')}>
                        {qty(r.qty)} {r.unitShort}
                      </td>
                      <td className="num text-right">{money(r.unitCost)}</td>
                      <td className="num text-right font-semibold">{money(r.costValue)}</td>
                      <td className="num text-right">{money(r.retailValue)}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            )}
          </Box>
        </div>
      )}
    </Loading>
  );
}

function LowStockReport() {
  const q = useReport(['lowstock'], api.reports.lowStock);
  return (
    <Box title="Products at or below their reorder level">
      <Loading q={q}>
        {!q.data?.length ? (
          <EmptyState title="Nothing needs restocking" description="Set a reorder level on products to be warned early." />
        ) : (
          <table className="table">
            <thead>
              <tr>
                <th>Product</th>
                <th>Brand</th>
                <th className="text-right">On hand</th>
                <th className="text-right">Reorder level</th>
              </tr>
            </thead>
            <tbody>
              {q.data.map((p) => (
                <tr key={p.id}>
                  <td>
                    <p className="font-medium text-slate-900">{p.displayName}</p>
                    <p className="text-2xs text-slate-500">{p.sku}</p>
                  </td>
                  <td className="text-slate-600">{p.brandName ?? '—'}</td>
                  <td className={cx('num text-right font-semibold', p.stockQty <= 0 ? 'text-rose-700' : 'text-amber-700')}>
                    {qty(p.stockQty)} {p.unitShort}
                  </td>
                  <td className="num text-right">{qty(p.reorderLevel)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </Loading>
    </Box>
  );
}

function ExpiryReport() {
  const settings = useSettings();
  const [days, setDays] = useState(settings.expiryWarningDays);
  const q = useReport(['expiry', days], () => api.reports.expiring(days));
  return (
    <Box
      title="Batches expiring soon"
      actions={
        <Select className="w-44" value={String(days)} onChange={(e) => setDays(Number(e.target.value))}>
          {[0, 30, 60, 90, 180, 365].map((d) => (
            <option key={d} value={d}>
              {d === 0 ? 'Already expired' : `Within ${d} days`}
            </option>
          ))}
        </Select>
      }
    >
      <Loading q={q}>
        {!q.data?.length ? (
          <EmptyState title="No batches expiring in this window" />
        ) : (
          <table className="table">
            <thead>
              <tr>
                <th>Product</th>
                <th>Batch</th>
                <th>Expiry</th>
                <th className="text-right">Days left</th>
                <th className="text-right">Remaining</th>
              </tr>
            </thead>
            <tbody>
              {q.data.map((b) => (
                <tr key={b.id}>
                  <td className="font-medium text-slate-900">{b.productName}</td>
                  <td>{b.batchNo ?? '—'}</td>
                  <td>{date(b.expiryDate)}</td>
                  <td className={cx('num text-right font-semibold', (b.daysToExpiry ?? 0) < 0 ? 'text-rose-700' : 'text-amber-700')}>
                    {(b.daysToExpiry ?? 0) < 0 ? 'Expired' : b.daysToExpiry}
                  </td>
                  <td className="num text-right">
                    {qty(b.qtyRemaining)} {b.unitShort}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </Loading>
    </Box>
  );
}
