import { useEffect, useMemo, useState } from 'react';
import { useNavigate, useSearchParams } from 'react-router-dom';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { ArrowLeft, PackagePlus, Trash } from 'lucide-react';
import { api, type PaymentMethod, type Product } from '@/lib/api';
import { money, qty as fmtQty, today } from '@/lib/format';
import { newTxnId } from '@/lib/utils';
import { Button, Field, Input, KeyValue, MoneyInput, PageHeader, QtyInput, Select, Spinner, Textarea, useToast } from '@/components/ui';
import { ProductSearch } from '@/components/ProductSearch';

interface Line {
  productId: string;
  name: string;
  sku: string;
  unitShort: string;
  allowDecimal: boolean;
  trackBatches: boolean;
  currentPrice: number;
  qty: number | null;
  unitCost: number | null;
  batchNo: string;
  expiryDate: string;
  poItemId: string | null;
  newSalePrice: number | null;
}

function lineFromProduct(p: Product): Line {
  return {
    productId: p.id,
    name: p.displayName,
    sku: p.sku,
    unitShort: p.unitShort,
    allowDecimal: p.allowDecimal,
    trackBatches: p.trackBatches,
    currentPrice: p.salePrice,
    qty: null,
    unitCost: p.costPrice,
    batchNo: '',
    expiryDate: '',
    poItemId: null,
    newSalePrice: null,
  };
}

