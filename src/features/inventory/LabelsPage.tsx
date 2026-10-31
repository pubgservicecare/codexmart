import { useState } from 'react';
import { Barcode as BarcodeIcon, Printer, Trash, TriangleAlert } from 'lucide-react';
import type { Product } from '@/lib/api';
import { money } from '@/lib/format';
import { usePrint, type PaperSize } from '@/lib/print';
import { useSettings } from '@/lib/session';
import { Button, EmptyState, Field, Input, PageHeader, Select, Switch } from '@/components/ui';
import { Barcode } from '@/components/Barcode';
import { ProductSearch } from '@/components/ProductSearch';

type LabelSize = '38x25' | '50x25' | 'a4-65';

const SIZES: Record<LabelSize, { label: string; w: string; h: string; paper: PaperSize }> = {
  '38x25': { label: '38 × 25 mm roll labels', w: '38mm', h: '25mm', paper: 'label-38x25' },
  '50x25': { label: '50 × 25 mm roll labels', w: '50mm', h: '25mm', paper: 'label-50x25' },
  'a4-65': { label: 'A4 sheet, 65 labels (38.1 × 21.2 mm)', w: '38.1mm', h: '21.2mm', paper: 'a4' },
};

interface Item {
  product: Product;
  copies: number;
}

function Label({ p, size, showPrice, storeName }: { p: Product; size: LabelSize; showPrice: boolean; storeName: string }) {
  const s = SIZES[size];
  const code = p.barcode ?? p.sku;
  return (
    <div
      style={{
        width: s.w,
        height: s.h,
        padding: '1mm 1.5mm',
        boxSizing: 'border-box',
        overflow: 'hidden',
        display: 'flex',
        flexDirection: 'column',
        alignItems: 'center',
        justifyContent: 'center',
        fontFamily: '"Inter Variable", Arial, sans-serif',
        color: '#000',
        background: '#fff',
        pageBreakAfter: size === 'a4-65' ? undefined : 'always',
        breakInside: 'avoid',
      }}
    >
      <div style={{ fontSize: 6.5, fontWeight: 700, whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis', maxWidth: '100%' }}>{storeName}</div>
      <div style={{ fontSize: 7, whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis', maxWidth: '100%' }}>{p.displayName}</div>
      <Barcode value={code} height={size === 'a4-65' ? 22 : 26} width={size === '50x25' ? 1.3 : 1} fontSize={8} />
      {showPrice && <div style={{ fontSize: 9, fontWeight: 800 }}>{money(p.salePrice)}</div>}
    </div>
  );
}

/** Prints barcode labels so products without a maker's barcode can be scanned. */
export function LabelsPage() {
  const settings = useSettings();
  const print = usePrint();
  const [items, setItems] = useState<Item[]>([]);
  const [size, setSize] = useState<LabelSize>(settings.labelSize);
  const [showPrice, setShowPrice] = useState(true);

  const add = (p: Product) =>
    setItems((list) => (list.some((i) => i.product.id === p.id) ? list : [...list, { product: p, copies: Math.max(1, Math.round(p.stockQty / 1000)) }]));
  const total = items.reduce((s, i) => s + i.copies, 0);
  const missingCode = items.filter((i) => !i.product.barcode);

  const labels = items.flatMap((i) => Array.from({ length: i.copies }, (_, n) => ({ key: `${i.product.id}-${n}`, p: i.product })));

  const doPrint = () =>
    print(
      size === 'a4-65' ? (
        <div style={{ display: 'grid', gridTemplateColumns: 'repeat(5, 38.1mm)', gap: '0 2.5mm', justifyContent: 'center' }}>
          {labels.map((l) => (
            <Label key={l.key} p={l.p} size={size} showPrice={showPrice} storeName={settings.storeName} />
          ))}
        </div>
      ) : (
        <div>
          {labels.map((l) => (
            <Label key={l.key} p={l.p} size={size} showPrice={showPrice} storeName={settings.storeName} />
          ))}
        </div>
      ),
      SIZES[size].paper,
    );

  return (
    <div>
      <PageHeader
        title="Barcode labels"
        description="Print price and barcode stickers. Products without a barcode use their SKU."
        icon={<BarcodeIcon className="h-5 w-5" />}
        actions={
          <Button variant="primary" icon={<Printer className="h-4 w-4" />} disabled={!total} onClick={doPrint}>
            Print {total} label(s)
          </Button>
        }
      />
      <div className="grid grid-cols-5 gap-5">
        <div className="card col-span-3 p-5">
          <ProductSearch onPick={add} autoFocus />
          {items.length === 0 ? (
            <EmptyState icon={<BarcodeIcon className="h-6 w-6" />} title="No products selected" description="Search or scan products above. Copies default to the quantity in stock." />
          ) : (
            <table className="table mt-4">
              <thead>
                <tr>
                  <th>Product</th>
                  <th>Code printed</th>
                  <th className="w-28">Copies</th>
                  <th />
                </tr>
              </thead>
              <tbody>
                {items.map((i, idx) => (
                  <tr key={i.product.id}>
                    <td className="font-medium text-slate-900">{i.product.displayName}</td>
                    <td className="font-mono text-xs">{i.product.barcode ?? `${i.product.sku} (SKU)`}</td>
                    <td>
                      <Input
                        type="number"
                        min={1}
                        max={500}
                        className="h-8 text-right"
                        value={i.copies}
                        onChange={(e) => setItems((l) => l.map((x, n) => (n === idx ? { ...x, copies: Math.min(500, Math.max(1, Number(e.target.value) || 1)) } : x)))}
                      />
                    </td>
                    <td>
                      <Button size="sm" variant="ghost" iconOnly aria-label="Remove" onClick={() => setItems((l) => l.filter((_, n) => n !== idx))}>
                        <Trash className="h-4 w-4" />
                      </Button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
          {missingCode.length > 0 && (
            <p className="mt-3 flex items-start gap-2 rounded-lg bg-amber-50 px-3 py-2 text-xs text-amber-900">
              <TriangleAlert className="mt-0.5 h-3.5 w-3.5 shrink-0" />
              {missingCode.length} product(s) have no barcode; their SKU is printed instead. Use “Generate” in the product form to give them an EAN-13 barcode.
            </p>
          )}
        </div>
        <div className="col-span-2 space-y-5">
          <div className="card space-y-4 p-5">
            <Field label="Label size">
              <Select value={size} onChange={(e) => setSize(e.target.value as LabelSize)}>
                {Object.entries(SIZES).map(([k, v]) => (
                  <option key={k} value={k}>
                    {v.label}
                  </option>
                ))}
              </Select>
            </Field>
            <Switch checked={showPrice} onChange={setShowPrice} label="Print the selling price" />
            <p className="text-xs text-slate-500">
              In the print dialog choose your label printer, set margins to “None” and scale to 100%.
            </p>
          </div>
          <div className="card p-5">
            <p className="mb-3 text-xs font-semibold uppercase text-slate-500">Preview</p>
            <div className="flex min-h-[120px] items-center justify-center rounded-lg bg-slate-100 p-4">
              {items[0] ? (
                <div className="shadow-card">
                  <Label p={items[0].product} size={size} showPrice={showPrice} storeName={settings.storeName} />
                </div>
              ) : (
                <span className="text-xs text-slate-400">Add a product to preview</span>
              )}
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
