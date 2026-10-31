import { useState } from 'react';
import { keepPreviousData, useQuery } from '@tanstack/react-query';
import { Download, Receipt } from 'lucide-react';
import { api, type DateRange } from '@/lib/api';
import { presetRange } from '@/lib/dates';
import { csvMoney, csvQty, exportCsv } from '@/lib/csv';
import { dateTime, money, qty } from '@/lib/format';
import { useDebounced } from '@/lib/utils';
import { Badge, Button, DateRangePicker, EmptyState, ErrorBox, PageHeader, Pager, SearchBox, Select, Spinner, useToast } from '@/components/ui';
import { SaleDetailDialog } from './SaleDetailDialog';

const LIMIT = 50;

export function SalesPage() {
  const toast = useToast();
  const [range, setRange] = useState<DateRange>(presetRange('today'));
  const [search, setSearch] = useState('');
  const [status, setStatus] = useState('');
  const [offset, setOffset] = useState(0);
  const [openId, setOpenId] = useState<string | null>(null);
  const q = useDebounced(search, 200);
  const filter = { search: q, status: status || undefined, dateFrom: q ? undefined : range.from, dateTo: q ? undefined : range.to };
  const { data, isLoading, error, refetch } = useQuery({
    queryKey: ['sales', 'list', filter, offset],
    queryFn: () => api.sales.list({ ...filter, limit: LIMIT, offset }),
    placeholderData: keepPreviousData,
  });

  const exportRows = async () => {
    try {
      const all = await api.sales.list({ ...filter, limit: 1000 });
      const ok = await exportCsv(`sales_${range.from}_${range.to}.csv`, all.items, [
        { header: 'Invoice', value: (r) => r.invoiceNo },
        { header: 'Date', value: (r) => r.businessDate },
        { header: 'Time', value: (r) => dateTime(r.createdAt) },
        { header: 'Status', value: (r) => r.status },
        { header: 'Customer', value: (r) => r.customerName ?? '' },
        { header: 'Items', value: (r) => csvQty(r.totalQty) },
        { header: 'Subtotal', value: (r) => csvMoney(r.subtotal) },
        { header: 'Discount', value: (r) => csvMoney(r.discountTotal) },
        { header: 'Total', value: (r) => csvMoney(r.total) },
        { header: 'Paid', value: (r) => csvMoney(r.paidAmount) },
        { header: 'Credit', value: (r) => csvMoney(r.creditAmount) },
        { header: 'Refunded', value: (r) => csvMoney(r.returnedAmount) },
        { header: 'Cashier', value: (r) => r.cashierName },
      ]);
      if (ok) toast.success('Exported');
    } catch (e) {
      toast.error(e);
    }
  };

  const pageTotal = data?.items.filter((s) => s.status === 'completed').reduce((s, r) => s + r.total - r.returnedAmount, 0) ?? 0;

  return (
    <div>
      <PageHeader
        title="Sales history"
        description="Every bill, with reprint, returns and void."
        icon={<Receipt className="h-5 w-5" />}
        actions={
          <Button icon={<Download className="h-4 w-4" />} onClick={exportRows}>
            Export CSV
          </Button>
        }
      />
      <div className="card">
        <div className="flex flex-wrap items-center gap-3 border-b border-slate-100 p-4">
          <SearchBox value={search} onChange={(v) => { setSearch(v); setOffset(0); }} placeholder="Invoice no, customer or phone (or scan a receipt)" className="w-80" />
          {!q && <DateRangePicker value={range} onChange={(r) => { setRange(r); setOffset(0); }} />}
          <Select value={status} onChange={(e) => { setStatus(e.target.value); setOffset(0); }} className="w-36">
            <option value="">All statuses</option>
            <option value="completed">Completed</option>
            <option value="void">Void</option>
          </Select>
          <span className="ml-auto text-sm text-slate-500">
            Net on this page: <b className="num text-slate-900">{money(pageTotal)}</b>
          </span>
        </div>
        {error ? (
          <ErrorBox error={error} onRetry={refetch} />
        ) : isLoading ? (
          <Spinner />
        ) : !data?.items.length ? (
          <EmptyState icon={<Receipt className="h-6 w-6" />} title="No sales found" description="Try a different date range or search." />
        ) : (
          <>
            <div className="overflow-x-auto">
              <table className="table">
                <thead>
                  <tr>
                    <th>Invoice</th>
                    <th>Date & time</th>
                    <th>Customer</th>
                    <th className="text-right">Items</th>
                    <th className="text-right">Total</th>
                    <th className="text-right">Paid</th>
                    <th>Status</th>
                    <th>Cashier</th>
                  </tr>
                </thead>
                <tbody>
                  {data.items.map((s) => (
                    <tr key={s.id} className="clickable" onClick={() => setOpenId(s.id)}>
                      <td className="font-medium text-slate-900">{s.invoiceNo}</td>
                      <td className="text-slate-600">{dateTime(s.createdAt)}</td>
                      <td>{s.customerName ?? <span className="text-slate-400">Walk-in</span>}</td>
                      <td className="num text-right">{qty(s.totalQty)}</td>
                      <td className="num text-right font-semibold text-slate-900">{money(s.total)}</td>
                      <td className="num text-right">{money(s.paidAmount)}</td>
                      <td>
                        {s.status === 'void' ? (
                          <Badge tone="red">Void</Badge>
                        ) : s.returnedAmount > 0 ? (
                          <Badge tone="amber">Returned {money(s.returnedAmount)}</Badge>
                        ) : s.creditAmount > 0 ? (
                          <Badge tone="blue">Credit {money(s.creditAmount)}</Badge>
                        ) : (
                          <Badge tone="green">Paid</Badge>
                        )}
                      </td>
                      <td className="text-slate-600">{s.cashierName}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
            <Pager total={data.total} limit={LIMIT} offset={offset} onChange={setOffset} />
          </>
        )}
      </div>
      <SaleDetailDialog saleId={openId} onClose={() => setOpenId(null)} />
    </div>
  );
}
