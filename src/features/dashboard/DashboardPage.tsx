import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { useQuery } from '@tanstack/react-query';
import {
  Boxes,
  CalendarClock,
  CircleDollarSign,
  HandCoins,
  LayoutDashboard,
  PackageX,
  Receipt,
  ShoppingCart,
  TrendingDown,
  TrendingUp,
  Truck,
  Wallet,
} from 'lucide-react';
import { api } from '@/lib/api';
import { date, money, percent, qty, shortDate, time } from '@/lib/format';
import { useSession } from '@/lib/session';
import { cx } from '@/lib/utils';
import { Badge, Button, EmptyState, ErrorBox, KeyValue, PageHeader, Spinner, StatCard } from '@/components/ui';
import { SalesTrendChart } from './SalesTrendChart';

function greeting(): string {
  const h = new Date().getHours();
  return h < 12 ? 'Good morning' : h < 17 ? 'Good afternoon' : 'Good evening';
}

export function DashboardPage() {
  const navigate = useNavigate();
  const { session, can } = useSession();
  const [asTable, setAsTable] = useState(false);
  const { data: d, isLoading, error, refetch } = useQuery({ queryKey: ['dashboard'], queryFn: api.reports.dashboard, refetchInterval: 60_000 });

  if (isLoading) return <Spinner />;
  if (error || !d) return <ErrorBox error={error} onRetry={refetch} />;

  const net = d.day.netOperatingResult;
  return (
    <div className="space-y-5">
      <PageHeader
        title={`${greeting()}, ${session?.user.fullName.split(' ')[0]}`}
        description={`Here is how the shop is doing today, ${date(d.today)}.`}
        icon={<LayoutDashboard className="h-5 w-5" />}
        actions={
          can('sell') && (
            <Button variant="primary" icon={<ShoppingCart className="h-4 w-4" />} onClick={() => navigate('/pos')}>
              New sale <span className="kbd ml-1 border-brand-400 bg-brand-500 text-white">F1</span>
            </Button>
          )
        }
      />

      <div className="grid grid-cols-4 gap-4">
        <StatCard
          label="Today's sales (net)"
          value={money(d.day.netSales)}
          sub={`${d.day.invoices} bill(s) · average ${money(d.day.averageBill)}`}
          icon={<Receipt className="h-4 w-4" />}
        />
        <StatCard
          label="Gross profit today"
          value={money(d.day.grossProfit)}
          sub={d.day.netSales ? `Margin ${percent(d.day.grossMarginBp)}` : 'From recorded cost of goods sold'}
          icon={<TrendingUp className="h-4 w-4" />}
          tone="green"
        />
        <StatCard
          label="Expenses today"
          value={money(d.day.expensesTotal)}
          sub={d.day.returns ? `Refunds given: ${money(d.day.returns)}` : 'Rent, bills, salaries…'}
          icon={<Wallet className="h-4 w-4" />}
          tone="amber"
        />
        <StatCard
          label="Net operating result today"
          value={<span className={net < 0 ? 'text-rose-700' : undefined}>{money(net)}</span>}
          sub="Gross profit − expenses ± stock adjustments"
          icon={net < 0 ? <TrendingDown className="h-4 w-4" /> : <CircleDollarSign className="h-4 w-4" />}
          tone={net < 0 ? 'red' : 'brand'}
        />
      </div>

      <div className="grid grid-cols-4 gap-4">
        <StatCard label="Stock value (cost)" value={money(d.stockValue)} sub={`At selling price ${money(d.stockRetailValue)}`} icon={<Boxes className="h-4 w-4" />} tone="gray" />
        <StatCard
          label="Low / out of stock"
          value={
            <span>
              {d.lowStockCount} <span className="text-slate-300">/</span> <span className={d.outOfStockCount ? 'text-rose-700' : ''}>{d.outOfStockCount}</span>
            </span>
          }
          sub={d.expiringSoon ? `${d.expiringSoon} batch(es) expiring soon` : 'No batches expiring soon'}
          icon={<PackageX className="h-4 w-4" />}
          tone={d.outOfStockCount ? 'red' : 'amber'}
        />
        <StatCard label="Customers owe you" value={money(d.receivables)} sub="Credit (udhaar) balances" icon={<HandCoins className="h-4 w-4" />} tone="blue" />
        <StatCard label="You owe suppliers" value={money(d.payables)} sub={`Purchases this month ${money(d.purchasesMonth)}`} icon={<Truck className="h-4 w-4" />} tone="gray" />
      </div>

      <div className="grid grid-cols-3 gap-5">
        <div className="card col-span-2">
          <div className="card-header">
            <div>
              <h2 className="card-title">Net sales, last 14 days</h2>
              <p className="text-2xs text-slate-500">After discounts and returns</p>
            </div>
            <div className="flex rounded-lg bg-slate-100 p-0.5 text-xs">
              {(['Chart', 'Table'] as const).map((v) => (
                <button key={v} className={cx('rounded-md px-2.5 py-1 font-medium', asTable === (v === 'Table') ? 'bg-white shadow-sm' : 'text-slate-500')} onClick={() => setAsTable(v === 'Table')}>
                  {v}
                </button>
              ))}
            </div>
          </div>
          <div className="p-4">
            {asTable ? (
              <div className="max-h-56 overflow-y-auto">
                <table className="table">
                  <thead>
                    <tr>
                      <th>Day</th>
                      <th className="text-right">Bills</th>
                      <th className="text-right">Net sales</th>
                      <th className="text-right">Gross profit</th>
                    </tr>
                  </thead>
                  <tbody>
                    {[...d.trend].reverse().map((p) => (
                      <tr key={p.date}>
                        <td>{shortDate(p.date)}</td>
                        <td className="num text-right">{p.invoices}</td>
                        <td className="num text-right">{money(p.netSales)}</td>
                        <td className="num text-right">{money(p.grossProfit)}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            ) : (
              <SalesTrendChart points={d.trend} />
            )}
          </div>
        </div>
        <div className="card p-5">
          <h2 className="card-title">This month</h2>
          <div className="mt-3">
            <KeyValue label="Net sales" value={money(d.month.netSales)} />
            <KeyValue label="Cost of goods sold" value={`−${money(d.month.cogs)}`} />
            <KeyValue label="Gross profit" value={money(d.month.grossProfit)} strong />
            <KeyValue label="Expenses" value={`−${money(d.month.expensesTotal)}`} />
            {d.month.inventoryAdjustments !== 0 && <KeyValue label="Stock adjustments" value={money(d.month.inventoryAdjustments)} />}
            <div className="mt-2 border-t border-slate-200 pt-2">
              <KeyValue
                label="Net operating result"
                value={<span className={d.month.netOperatingResult < 0 ? 'text-rose-700' : 'text-emerald-700'}>{money(d.month.netOperatingResult)}</span>}
                strong
              />
            </div>
            <p className="mt-3 text-2xs text-slate-500">
              {d.month.invoices} bills · margin {percent(d.month.grossMarginBp)}. Purchases are not expenses — they count as cost only when the goods are sold.
            </p>
          </div>
        </div>
      </div>

      <div className="grid grid-cols-2 gap-5">
        <div className="card">
          <div className="card-header">
            <h2 className="card-title">Needs restocking</h2>
            <Button size="sm" variant="ghost" onClick={() => navigate('/inventory')}>
              Inventory
            </Button>
          </div>
          {d.lowStock.length === 0 ? (
            <EmptyState icon={<Boxes className="h-6 w-6" />} title="Stock levels look fine" description="Set reorder levels on products to get early warnings." />
          ) : (
            <table className="table">
              <tbody>
                {d.lowStock.map((p) => (
                  <tr key={p.id}>
                    <td>
                      <p className="font-medium text-slate-900">{p.displayName}</p>
                      <p className="text-2xs text-slate-500">{p.sku}</p>
                    </td>
                    <td className="text-right">
                      <Badge tone={p.stockQty <= 0 ? 'red' : 'amber'}>
                        {qty(p.stockQty)} {p.unitShort}
                      </Badge>
                      {p.reorderLevel > 0 && <p className="mt-0.5 text-2xs text-slate-400">reorder at {qty(p.reorderLevel)}</p>}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </div>
        <div className="card">
          <div className="card-header">
            <h2 className="card-title">Recent sales</h2>
            <Button size="sm" variant="ghost" onClick={() => navigate('/sales')}>
              All sales
            </Button>
          </div>
          {d.recentSales.length === 0 ? (
            <EmptyState icon={<Receipt className="h-6 w-6" />} title="No sales yet" description="Sales appear here as soon as they are made." />
          ) : (
            <table className="table">
              <tbody>
                {d.recentSales.map((s) => (
                  <tr key={s.id}>
                    <td>
                      <p className="font-medium text-slate-900">{s.invoiceNo}</p>
                      <p className="text-2xs text-slate-500">
                        {date(s.businessDate)} · {time(s.createdAt)} · {s.customerName ?? 'Walk-in'}
                      </p>
                    </td>
                    <td className="text-right">
                      <p className="num font-semibold text-slate-900">{money(s.total)}</p>
                      {s.status === 'void' && <Badge tone="red">Void</Badge>}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
          {d.expiringSoon > 0 && (
            <div className="flex items-center gap-2 border-t border-slate-100 px-5 py-3 text-xs text-amber-800">
              <CalendarClock className="h-4 w-4" /> {d.expiringSoon} batch(es) expire soon — see Inventory → Batches & expiry.
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
