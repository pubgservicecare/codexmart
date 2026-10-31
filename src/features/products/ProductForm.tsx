import { useState } from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { Barcode as BarcodeIcon, Package, Wand2 } from 'lucide-react';
import { api, type Product, type ProductInput } from '@/lib/api';
import { money, percent } from '@/lib/format';
import { useSession, useSettings } from '@/lib/session';
import { Button, Field, Input, Modal, MoneyInput, QtyInput, Select, Switch, Textarea, useToast } from '@/components/ui';
import { Barcode } from '@/components/Barcode';
import { LookupSelect } from './LookupSelect';

export type ProductFormMode = { kind: 'new' } | { kind: 'edit'; product: Product } | { kind: 'variant'; of: Product };

function initial(mode: ProductFormMode): ProductInput & { openingQty: number | null; openingCost: number | null; batchNo: string; expiryDate: string } {
  const base = mode.kind === 'edit' ? mode.product : mode.kind === 'variant' ? mode.of : null;
  return {
    name: base?.name ?? '',
    sku: mode.kind === 'edit' ? base!.sku : '',
    barcode: mode.kind === 'edit' ? (base!.barcode ?? '') : '',
    categoryId: base?.categoryId ?? null,
    brandId: base?.brandId ?? null,
    unitId: base?.unitId ?? '00000000-0000-7000-8000-000000000101',
    variantGroupId: mode.kind === 'variant' ? (mode.of.variantGroupId ?? mode.of.id) : (base?.variantGroupId ?? null),
    size: mode.kind === 'edit' ? (base!.size ?? '') : '',
    color: mode.kind === 'edit' ? (base!.color ?? '') : '',
    description: base?.description ?? '',
    salePrice: base?.salePrice ?? 0,
    wholesalePrice: base?.wholesalePrice ?? null,
    costPrice: base?.costPrice ?? null,
    reorderLevel: base?.reorderLevel ?? 0,
    trackBatches: base?.trackBatches ?? false,
    isActive: base?.isActive ?? true,
    openingQty: null,
    openingCost: null,
    batchNo: '',
    expiryDate: '',
  };
}

