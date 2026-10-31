import { useState } from 'react';
import { keepPreviousData, useQuery } from '@tanstack/react-query';
import { Undo2 } from 'lucide-react';
import { api, type DateRange } from '@/lib/api';
import { presetRange } from '@/lib/dates';
import { dateTime, methodLabel, money } from '@/lib/format';
import { useDebounced } from '@/lib/utils';
import { DateRangePicker, EmptyState, ErrorBox, PageHeader, Pager, SearchBox, Spinner } from '@/components/ui';
import { SaleDetailDialog } from './SaleDetailDialog';

const LIMIT = 50;

export function ReturnsPage() {
  const [range, setRange] = useState<DateRange>(presetRange('last30'));
  const [search, setSearch] = useState('');
  const [offset, setOffset] = useState(0);
  const [saleId, setSaleId] = useState<string | null>(null);
  const q = useDebounced(search, 200);
  const { data, isLoading, error, refetch } = useQuery({
    queryKey: ['returns', range, q, offset],
    queryFn: () => api.sales.returns({ search: q, dateFrom: range.from, dateTo: range.to, limit: LIMIT, offset }),
    placeholderData: keepPreviousData,
  });

  return (
    <div>
      <PageHeader
        title="Returns & refunds"
        description="To make a new return, open the sale in Sales history and choose “Return items”."
        icon={<Undo2 className="h-5 w-5" />}
      />
      <div className="card">
        <div className="flex flex-wrap items-center gap-3 border-b border-slate-100 p-4">
          <SearchBox value={search} onChange={(v) => { setSearch(v); setOffset(0); }} placeholder="Return or invoice number" className="w-72" />
          <DateRangePicker value={range} onChange={(r) => { setRange(r); setOffset(0); }} />
        </div>
        {error ? (
          <ErrorBox error={error} onRetry={refetch} />
        ) : isLoading ? (
          <Spinner />
        ) : !data?.items.length ? (
          <EmptyState icon={<Undo2 className="h-6 w-6" />} title="No returns in this period" />
        ) : (
          <>
            <table className="table">
              <thead>
                <tr>
                  <th>Return</th>
                  <th>Invoice</th>
                  <th>Date</th>
                  <th>Customer</th>
                  <th>Refund method</th>
                  <th>Reason</th>
                  <th className="text-right">Refund</th>
                  <th>By</th>
                </tr>
              </thead>
              <tbody>
                {data.items.map((r) => (
                  <tr key={r.id} className="clickable" onClick={() => setSaleId(r.saleId)}>
                    <td className="font-medium text-slate-900">{r.returnNo}</td>
                    <td>{r.invoiceNo}</td>
                    <td className="text-slate-600">{dateTime(r.createdAt)}</td>
                    <td>{r.customerName ?? '—'}</td>
                    <td>{methodLabel(r.refundMethod)}</td>
                    <td className="max-w-[220px] truncate text-slate-600">{r.reason ?? '—'}</td>
                    <td className="num text-right font-semibold">{money(r.totalRefund)}</td>
                    <td className="text-slate-600">{r.userName}</td>
                  </tr>
                ))}
              </tbody>
            </table>
            <Pager total={data.total} limit={LIMIT} offset={offset} onChange={setOffset} />
          </>
        )}
      </div>
      <SaleDetailDialog saleId={saleId} onClose={() => setSaleId(null)} />
    </div>
  );
}
