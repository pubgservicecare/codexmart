import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { ClipboardList, PackagePlus, Printer, Trash } from 'lucide-react';
import { api, type Product, type PurchaseOrder } from '@/lib/api';
import { date, money, qty as fmtQty, today } from '@/lib/format';
import { usePrint } from '@/lib/print';
import { useSettings } from '@/lib/session';
import { Badge, Button, Field, Input, Modal, MoneyInput, QtyInput, Select, Textarea, useToast } from '@/components/ui';
import { ProductSearch } from '@/components/ProductSearch';

interface Line {
  productId: string;
  name: string;
  unitShort: string;
  allowDecimal: boolean;
  qty: number | null;
  unitCost: number | null;
}

export const PO_STATUS: Record<PurchaseOrder['status'], { label: string; tone: 'gray' | 'blue' | 'amber' | 'green' | 'red' }> = {
  draft: { label: 'Draft', tone: 'gray' },
  ordered: { label: 'Ordered', tone: 'blue' },
  partial: { label: 'Partly received', tone: 'amber' },
  received: { label: 'Received', tone: 'green' },
  cancelled: { label: 'Cancelled', tone: 'red' },
};

function PoPrint({ po, storeName }: { po: PurchaseOrder; storeName: string }) {
  const td = { borderBottom: '1px solid #ddd', padding: '6px 4px', fontSize: 12 } as const;
  return (
    <div style={{ fontFamily: '"Inter Variable", Arial, sans-serif', color: '#000' }}>
      <div style={{ display: 'flex', justifyContent: 'space-between' }}>
        <div style={{ fontSize: 20, fontWeight: 800 }}>{storeName}</div>
        <div style={{ textAlign: 'right' }}>
          <div style={{ fontSize: 18, fontWeight: 700 }}>PURCHASE ORDER</div>
          <div>{po.poNo}</div>
          <div>Date: {date(po.orderDate)}</div>
          {po.expectedDate && <div>Expected: {date(po.expectedDate)}</div>}
        </div>
      </div>
      <p style={{ marginTop: 16 }}>
        To: <b>{po.supplierName}</b>
      </p>
      <table style={{ width: '100%', borderCollapse: 'collapse', marginTop: 12 }}>
        <thead>
          <tr>
            <th style={{ ...td, textAlign: 'left' }}>#</th>
            <th style={{ ...td, textAlign: 'left' }}>Item</th>
            <th style={{ ...td, textAlign: 'right' }}>Qty</th>
            <th style={{ ...td, textAlign: 'right' }}>Unit cost</th>
            <th style={{ ...td, textAlign: 'right' }}>Amount</th>
          </tr>
        </thead>
        <tbody>
          {po.items.map((i) => (
            <tr key={i.id}>
              <td style={td}>{i.lineNo}</td>
              <td style={td}>
                {i.productName} <span style={{ color: '#666' }}>({i.sku})</span>
              </td>
              <td style={{ ...td, textAlign: 'right' }}>
                {fmtQty(i.qtyOrdered)} {i.unitShort}
              </td>
              <td style={{ ...td, textAlign: 'right' }}>{money(i.unitCost)}</td>
              <td style={{ ...td, textAlign: 'right' }}>{money(Math.round((i.unitCost * i.qtyOrdered) / 1000))}</td>
            </tr>
          ))}
        </tbody>
      </table>
      <p style={{ textAlign: 'right', marginTop: 10, fontWeight: 700 }}>Estimated total: {money(po.totalEstimate)}</p>
      {po.note && <p style={{ marginTop: 12 }}>Note: {po.note}</p>}
    </div>
  );
}