export function ProductForm({ mode, onClose, onSaved }: { mode: ProductFormMode; onClose: () => void; onSaved?: (p: Product) => void }) {
  const settings = useSettings();
  const { can } = useSession();
  const toast = useToast();
  const qc = useQueryClient();
  const [form, setForm] = useState(() => initial(mode));
  const [busy, setBusy] = useState(false);
  const { data: units } = useQuery({ queryKey: ['units'], queryFn: () => api.units.list() });
  const set = (patch: Partial<typeof form>) => setForm((f) => ({ ...f, ...patch }));

  const isEdit = mode.kind === 'edit';
  const product = isEdit ? mode.product : null;
  const unit = units?.find((u) => u.id === form.unitId);
  const allowDecimal = unit?.allowDecimal ?? false;
  const costEditable = !isEdit || (product?.stockQty ?? 0) <= 0;
  const cost = isEdit ? form.costPrice : (form.openingCost ?? form.costPrice);
  const margin = cost && form.salePrice > 0 ? Math.round(((form.salePrice - cost) * 10000) / form.salePrice) : null;

  const generate = async () => {
    try {
      set({ barcode: await api.products.generateBarcode() });
    } catch (e) {
      toast.error(e);
    }
  };

  const save = async () => {
    setBusy(true);
    const input: ProductInput = {
      name: form.name,
      sku: form.sku || null,
      barcode: form.barcode || null,
      categoryId: form.categoryId,
      brandId: form.brandId,
      unitId: form.unitId,
      variantGroupId: form.variantGroupId,
      size: form.size || null,
      color: form.color || null,
      description: form.description || null,
      salePrice: form.salePrice,
      wholesalePrice: settings.enableWholesale ? form.wholesalePrice : (product?.wholesalePrice ?? null),
      costPrice: isEdit ? (costEditable ? form.costPrice : product?.costPrice) : (form.openingCost ?? form.costPrice),
      reorderLevel: form.reorderLevel,
      trackBatches: form.trackBatches,
      isActive: form.isActive,
      openingStock:
        !isEdit && form.openingQty
          ? { qty: form.openingQty, unitCost: form.openingCost ?? form.costPrice ?? 0, batchNo: form.batchNo || null, expiryDate: form.expiryDate || null }
          : null,
    };
    try {
      const saved = isEdit ? await api.products.update(product!.id, input) : await api.products.create(input);
      void qc.invalidateQueries({ queryKey: ['products'] });
      void qc.invalidateQueries({ queryKey: ['pos-browse'] });
      void qc.invalidateQueries({ queryKey: ['dashboard'] });
      toast.success(isEdit ? 'Product updated' : 'Product added', `${saved.displayName} · ${saved.sku}`);
      onSaved?.(saved);
      onClose();
    } catch (e) {
      toast.error(e);
    } finally {
      setBusy(false);
    }
  };

  const title = isEdit ? `Edit ${product!.displayName}` : mode.kind === 'variant' ? `New variant of ${mode.of.name}` : 'New product';

  return (
    <Modal
      open
      onClose={onClose}
      size="lg"
      title={title}
      icon={<Package className="h-5 w-5" />}
      closeOnBackdrop={false}
      footer={
        <>
          <Button onClick={onClose}>Cancel</Button>
          <Button variant="primary" loading={busy} disabled={!form.name.trim() || !form.unitId} onClick={save}>
            {isEdit ? 'Save changes' : 'Add product'}
          </Button>
        </>
      }
    >
      <form
        onSubmit={(e) => {
          e.preventDefault();
          void save();
        }}
        className="space-y-6"
      >
        <section className="grid grid-cols-6 gap-4">
          <Field label="Product name" required className="col-span-6">
            <Input autoFocus value={form.name} onChange={(e) => set({ name: e.target.value })} placeholder="e.g. Fair & Lovely Cream 50g" />
          </Field>
          <Field label="Category" className="col-span-2">
            <LookupSelect kind="category" value={form.categoryId ?? null} onChange={(v) => set({ categoryId: v })} canAdd={can('manageCatalog')} />
          </Field>
          <Field label="Brand" className="col-span-2">
            <LookupSelect kind="brand" value={form.brandId ?? null} onChange={(v) => set({ brandId: v })} canAdd={can('manageCatalog')} />
          </Field>
          <Field label="Unit" required className="col-span-2">
            <Select value={form.unitId} onChange={(e) => set({ unitId: e.target.value })}>
              {units?.map((u) => (
                <option key={u.id} value={u.id}>
                  {u.name} ({u.shortName}){u.allowDecimal ? ' · decimals' : ''}
                </option>
              ))}
            </Select>
          </Field>
          {settings.enableVariants && (
            <>
              <Field label="Size" className="col-span-3" hint="Optional — e.g. 50ml, Large, 38">
                <Input value={form.size ?? ''} onChange={(e) => set({ size: e.target.value })} />
              </Field>
              <Field label="Colour / shade" className="col-span-3" hint="Optional — e.g. Ruby Red, Black">
                <Input value={form.color ?? ''} onChange={(e) => set({ color: e.target.value })} />
              </Field>
            </>
          )}
        </section>

        <section className="grid grid-cols-6 gap-4 rounded-xl bg-slate-50 p-4">
          <Field label="SKU" className="col-span-2" hint={isEdit ? undefined : 'Leave empty to number automatically.'}>
            <Input value={form.sku ?? ''} onChange={(e) => set({ sku: e.target.value.toUpperCase() })} placeholder="Auto" />
          </Field>
          <Field label="Barcode" className="col-span-4" hint="Scan the product's barcode, type it, or generate an in-store code.">
            <div className="flex gap-2">
              <Input value={form.barcode ?? ''} onChange={(e) => set({ barcode: e.target.value.trim() })} placeholder="Scan or type" />
              <Button icon={<Wand2 className="h-4 w-4" />} onClick={generate} title="Generate an in-store EAN-13 barcode">
                Generate
              </Button>
            </div>
          </Field>
          {form.barcode && (
            <div className="col-span-6 flex items-center gap-3 rounded-lg bg-white px-3 py-2">
              <BarcodeIcon className="h-4 w-4 text-slate-400" />
              <Barcode value={form.barcode} height={30} width={1.3} fontSize={10} />
            </div>
          )}
        </section>

        <section className="grid grid-cols-6 gap-4">
          <Field label="Selling price" required className="col-span-2">
            <MoneyInput value={form.salePrice} onChange={(v) => set({ salePrice: v ?? 0 })} />
          </Field>
          {settings.enableWholesale && (
            <Field label="Wholesale price" className="col-span-2">
              <MoneyInput value={form.wholesalePrice ?? null} onChange={(v) => set({ wholesalePrice: v })} />
            </Field>
          )}
          {can('viewCosts') && (
            <Field
              label="Cost price"
              className="col-span-2"
              hint={
                costEditable
                  ? margin !== null
                    ? `Margin ${percent(margin)} · profit ${money(form.salePrice - (cost ?? 0))}`
                    : 'Purchase cost per unit.'
                  : 'Average cost from purchases (cannot be typed while stock is on hand).'
              }
            >
              <MoneyInput value={form.costPrice ?? null} disabled={!costEditable} onChange={(v) => set({ costPrice: v })} />
            </Field>
          )}
          <Field label={`Reorder level (${unit?.shortName ?? 'units'})`} className="col-span-2" hint="Warn when stock falls to this level.">
            <QtyInput value={form.reorderLevel} allowDecimal={allowDecimal} onChange={(v) => set({ reorderLevel: v ?? 0 })} />
          </Field>
        </section>

        {!isEdit && can('manageInventory') && (
          <section className="rounded-xl border border-dashed border-slate-300 p-4">
            <p className="mb-3 text-sm font-semibold text-slate-800">Opening stock</p>
            <div className="grid grid-cols-6 gap-4">
              <Field label="Quantity on hand" className="col-span-2">
                <QtyInput value={form.openingQty} allowDecimal={allowDecimal} onChange={(v) => set({ openingQty: v })} placeholder="0" />
              </Field>
              <Field label="Cost per unit" className="col-span-2">
                <MoneyInput value={form.openingCost ?? form.costPrice ?? null} onChange={(v) => set({ openingCost: v })} />
              </Field>
              {form.trackBatches && (
                <>
                  <Field label="Batch no." className="col-span-1">
                    <Input value={form.batchNo} onChange={(e) => set({ batchNo: e.target.value })} />
                  </Field>
                  <Field label="Expiry" className="col-span-1">
                    <Input type="date" value={form.expiryDate} onChange={(e) => set({ expiryDate: e.target.value })} />
                  </Field>
                </>
              )}
            </div>
          </section>
        )}

        <section className="space-y-4">
          {settings.enableBatchTracking && (
            <Switch
              checked={form.trackBatches}
              onChange={(v) => set({ trackBatches: v })}
              label="Track batches and expiry dates"
              description="For creams, cosmetics and other dated goods. Stock is sold first-expiry-first-out."
            />
          )}
          {isEdit && (
            <Switch checked={form.isActive ?? true} onChange={(v) => set({ isActive: v })} label="Available for sale" description="Inactive products are hidden from billing but keep their history." />
          )}
          <Field label="Description / notes">
            <Textarea rows={2} value={form.description ?? ''} onChange={(e) => set({ description: e.target.value })} />
          </Field>
        </section>
        <button type="submit" className="hidden" />
      </form>
    </Modal>
  );
}
