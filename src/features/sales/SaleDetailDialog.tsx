import { useState } from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { Ban, FileText, Printer, Receipt as ReceiptIcon, Undo2 } from 'lucide-react';
import { api } from '@/lib/api';
import { dateTime, methodLabel, money, qty, today } from '@/lib/format';
import { useSession } from '@/lib/session';
import { Badge, Button, ConfirmDialog, KeyValue, Modal, Spinner, useToast } from '@/components/ui';
import { useReceiptPrinter } from '@/components/useReceiptPrinter';
import { ReturnDialog } from './ReturnDialog';

export function SaleDetailDialog({ saleId, onClose }: { saleId: string | null; onClose: () => void }) {
  const { can } = useSession();
  const toast = useToast();
  const qc = useQueryClient();
  const printSale = useReceiptPrinter();
  const [voidOpen, setVoidOpen] = useState(false);
  const [returnOpen, setReturnOpen] = useState(false);
  const [busy, setBusy] = useState(false);
  const { data: sale, isLoading } = useQuery({
    queryKey: ['sales', 'detail', saleId],
    queryFn: () => api.sales.get(saleId!),
    enabled: !!saleId,
  });

  const refresh = () => {
    void qc.invalidateQueries({ queryKey: ['sales'] });
    void qc.invalidateQueries({ queryKey: ['returns'] });
    void qc.invalidateQueries({ queryKey: ['dashboard'] });
    void qc.invalidateQueries({ queryKey: ['products'] });
  };

  const doVoid = async (reason: string) => {
    if (!sale) return;
    setBusy(true);
    try {
      await api.sales.void(sale.id, reason);
      toast.success('Sale voided', `${sale.invoiceNo} — stock has been put back.`);
      setVoidOpen(false);
      refresh();
    } catch (e) {
      toast.error(e);
    } finally {
      setBusy(false);
    }
  };

  const canVoid = sale && sale.status === 'completed' && sale.businessDate === today() && sale.returns.length === 0 && can('voidSale');
  const fullyReturned = sale && sale.items.every((i) => i.returnedQty >= i.qty);
  const canReturn = sale && sale.status === 'completed' && !fullyReturned && can('processReturn');

  return (
    <Modal
      open={!!saleId}
      onClose={onClose}
      size="lg"
      icon={<ReceiptIcon className="h-5 w-5" />}
      title={
        sale ? (
          <span className="flex items-center gap-2">
            {sale.invoiceNo}
            {sale.status === 'void' ? <Badge tone="red">Void</Badge> : sale.returnedAmount > 0 ? <Badge tone="amber">Returns</Badge> : <Badge tone="green">Completed</Badge>}
          </span>
        ) : (
          'Sale'
        )
      }
      description={sale ? `${dateTime(sale.createdAt)} · Cashier ${sale.cashierName}` : undefined}
      footer={
        sale && (
          <>
            {canVoid && (
              <Button variant="ghost" className="mr-auto text-rose-700 hover:bg-rose-50" icon={<Ban className="h-4 w-4" />} onClick={() => setVoidOpen(true)}>
                Void sale
              </Button>
            )}
            {canReturn && (
              <Button icon={<Undo2 className="h-4 w-4" />} onClick={() => setReturnOpen(true)}>
                Return items
              </Button>
            )}
            <Button icon={<FileText className="h-4 w-4" />} onClick={() => void printSale(sale, { reprint: true, a4: true })}>
              A4 invoice
            </Button>
            <Button variant="primary" icon={<Printer className="h-4 w-4" />} onClick={() => void printSale(sale, { reprint: true })}>
              Reprint receipt
            </Button>
          </>
        )
      }
    >
      {isLoading || !sale ? (
        <Spinner />
      ) : (
        <div className="space-y-5">
          {sale.status === 'void' && (
            <div className="rounded-lg bg-rose-50 px-4 py-3 text-sm text-rose-800">
              Voided {dateTime(sale.voidedAt)} by {sale.voidedByName}: {sale.voidReason}
            </div>
          )}
          <div className="grid grid-cols-3 gap-3 text-sm">
            <div className="rounded-lg bg-slate-50 px-3 py-2">
              <p className="text-2xs uppercase text-slate-500">Customer</p>
              <p className="font-medium text-slate-900">{sale.customerName ?? 'Walk-in'}</p>
            </div>
            <div className="rounded-lg bg-slate-50 px-3 py-2">
              <p className="text-2xs uppercase text-slate-500">Paid</p>
              <p className="num font-medium text-slate-900">{money(sale.paidAmount)}</p>
            </div>
            <div className="rounded-lg bg-slate-50 px-3 py-2">
              <p className="text-2xs uppercase text-slate-500">On credit</p>
              <p className="num font-medium text-slate-900">{money(sale.creditAmount)}</p>
            </div>
          </div>
          <div className="overflow-hidden rounded-xl border border-slate-200">
            <table className="table">
              <thead>
                <tr>
                  <th>Item</th>
                  <th className="text-right">Qty</th>
                  <th className="text-right">Price</th>
                  <th className="text-right">Discount</th>
                  <th className="text-right">Amount</th>
                  {sale.cogsTotal !== null && <th className="text-right">Cost</th>}
                </tr>
              </thead>
              <tbody>
                {sale.items.map((i) => (
                  <tr key={i.id}>
                    <td>
                      <p className="font-medium text-slate-900">{i.productName}</p>
                      <p className="text-2xs text-slate-500">
                        {i.sku}
                        {i.returnedQty > 0 && <span className="ml-2 font-semibold text-amber-700">Returned {qty(i.returnedQty)}</span>}
                      </p>
                    </td>
                    <td className="num text-right">
                      {qty(i.qty)} {i.unitName}
                    </td>
                    <td className="num text-right">{money(i.unitPrice)}</td>
                    <td className="num text-right">{i.lineDiscount + i.billDiscountShare > 0 ? `−${money(i.lineDiscount + i.billDiscountShare)}` : '—'}</td>
                    <td className="num text-right font-medium">{money(i.netAmount)}</td>
                    {sale.cogsTotal !== null && <td className="num text-right text-slate-500">{money(i.costAmount)}</td>}
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
          <div className="grid grid-cols-2 gap-6">
            <div>
              <p className="mb-1 text-xs font-semibold uppercase text-slate-500">Payments</p>
              {sale.payments.length === 0 && <p className="text-sm text-slate-500">No payment (full credit)</p>}
              {sale.payments.map((p, idx) => (
                <KeyValue key={idx} label={`${methodLabel(p.method)}${p.reference ? ` · ${p.reference}` : ''}`} value={money(p.amount)} />
              ))}
              {sale.tenderedCash > 0 && (
                <>
                  <KeyValue label="Cash received" value={money(sale.tenderedCash)} />
                  <KeyValue label="Change given" value={money(sale.changeAmount)} />
                </>
              )}
            </div>
            <div>
              <KeyValue label="Subtotal" value={money(sale.subtotal)} />
              <KeyValue label="Discounts" value={`−${money(sale.discountTotal)}`} />
              <KeyValue label="Total" value={money(sale.total)} strong />
              {sale.returnedAmount > 0 && <KeyValue label="Refunded" value={`−${money(sale.returnedAmount)}`} />}
              {sale.cogsTotal !== null && <KeyValue label="Gross profit" value={money(sale.total - sale.cogsTotal)} />}
            </div>
          </div>
          {sale.returns.length > 0 && (
            <div>
              <p className="mb-1 text-xs font-semibold uppercase text-slate-500">Returns</p>
              {sale.returns.map((r) => (
                <KeyValue key={r.id} label={`${r.returnNo} · ${dateTime(r.createdAt)} · ${methodLabel(r.refundMethod)}`} value={money(r.totalRefund)} />
              ))}
            </div>
          )}
        </div>
      )}
      <ConfirmDialog
        open={voidOpen}
        onClose={() => setVoidOpen(false)}
        title={`Void ${sale?.invoiceNo}?`}
        message="The sale will be cancelled completely: items go back into stock and it is removed from all sales totals. Return the customer's money if they paid."
        confirmLabel="Void sale"
        danger
        requireReason
        busy={busy}
        onConfirm={doVoid}
      />
      {sale && returnOpen && (
        <ReturnDialog
          sale={sale}
          onClose={() => setReturnOpen(false)}
          onDone={() => {
            setReturnOpen(false);
            refresh();
          }}
        />
      )}
    </Modal>
  );
}