export function PurchaseOrderDialog({ po, onClose }: { po: PurchaseOrder | null; onClose: () => void }) {
  const toast = useToast();
  const qc = useQueryClient();
  const navigate = useNavigate();
  const print = usePrint();
  const settings = useSettings();
  const editable = !po || ((po.status === 'draft' || po.status === 'ordered') && po.items.every((i) => i.qtyReceived === 0));
  const [supplierId, setSupplierId] = useState(po?.supplierId ?? '');
  const [orderDate, setOrderDate] = useState(po?.orderDate ?? today());
  const [expected, setExpected] = useState(po?.expectedDate ?? '');
  const [note, setNote] = useState(po?.note ?? '');
  const [lines, setLines] = useState<Line[]>(
    po?.items.map((i) => ({ productId: i.productId, name: i.productName, unitShort: i.unitShort, allowDecimal: i.allowDecimal, qty: i.qtyOrdered, unitCost: i.unitCost })) ?? [],
  );
  const [busy, setBusy] = useState(false);
  const { data: suppliers } = useQuery({ queryKey: ['suppliers', 'active'], queryFn: () => api.suppliers.list({ limit: 1000 }) });
  const total = lines.reduce((s, l) => s + Math.round(((l.unitCost ?? 0) * (l.qty ?? 0)) / 1000), 0);
  const valid = supplierId && lines.length > 0 && lines.every((l) => (l.qty ?? 0) > 0 && l.unitCost !== null);

  const add = (p: Product) =>
    setLines((ls) =>
      ls.some((l) => l.productId === p.id) ? ls : [...ls, { productId: p.id, name: p.displayName, unitShort: p.unitShort, allowDecimal: p.allowDecimal, qty: null, unitCost: p.costPrice }],
    );
  const setLine = (i: number, patch: Partial<Line>) => setLines((ls) => ls.map((l, idx) => (idx === i ? { ...l, ...patch } : l)));
  const refresh = () => void qc.invalidateQueries({ queryKey: ['po'] });

  const save = async (status: 'draft' | 'ordered') => {
    setBusy(true);
    try {
      const input = {
        supplierId,
        orderDate,
        expectedDate: expected || null,
        note: note || null,
        status,
        lines: lines.map((l) => ({ productId: l.productId, qty: l.qty ?? 0, unitCost: l.unitCost ?? 0 })),
      };
      const saved = po ? await api.purchases.updateOrder(po.id, input) : await api.purchases.createOrder(input);
      refresh();
      toast.success(`Purchase order ${saved.poNo} saved`);
      onClose();
    } catch (e) {
      toast.error(e);
    } finally {
      setBusy(false);
    }
  };

  const action = async (a: 'ordered' | 'cancelled' | 'closed') => {
    if (!po) return;
    setBusy(true);
    try {
      await api.purchases.setOrderStatus(po.id, a);
      refresh();
      toast.success('Purchase order updated');
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
      icon={<ClipboardList className="h-5 w-5" />}
      title={
        po ? (
          <span className="flex items-center gap-2">
            {po.poNo} <Badge tone={PO_STATUS[po.status].tone}>{PO_STATUS[po.status].label}</Badge>
          </span>
        ) : (
          'New purchase order'
        )
      }
      description="A purchase order does not change stock. Stock is added when you receive the goods."
      footer={
        <>
          {po && (
            <Button variant="ghost" className="mr-auto" icon={<Printer className="h-4 w-4" />} onClick={() => print(<PoPrint po={po} storeName={settings.storeName} />, 'a4')}>
              Print
            </Button>
          )}
          {po && (po.status === 'draft' || po.status === 'ordered') && po.items.every((i) => i.qtyReceived === 0) && (
            <Button variant="ghost" className="text-rose-700" onClick={() => action('cancelled')} disabled={busy}>
              Cancel order
            </Button>
          )}
          {po?.status === 'partial' && (
            <Button onClick={() => action('closed')} disabled={busy}>
              Close (nothing more expected)
            </Button>
          )}
          {editable && (
            <Button onClick={() => save('draft')} loading={busy} disabled={!valid}>
              Save draft
            </Button>
          )}
          {editable && (
            <Button variant="primary" onClick={() => save('ordered')} loading={busy} disabled={!valid}>
              {po?.status === 'ordered' ? 'Save' : 'Save as ordered'}
            </Button>
          )}
          {po && ['draft', 'ordered', 'partial'].includes(po.status) && (
            <Button variant="success" icon={<PackagePlus className="h-4 w-4" />} onClick={() => navigate(`/purchases/receive?po=${po.id}`)}>
              Receive goods
            </Button>
          )}
        </>
      }
    >
      <div className="space-y-4">
        <div className="grid grid-cols-3 gap-4">
          <Field label="Supplier" required>
            <Select value={supplierId} disabled={!editable} onChange={(e) => setSupplierId(e.target.value)}>
              <option value="">Choose supplier…</option>
              {suppliers?.items.map((s) => (
                <option key={s.id} value={s.id}>
                  {s.name}
                </option>
              ))}
            </Select>
          </Field>
          <Field label="Order date">
            <Input type="date" value={orderDate} max={today()} disabled={!editable} onChange={(e) => setOrderDate(e.target.value)} />
          </Field>
          <Field label="Expected delivery">
            <Input type="date" value={expected} disabled={!editable} onChange={(e) => setExpected(e.target.value)} />
          </Field>
        </div>
        {editable && <ProductSearch onPick={add} showCost />}
        <div className="overflow-hidden rounded-xl border border-slate-200">
          <table className="table">
            <thead>
              <tr>
                <th>Product</th>
                <th className="w-32 text-right">Qty</th>
                {po && <th className="text-right">Received</th>}
                <th className="w-32 text-right">Unit cost</th>
                <th className="text-right">Amount</th>
                {editable && <th />}
              </tr>
            </thead>
            <tbody>
              {lines.map((l, i) => (
                <tr key={l.productId}>
                  <td className="font-medium text-slate-900">{l.name}</td>
                  <td>
                    {editable ? (
                      <QtyInput value={l.qty} allowDecimal={l.allowDecimal} onChange={(v) => setLine(i, { qty: v })} className="h-8" />
                    ) : (
                      <span className="num block text-right">
                        {fmtQty(l.qty ?? 0)} {l.unitShort}
                      </span>
                    )}
                  </td>
                  {po && <td className="num text-right">{fmtQty(po.items.find((x) => x.productId === l.productId)?.qtyReceived ?? 0)}</td>}
                  <td>{editable ? <MoneyInput value={l.unitCost} onChange={(v) => setLine(i, { unitCost: v })} className="h-8" /> : <span className="num block text-right">{money(l.unitCost)}</span>}</td>
                  <td className="num text-right">{money(Math.round(((l.unitCost ?? 0) * (l.qty ?? 0)) / 1000))}</td>
                  {editable && (
                    <td>
                      <Button size="sm" variant="ghost" iconOnly aria-label="Remove" onClick={() => setLines((ls) => ls.filter((_, idx) => idx !== i))}>
                        <Trash className="h-4 w-4" />
                      </Button>
                    </td>
                  )}
                </tr>
              ))}
              {lines.length === 0 && (
                <tr>
                  <td colSpan={6} className="py-8 text-center text-sm text-slate-500">
                    Add the products you want to order.
                  </td>
                </tr>
              )}
            </tbody>
            <tfoot>
              <tr>
                <td colSpan={po ? 4 : 3} className="text-right">
                  Estimated total
                </td>
                <td className="num text-right">{money(total)}</td>
                {editable && <td />}
              </tr>
            </tfoot>
          </table>
        </div>
        <Field label="Note">
          <Textarea rows={2} value={note} disabled={!editable} onChange={(e) => setNote(e.target.value)} />
        </Field>
      </div>
    </Modal>
  );
}
