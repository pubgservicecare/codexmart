import { useState } from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { Banknote, ClipboardList, Undo2 } from 'lucide-react';
import { api, type PurchaseDetail } from '@/lib/api';
import { date, dateTime, money, qty } from '@/lib/format';
import { newTxnId } from '@/lib/utils';
import { Button, Field, Input, KeyValue, Modal, QtyInput, Spinner, useToast } from '@/components/ui';
import { SupplierPaymentDialog } from './SupplierPaymentDialog';

export function PurchaseDetailDialog({ id, onClose }: { id: string; onClose: () => void }) {
  const { data: p, isLoading } = useQuery({ queryKey: ['purchases', 'detail', id], queryFn: () => api.purchases.get(id) });
  const [paying, setPaying] = useState(false);
  const [returning, setReturning] = useState(false);
  const canReturn = p?.items.some((i) => i.returnedQty < i.qty);

  return (
    <Modal
      open
      onClose={onClose}
      size="lg"
      icon={<ClipboardList className="h-5 w-5" />}
      title={p ? `${p.purchaseNo} · ${p.supplierName}` : 'Purchase'}
      description={p ? `Invoice ${p.supplierInvoiceNo ?? '—'} · ${date(p.businessDate)} · entered by ${p.userName}` : undefined}
      footer={
        p && (
          <>
            {canReturn && (
              <Button className="mr-auto" icon={<Undo2 className="h-4 w-4" />} onClick={() => setReturning(true)}>
                Return to supplier
              </Button>
            )}
            <Button variant="primary" icon={<Banknote className="h-4 w-4" />} onClick={() => setPaying(true)}>
              Pay supplier
            </Button>
          </>
        )
      }
    >
      {isLoading || !p ? (
        <Spinner />
      ) : (
        <div className="space-y-5">
          <div className="overflow-hidden rounded-xl border border-slate-200">
            <table className="table">
              <thead>
                <tr>
                  <th>Product</th>
                  <th className="text-right">Qty</th>
                  <th className="text-right">Unit cost</th>
                  <th className="text-right">Discount share</th>
                  <th className="text-right">Net cost</th>
                  <th>Batch</th>
                </tr>
              </thead>
              <tbody>
                {p.items.map((i) => (
                  <tr key={i.id}>
                    <td>
                      <p className="font-medium text-slate-900">{i.productName}</p>
                      {i.returnedQty > 0 && <p className="text-2xs font-semibold text-amber-700">Returned {qty(i.returnedQty)}</p>}
                    </td>
                    <td className="num text-right">
                      {qty(i.qty)} {i.unitShort}
                    </td>
                    <td className="num text-right">{money(i.unitCost)}</td>
                    <td className="num text-right">{i.discountShare ? `−${money(i.discountShare)}` : '—'}</td>
                    <td className="num text-right font-medium">{money(i.netCost)}</td>
                    <td className="text-xs text-slate-600">{i.batchNo || i.expiryDate ? `${i.batchNo ?? ''} ${i.expiryDate ? `exp ${date(i.expiryDate)}` : ''}` : '—'}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
          <div className="ml-auto w-72">
            <KeyValue label="Subtotal" value={money(p.subtotal)} />
            <KeyValue label="Discount" value={`−${money(p.discount)}`} />
            <KeyValue label="Total" value={money(p.total)} strong />
            <KeyValue label="Paid against this invoice" value={money(p.paidAmount)} />
            {p.returnedAmount > 0 && <KeyValue label="Returned" value={`−${money(p.returnedAmount)}`} />}
          </div>
          {p.note && <p className="text-sm text-slate-600">Note: {p.note}</p>}
          <p className="text-xs text-slate-400">Recorded {dateTime(p.createdAt)}</p>
        </div>
      )}
      {paying && p && <SupplierPaymentDialog supplierId={p.supplierId} purchaseId={p.id} onClose={() => setPaying(false)} />}
      {returning && p && <PurchaseReturnDialog purchase={p} onClose={() => setReturning(false)} />}
    </Modal>
  );
}

function PurchaseReturnDialog({ purchase, onClose }: { purchase: PurchaseDetail; onClose: () => void }) {
  const toast = useToast();
  const qc = useQueryClient();
  const [txnId] = useState(newTxnId);
  const [qtys, setQtys] = useState<Record<string, number | null>>({});
  const [reason, setReason] = useState('');
  const [busy, setBusy] = useState(false);
  const preview = purchase.items.reduce((s, i) => s + Math.round((i.netCost * (qtys[i.id] ?? 0)) / i.qty), 0);
  const anyQty = Object.values(qtys).some((q) => (q ?? 0) > 0);
  const over = purchase.items.some((i) => (qtys[i.id] ?? 0) > i.qty - i.returnedQty);

  const save = async () => {
    setBusy(true);
    try {
      const r = await api.purchases.createReturn({
        clientTxnId: txnId,
        purchaseId: purchase.id,
        reason: reason || null,
        lines: purchase.items.filter((i) => (qtys[i.id] ?? 0) > 0).map((i) => ({ purchaseItemId: i.id, qty: qtys[i.id] ?? 0 })),
      });
      ['purchases', 'products', 'suppliers', 'dashboard', 'inventory'].forEach((k) => void qc.invalidateQueries({ queryKey: [k] }));
      toast.success(`Return ${r.returnNo} saved`, `${money(r.total)} taken off what you owe ${r.supplierName}.`);
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
      size="lg"
      title={`Return goods from ${purchase.purchaseNo}`}
      description="Stock goes down and the supplier balance is reduced by the purchase cost."
      icon={<Undo2 className="h-5 w-5" />}
      footer={
        <>
          <span className="mr-auto text-sm text-slate-600">
            Credit from supplier: <b className="num text-slate-900">{money(preview)}</b>
          </span>
          <Button onClick={onClose}>Cancel</Button>
          <Button variant="primary" loading={busy} disabled={!anyQty || over} onClick={save}>
            Save return
          </Button>
        </>
      }
    >
      <div className="space-y-4">
        <table className="table">
          <thead>
            <tr>
              <th>Product</th>
              <th className="text-right">Bought</th>
              <th className="text-right">Can return</th>
              <th className="w-32 text-right">Return qty</th>
            </tr>
          </thead>
          <tbody>
            {purchase.items.map((i) => (
              <tr key={i.id}>
                <td className="font-medium text-slate-900">{i.productName}</td>
                <td className="num text-right">{qty(i.qty)}</td>
                <td className="num text-right">{qty(i.qty - i.returnedQty)}</td>
                <td>
                  <QtyInput
                    value={qtys[i.id] ?? null}
                    disabled={i.qty - i.returnedQty <= 0}
                    invalid={(qtys[i.id] ?? 0) > i.qty - i.returnedQty}
                    onChange={(v) => setQtys((s) => ({ ...s, [i.id]: v }))}
                    className="h-8"
                    placeholder="0"
                  />
                </td>
              </tr>
            ))}
          </tbody>
        </table>
        <Field label="Reason">
          <Input value={reason} onChange={(e) => setReason(e.target.value)} placeholder="e.g. Leaking, wrong item, near expiry" />
        </Field>
      </div>
    </Modal>
  );
}