/** Records a supplier invoice: stock comes in at its real cost. */
export function ReceiveGoodsPage() {
  const toast = useToast();
  const qc = useQueryClient();
  const navigate = useNavigate();
  const [params] = useSearchParams();
  const poId = params.get('po');
  const [txnId] = useState(newTxnId);
  const [supplierId, setSupplierId] = useState('');
  const [invoiceNo, setInvoiceNo] = useState('');
  const [day, setDay] = useState(today());
  const [lines, setLines] = useState<Line[]>([]);
  const [discount, setDiscount] = useState<number | null>(null);
  const [payNow, setPayNow] = useState<number | null>(null);
  const [payMethod, setPayMethod] = useState<PaymentMethod>('cash');
  const [note, setNote] = useState('');
  const [busy, setBusy] = useState(false);

  const { data: suppliers } = useQuery({ queryKey: ['suppliers', 'active'], queryFn: () => api.suppliers.list({ limit: 1000 }) });
  const po = useQuery({ queryKey: ['po', poId], queryFn: () => api.purchases.getOrder(poId!), enabled: !!poId });

  useEffect(() => {
    if (!po.data) return;
    setSupplierId(po.data.supplierId);
    void Promise.all(
      po.data.items
        .filter((i) => i.qtyReceived < i.qtyOrdered)
        .map(async (i) => {
          const p = await api.products.get(i.productId);
          return { ...lineFromProduct(p), qty: i.qtyOrdered - i.qtyReceived, unitCost: i.unitCost, poItemId: i.id };
        }),
    ).then(setLines);
  }, [po.data]);

  const totals = useMemo(() => {
    const subtotal = lines.reduce((s, l) => s + Math.round(((l.unitCost ?? 0) * (l.qty ?? 0)) / 1000), 0);
    return { subtotal, total: subtotal - (discount ?? 0) };
  }, [lines, discount]);

  const setLine = (i: number, patch: Partial<Line>) => setLines((ls) => ls.map((l, idx) => (idx === i ? { ...l, ...patch } : l)));
  const add = (p: Product) => setLines((ls) => (ls.some((l) => l.productId === p.id && !l.poItemId) ? ls : [...ls, lineFromProduct(p)]));
  const valid = supplierId && lines.length > 0 && lines.every((l) => (l.qty ?? 0) > 0 && l.unitCost !== null) && totals.total >= 0 && (payNow ?? 0) <= totals.total;

  const save = async () => {
    setBusy(true);
    try {
      const doc = await api.purchases.create({
        clientTxnId: txnId,
        supplierId,
        supplierInvoiceNo: invoiceNo || null,
        poId: poId ?? null,
        businessDate: day,
        discount: discount ?? 0,
        note: note || null,
        lines: lines.map((l) => ({
          productId: l.productId,
          qty: l.qty ?? 0,
          unitCost: l.unitCost ?? 0,
          batchNo: l.batchNo || null,
          expiryDate: l.expiryDate || null,
          poItemId: l.poItemId,
          newSalePrice: l.newSalePrice,
        })),
        payment: payNow ? { amount: payNow, method: payMethod } : null,
      });
      ['purchases', 'products', 'suppliers', 'dashboard', 'inventory', 'batches', 'po'].forEach((k) => void qc.invalidateQueries({ queryKey: [k] }));
      toast.success(`Purchase ${doc.purchaseNo} saved`, `${doc.items.length} product(s) added to stock · ${money(doc.total)}`);
      navigate('/purchases');
    } catch (e) {
      toast.error(e);
    } finally {
      setBusy(false);
    }
  };

  if (poId && po.isLoading) return <Spinner />;

  return (
    <div>
      <PageHeader
        title={po.data ? `Receive goods for ${po.data.poNo}` : 'Receive goods'}
        description="Enter the supplier's invoice. Stock and average cost update when you save."
        icon={<PackagePlus className="h-5 w-5" />}
        actions={
          <Button variant="ghost" icon={<ArrowLeft className="h-4 w-4" />} onClick={() => navigate('/purchases')}>
            Back to purchases
          </Button>
        }
      />
      <div className="grid grid-cols-4 gap-5">
        <div className="col-span-3 space-y-5">
          <div className="card grid grid-cols-3 gap-4 p-5">
            <Field label="Supplier" required>
              <Select value={supplierId} onChange={(e) => setSupplierId(e.target.value)} disabled={!!po.data}>
                <option value="">Choose supplier…</option>
                {suppliers?.items.map((s) => (
                  <option key={s.id} value={s.id}>
                    {s.name}
                  </option>
                ))}
              </Select>
            </Field>
            <Field label="Supplier invoice no." hint="Prevents entering the same invoice twice.">
              <Input value={invoiceNo} onChange={(e) => setInvoiceNo(e.target.value)} />
            </Field>
            <Field label="Invoice date">
              <Input type="date" value={day} max={today()} onChange={(e) => setDay(e.target.value)} />
            </Field>
          </div>

          <div className="card">
            <div className="border-b border-slate-100 p-4">
              <ProductSearch onPick={add} showCost autoFocus={!poId} placeholder="Scan or search products on the invoice…" />
            </div>
            <div className="overflow-x-auto">
              <table className="table">
                <thead>
                  <tr>
                    <th>Product</th>
                    <th className="w-28 text-right">Qty</th>
                    <th className="w-32 text-right">Unit cost</th>
                    <th className="text-right">Line total</th>
                    <th>Batch / expiry</th>
                    <th className="w-32 text-right">New sale price</th>
                    <th />
                  </tr>
                </thead>
                <tbody>
                  {lines.length === 0 && (
                    <tr>
                      <td colSpan={7} className="py-10 text-center text-sm text-slate-500">
                        Add the products listed on the supplier's invoice.
                      </td>
                    </tr>
                  )}
                  {lines.map((l, i) => (
                    <tr key={`${l.productId}-${i}`}>
                      <td>
                        <p className="font-medium text-slate-900">{l.name}</p>
                        <p className="text-2xs text-slate-500">
                          {l.sku} · sells at {money(l.currentPrice)}
                        </p>
                      </td>
                      <td>
                        <QtyInput value={l.qty} allowDecimal={l.allowDecimal} onChange={(v) => setLine(i, { qty: v })} className="h-8" placeholder={l.unitShort} />
                      </td>
                      <td>
                        <MoneyInput value={l.unitCost} onChange={(v) => setLine(i, { unitCost: v })} className="h-8" />
                      </td>
                      <td className="num text-right font-medium">{money(Math.round(((l.unitCost ?? 0) * (l.qty ?? 0)) / 1000))}</td>
                      <td>
                        {l.trackBatches ? (
                          <div className="flex gap-1">
                            <Input className="h-8 w-24 text-xs" placeholder="Batch" value={l.batchNo} onChange={(e) => setLine(i, { batchNo: e.target.value })} />
                            <Input type="date" className="h-8 w-36 text-xs" value={l.expiryDate} onChange={(e) => setLine(i, { expiryDate: e.target.value })} />
                          </div>
                        ) : (
                          <span className="text-2xs text-slate-400">—</span>
                        )}
                      </td>
                      <td>
                        <MoneyInput value={l.newSalePrice} onChange={(v) => setLine(i, { newSalePrice: v })} className="h-8" placeholder="Keep" />
                      </td>
                      <td>
                        <Button size="sm" variant="ghost" iconOnly aria-label="Remove" onClick={() => setLines((ls) => ls.filter((_, idx) => idx !== i))}>
                          <Trash className="h-4 w-4" />
                        </Button>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </div>
          <Field label="Note">
            <Textarea rows={2} value={note} onChange={(e) => setNote(e.target.value)} />
          </Field>
        </div>

        <div className="space-y-4">
          <div className="card p-5">
            <p className="mb-2 text-xs font-semibold uppercase text-slate-500">Invoice total</p>
            <KeyValue label="Items" value={`${lines.length} · ${fmtQty(lines.reduce((s, l) => s + (l.qty ?? 0), 0))} units`} />
            <KeyValue label="Subtotal" value={money(totals.subtotal)} />
            <Field label="Invoice discount" className="mt-2" hint="Spread over the items' cost automatically.">
              <MoneyInput value={discount} onChange={setDiscount} invalid={(discount ?? 0) > totals.subtotal} />
            </Field>
            <div className="mt-3 border-t border-slate-200 pt-3">
              <KeyValue label="Total payable" value={<span className="text-xl font-semibold">{money(totals.total)}</span>} />
            </div>
          </div>
          <div className="card space-y-3 p-5">
            <p className="text-xs font-semibold uppercase text-slate-500">Paid now (optional)</p>
            <MoneyInput value={payNow} onChange={setPayNow} invalid={(payNow ?? 0) > totals.total} placeholder="0" />
            <Select value={payMethod} onChange={(e) => setPayMethod(e.target.value as PaymentMethod)}>
              <option value="cash">Cash</option>
              <option value="bank">Bank transfer</option>
              <option value="cheque">Cheque</option>
              <option value="wallet">Mobile wallet</option>
            </Select>
            <p className="text-xs text-slate-500">The unpaid part is added to what you owe this supplier.</p>
          </div>
          <Button variant="primary" size="lg" className="w-full" loading={busy} disabled={!valid} onClick={save}>
            Save & add to stock
          </Button>
        </div>
      </div>
    </div>
  );
}
