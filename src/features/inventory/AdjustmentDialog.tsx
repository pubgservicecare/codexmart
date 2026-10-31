import { useMemo, useState } from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { ClipboardList, Trash } from 'lucide-react';
import { api, type AdjustReason, type Batch, type Product } from '@/lib/api';
import { qty as fmtQty } from '@/lib/format';
import { useSession } from '@/lib/session';
import { newTxnId } from '@/lib/utils';
import { Button, Field, Input, Modal, MoneyInput, QtyInput, Select, Textarea, useToast } from '@/components/ui';
import { ProductSearch } from '@/components/ProductSearch';

export const REASONS: { value: AdjustReason; label: string; mode: 'add' | 'remove' | 'set' | 'either'; hint: string }[] = [
  { value: 'stock_count', label: 'Stock count (set counted quantity)', mode: 'set', hint: 'Enter what you physically counted; the difference is recorded.' },
  { value: 'opening_stock', label: 'Opening stock', mode: 'add', hint: 'Stock you already had before using CODE X MART. Not counted as profit.' },
  { value: 'found', label: 'Found / extra stock', mode: 'add', hint: 'Stock found that was not recorded.' },
  { value: 'damaged', label: 'Damaged', mode: 'remove', hint: 'Broken or unsellable items. Recorded as a loss.' },
  { value: 'expired', label: 'Expired', mode: 'remove', hint: 'Items past their expiry date. Recorded as a loss.' },
  { value: 'lost', label: 'Lost', mode: 'remove', hint: 'Missing items. Recorded as a loss.' },
  { value: 'theft', label: 'Theft', mode: 'remove', hint: 'Stolen items. Recorded as a loss.' },
  { value: 'internal_use', label: 'Used in the shop', mode: 'remove', hint: 'Items taken for shop use.' },
  { value: 'other', label: 'Other correction', mode: 'either', hint: 'Any other correction; explain it in the note.' },
];

interface Line {
  product: Product;
  mode: 'add' | 'remove' | 'set';
  qty: number | null;
  unitCost: number | null;
  batchId: string;
  batchNo: string;
  expiryDate: string;
}

function BatchSelect({ productId, value, onChange }: { productId: string; value: string; onChange: (v: string) => void }) {
  const { data } = useQuery({
    queryKey: ['batches', productId, 'available'],
    queryFn: () => api.inventory.batches({ productId, onlyAvailable: true }),
  });
  return (
    <Select value={value} onChange={(e) => onChange(e.target.value)} className="h-8 text-xs">
      <option value="">Automatic (earliest expiry first)</option>
      {data?.map((b: Batch) => (
        <option key={b.id} value={b.id}>
          {b.batchNo ?? 'No batch no.'} · exp {b.expiryDate ?? '—'} · {fmtQty(b.qtyRemaining)} left
        </option>
      ))}
    </Select>
  );
}

