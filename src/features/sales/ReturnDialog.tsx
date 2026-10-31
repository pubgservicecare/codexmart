import { useMemo, useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { Undo2 } from 'lucide-react';
import { api, type RefundMethod, type SaleDetail } from '@/lib/api';
import { methodLabel, money, qty } from '@/lib/format';
import { newTxnId } from '@/lib/utils';
import { Button, Checkbox, Field, Input, Modal, QtyInput, Select, Spinner, useToast } from '@/components/ui';

export function ReturnDialog({ sale, onClose, onDone }: { sale: SaleDetail; onClose: () => void; onDone: () => void }) {
  const toast = useToast();
  const [txnId] = useState(newTxnId);
  const { data: lines, isLoading } = useQuery({ queryKey: ['returnable', sale.id], queryFn: () => api.sales.returnable(sale.id) });
  const [qtys, setQtys] = useState<Record<string, number | null>>({});
  const [restock, setRestock] = useState<Record<string, boolean>>({});
  const [method, setMethod] = useState<RefundMethod>(sale.creditAmount > 0 && sale.customerId ? 'account' : 'cash');
  const [reason, setReason] = useState('');
  const [busy, setBusy] = useState(false);

  // Preview; the backend calculates the exact refund.
  const preview = useMemo(() => {
    if (!lines) return 0;
    return lines.reduce((sum, l) => {
      const q = qtys[l.saleItemId] ?? 0;
      if (q <= 0) return sum;
      return sum + (q >= l.qtyReturnable ? l.refundableAmount : Math.round((l.netAmount * q) / l.qtySold));
    }, 0);
  }, [lines, qtys]);

  const invalid = !lines || lines.some((l) => (qtys[l.saleItemId] ?? 0) > l.qtyReturnable) || preview === 0 && !Object.values(qtys).some((q) => (q ?? 0) > 0);

  const submit = async () => {
    if (!lines) return;
    setBusy(true);
    try {
      const r = await api.sales.createReturn({
        clientTxnId: txnId,
        saleId: sale.id,
        refundMethod: method,
        reason: reason || null,
        lines: lines
          .filter((l) => (qtys[l.saleItemId] ?? 0) > 0)
          .map((l) => ({ saleItemId: l.saleItemId, qty: qtys[l.saleItemId] ?? 0, restock: restock[l.saleItemId] ?? true })),
      });
      toast.success(`Return ${r.returnNo} saved`, `Refund ${money(r.totalRefund)} by ${methodLabel(r.refundMethod)}.`);
      onDone();
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
      title={`Return items from ${sale.invoiceNo}`}
      description="Enter how many of each item the customer is returning."
      icon={<Undo2 className="h-5 w-5" />}
      footer={
        <>
          <span className="mr-auto text-sm text-slate-600">
            Refund: <b className="num text-lg text-slate-900">{money(preview)}</b>
          </span>
          <Button onClick={onClose}>Cancel</Button>
          <Button variant="primary" loading={busy} disabled={invalid} onClick={submit}>
            Save return
          </Button>
        </>
      }
    >
      {isLoading || !lines ? (
        <Spinner />
      ) : (
        <div className="space-y-4">
          <div className="overflow-hidden rounded-xl border border-slate-200">
            <table className="table">
              <thead>
                <tr>
                  <th>Item</th>
                  <th className="text-right">Sold</th>
                  <th className="text-right">Can return</th>
                  <th className="w-28 text-right">Return qty</th>
                  <th>Back to stock</th>
                </tr>
              </thead>
              <tbody>
                {lines.map((l) => (
                  <tr key={l.saleItemId}>
                    <td>
                      <p className="font-medium text-slate-900">{l.productName}</p>
                      <p className="text-2xs text-slate-500">
                        {l.sku} · paid {money(l.netAmount)}
                      </p>
                    </td>
                    <td className="num text-right">{qty(l.qtySold)}</td>
                    <td className="num text-right">{qty(l.qtyReturnable)}</td>
                    <td>
                      <QtyInput
                        value={qtys[l.saleItemId] ?? null}
                        allowDecimal={l.allowDecimal}
                        disabled={l.qtyReturnable <= 0}
                        invalid={(qtys[l.saleItemId] ?? 0) > l.qtyReturnable}
                        onChange={(v) => setQtys((s) => ({ ...s, [l.saleItemId]: v }))}
                        placeholder="0"
                      />
                    </td>
                    <td>
                      <Checkbox
                        checked={restock[l.saleItemId] ?? true}
                        onChange={(v) => setRestock((s) => ({ ...s, [l.saleItemId]: v }))}
                        label={restock[l.saleItemId] ?? true ? 'Yes' : 'Damaged'}
                      />
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
          <div className="grid grid-cols-2 gap-4">
            <Field label="Refund method">
              <Select value={method} onChange={(e) => setMethod(e.target.value as RefundMethod)}>
                <option value="cash">Cash</option>
                <option value="card">Card</option>
                <option value="bank">Bank transfer</option>
                <option value="wallet">Mobile wallet</option>
                {sale.customerId && <option value="account">Credit to customer account</option>}
              </Select>
            </Field>
            <Field label="Reason">
              <Input value={reason} onChange={(e) => setReason(e.target.value)} placeholder="e.g. Wrong shade, damaged" />
            </Field>
          </div>
          <p className="text-xs text-slate-500">
            Items marked “Damaged” are not put back on the shelf; their cost stays as a loss in the profit report.
          </p>
        </div>
      )}
    </Modal>
  );
}