export function AdjustmentDialog({ onClose, initialProduct }: { onClose: () => void; initialProduct?: Product }) {
  const { can } = useSession();
  const toast = useToast();
  const qc = useQueryClient();
  const [txnId] = useState(newTxnId);
  const [reason, setReason] = useState<AdjustReason>('stock_count');
  const [note, setNote] = useState('');
  const [busy, setBusy] = useState(false);
  const reasonInfo = REASONS.find((r) => r.value === reason)!;
  const defaultMode = (): Line['mode'] => (reasonInfo.mode === 'either' ? 'add' : reasonInfo.mode);
  const [lines, setLines] = useState<Line[]>(() =>
    initialProduct ? [{ product: initialProduct, mode: 'set', qty: null, unitCost: null, batchId: '', batchNo: '', expiryDate: '' }] : [],
  );

  const changeReason = (r: AdjustReason) => {
    setReason(r);
    const info = REASONS.find((x) => x.value === r)!;
    setLines((ls) => ls.map((l) => ({ ...l, mode: info.mode === 'either' ? l.mode === 'set' ? 'add' : l.mode : info.mode })));
  };

  const add = (p: Product) => {
    if (lines.some((l) => l.product.id === p.id)) return;
    setLines((ls) => [...ls, { product: p, mode: defaultMode(), qty: null, unitCost: null, batchId: '', batchNo: '', expiryDate: '' }]);
  };
  const setLine = (i: number, patch: Partial<Line>) => setLines((ls) => ls.map((l, idx) => (idx === i ? { ...l, ...patch } : l)));

  const valid = useMemo(() => lines.length > 0 && lines.every((l) => l.qty !== null && (l.mode === 'set' ? l.qty >= 0 : l.qty > 0)), [lines]);

  const save = async () => {
    setBusy(true);
    try {
      const adj = await api.inventory.adjust({
        clientTxnId: txnId,
        reason,
        note: note || null,
        lines: lines.map((l) => ({
          productId: l.product.id,
          mode: l.mode,
          qty: l.qty ?? 0,
          unitCost: l.mode !== 'remove' ? l.unitCost : null,
          batchId: l.batchId || null,
          batchNo: l.batchNo || null,
          expiryDate: l.expiryDate || null,
        })),
      });
      void qc.invalidateQueries({ queryKey: ['products'] });
      void qc.invalidateQueries({ queryKey: ['inventory'] });
      void qc.invalidateQueries({ queryKey: ['batches'] });
      void qc.invalidateQueries({ queryKey: ['dashboard'] });
      toast.success(`Adjustment ${adj.adjustmentNo} saved`, `${adj.lines.length} product(s) updated`);
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
      size="xl"
      closeOnBackdrop={false}
      title="Stock adjustment"
      description="Every change is recorded permanently in the stock history."
      icon={<ClipboardList className="h-5 w-5" />}
      footer={
        <>
          <Button onClick={onClose}>Cancel</Button>
          <Button variant="primary" loading={busy} disabled={!valid} onClick={save}>
            Save adjustment
          </Button>
        </>
      }
    >
      <div className="space-y-4">
        <div className="grid grid-cols-2 gap-4">
          <Field label="Reason" hint={reasonInfo.hint}>
            <Select value={reason} onChange={(e) => changeReason(e.target.value as AdjustReason)}>
              {REASONS.map((r) => (
                <option key={r.value} value={r.value}>
                  {r.label}
                </option>
              ))}
            </Select>
          </Field>
          <Field label="Add product">
            <ProductSearch onPick={add} showCost={can('viewCosts')} autoFocus={!initialProduct} />
          </Field>
        </div>
        <div className="overflow-hidden rounded-xl border border-slate-200">
          <table className="table">
            <thead>
              <tr>
                <th>Product</th>
                <th className="text-right">In stock</th>
                {reasonInfo.mode === 'either' && <th>Change</th>}
                <th className="w-32 text-right">{reasonInfo.mode === 'set' ? 'Counted qty' : 'Quantity'}</th>
                {reasonInfo.mode !== 'remove' && reasonInfo.mode !== 'set' && can('viewCosts') && <th className="w-32 text-right">Unit cost</th>}
                <th>Batch</th>
                <th />
              </tr>
            </thead>
            <tbody>
              {lines.length === 0 && (
                <tr>
                  <td colSpan={7} className="py-8 text-center text-sm text-slate-500">
                    Scan or search products above to add them.
                  </td>
                </tr>
              )}
              {lines.map((l, i) => (
                <tr key={l.product.id}>
                  <td>
                    <p className="font-medium text-slate-900">{l.product.displayName}</p>
                    <p className="text-2xs text-slate-500">{l.product.sku}</p>
                  </td>
                  <td className="num text-right">
                    {fmtQty(l.product.stockQty)} {l.product.unitShort}
                  </td>
                  {reasonInfo.mode === 'either' && (
                    <td>
                      <Select className="h-8 w-28 text-xs" value={l.mode} onChange={(e) => setLine(i, { mode: e.target.value as Line['mode'] })}>
                        <option value="add">Add</option>
                        <option value="remove">Remove</option>
                      </Select>
                    </td>
                  )}
                  <td>
                    <QtyInput value={l.qty} allowDecimal={l.product.allowDecimal} onChange={(v) => setLine(i, { qty: v })} className="h-8" placeholder="0" />
                    {l.mode === 'set' && l.qty !== null && (
                      <p className="mt-0.5 text-right text-2xs text-slate-500">
                        Change: {l.qty - l.product.stockQty > 0 ? '+' : ''}
                        {fmtQty(l.qty - l.product.stockQty)}
                      </p>
                    )}
                  </td>
                  {reasonInfo.mode !== 'remove' && reasonInfo.mode !== 'set' && can('viewCosts') && (
                    <td>
                      {l.mode === 'add' && (
                        <MoneyInput value={l.unitCost ?? l.product.costPrice} onChange={(v) => setLine(i, { unitCost: v })} className="h-8" />
                      )}
                    </td>
                  )}
                  <td className="min-w-[200px]">
                    {l.product.trackBatches ? (
                      l.mode === 'remove' ? (
                        <BatchSelect productId={l.product.id} value={l.batchId} onChange={(v) => setLine(i, { batchId: v })} />
                      ) : l.mode === 'add' ? (
                        <div className="flex gap-1">
                          <Input className="h-8 text-xs" placeholder="Batch no." value={l.batchNo} onChange={(e) => setLine(i, { batchNo: e.target.value })} />
                          <Input type="date" className="h-8 text-xs" value={l.expiryDate} onChange={(e) => setLine(i, { expiryDate: e.target.value })} />
                        </div>
                      ) : (
                        <span className="text-2xs text-slate-400">Automatic</span>
                      )
                    ) : (
                      <span className="text-2xs text-slate-400">Not tracked</span>
                    )}
                  </td>
                  <td>
                    <Button size="sm" variant="ghost" iconOnly aria-label="Remove line" onClick={() => setLines((ls) => ls.filter((_, idx) => idx !== i))}>
                      <Trash className="h-4 w-4" />
                    </Button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
        <Field label="Note">
          <Textarea rows={2} value={note} onChange={(e) => setNote(e.target.value)} placeholder="Optional explanation" />
        </Field>
      </div>
    </Modal>
  );
}
