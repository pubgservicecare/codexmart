import { useState } from 'react';
import { keepPreviousData, useQuery, useQueryClient } from '@tanstack/react-query';
import { Copy, Download, Package, Pencil, Plus, Power } from 'lucide-react';
import { api, type Product, type ProductFilter } from '@/lib/api';
import { csvMoney, csvQty, exportCsv } from '@/lib/csv';
import { money, qty } from '@/lib/format';
import { useSession } from '@/lib/session';
import { useDebounced } from '@/lib/utils';
import { Badge, Button, EmptyState, ErrorBox, PageHeader, Pager, SearchBox, Select, Spinner, useToast } from '@/components/ui';
import { LookupSelect } from './LookupSelect';
import { ProductForm, type ProductFormMode } from './ProductForm';

const LIMIT = 100;

export function stockTone(p: Product): 'red' | 'amber' | 'green' {
  if (p.stockQty <= 0) return 'red';
  if (p.reorderLevel > 0 && p.stockQty <= p.reorderLevel) return 'amber';
  return 'green';
}

export function ProductsPage() {
  const { can } = useSession();
  const toast = useToast();
  const qc = useQueryClient();
  const [search, setSearch] = useState('');
  const [filter, setFilter] = useState<ProductFilter>({ status: 'active', stock: '', sort: 'name' });
  const [offset, setOffset] = useState(0);
  const [form, setForm] = useState<ProductFormMode | null>(null);
  const q = useDebounced(search, 200);
  const query = { ...filter, search: q, stock: filter.stock || undefined };
  const { data, isLoading, error, refetch } = useQuery({
    queryKey: ['products', 'list', query, offset],
    queryFn: () => api.products.list({ ...query, limit: LIMIT, offset }),
    placeholderData: keepPreviousData,
  });

  const update = (patch: Partial<ProductFilter>) => {
    setFilter((f) => ({ ...f, ...patch }));
    setOffset(0);
  };

  const toggleActive = async (p: Product) => {
    try {
      await api.products.setActive(p.id, !p.isActive);
      void qc.invalidateQueries({ queryKey: ['products'] });
      toast.success(p.isActive ? 'Product deactivated' : 'Product reactivated', p.displayName);
    } catch (e) {
      toast.error(e);
    }
  };

  const exportAll = async () => {
    try {
      const all = await api.products.list({ ...query, limit: 1000 });
      const ok = await exportCsv('products.csv', all.items, [
        { header: 'Name', value: (p) => p.name },
        { header: 'Size', value: (p) => p.size },
        { header: 'Colour', value: (p) => p.color },
        { header: 'SKU', value: (p) => p.sku },
        { header: 'Barcode', value: (p) => p.barcode },
        { header: 'Category', value: (p) => p.categoryName },
        { header: 'Brand', value: (p) => p.brandName },
        { header: 'Unit', value: (p) => p.unitShort },
        { header: 'Stock', value: (p) => csvQty(p.stockQty) },
        { header: 'Reorder level', value: (p) => csvQty(p.reorderLevel) },
        { header: 'Cost', value: (p) => csvMoney(p.costPrice) },
        { header: 'Price', value: (p) => csvMoney(p.salePrice) },
        { header: 'Wholesale', value: (p) => csvMoney(p.wholesalePrice) },
        { header: 'Active', value: (p) => (p.isActive ? 'yes' : 'no') },
      ]);
      if (ok) toast.success('Exported');
    } catch (e) {
      toast.error(e);
    }
  };

  return (
    <div>
      <PageHeader
        title="Products"
        description="Everything you sell: prices, codes, stock levels and variants."
        icon={<Package className="h-5 w-5" />}
        actions={
          <>
            <Button icon={<Download className="h-4 w-4" />} onClick={exportAll}>
              Export CSV
            </Button>
            {can('manageCatalog') && (
              <Button variant="primary" icon={<Plus className="h-4 w-4" />} onClick={() => setForm({ kind: 'new' })}>
                New product
              </Button>
            )}
          </>
        }
      />
      <div className="card">
        <div className="flex flex-wrap items-center gap-3 border-b border-slate-100 p-4">
          <SearchBox value={search} onChange={(v) => { setSearch(v); setOffset(0); }} placeholder="Name, SKU, barcode, brand, size…" className="w-80" autoFocus />
          <div className="w-44">
            <LookupSelect kind="category" value={filter.categoryId ?? null} onChange={(v) => update({ categoryId: v ?? undefined })} emptyLabel="All categories" canAdd={false} />
          </div>
          <div className="w-40">
            <LookupSelect kind="brand" value={filter.brandId ?? null} onChange={(v) => update({ brandId: v ?? undefined })} emptyLabel="All brands" canAdd={false} />
          </div>
          <Select className="w-40" value={filter.stock} onChange={(e) => update({ stock: e.target.value as ProductFilter['stock'] })}>
            <option value="">Any stock</option>
            <option value="attention">Low or out</option>
            <option value="low">Low stock</option>
            <option value="out">Out of stock</option>
            <option value="in">In stock</option>
          </Select>
          <Select className="w-32" value={filter.status} onChange={(e) => update({ status: e.target.value as ProductFilter['status'] })}>
            <option value="active">Active</option>
            <option value="inactive">Inactive</option>
            <option value="all">All</option>
          </Select>
          <Select className="w-40" value={filter.sort} onChange={(e) => update({ sort: e.target.value as ProductFilter['sort'] })}>
            <option value="name">Sort: name</option>
            <option value="stock">Sort: lowest stock</option>
            <option value="updated">Sort: recently changed</option>
            <option value="price">Sort: highest price</option>
            <option value="sku">Sort: SKU</option>
          </Select>
        </div>
        {error ? (
          <ErrorBox error={error} onRetry={refetch} />
        ) : isLoading ? (
          <Spinner />
        ) : !data?.items.length ? (
          <EmptyState
            icon={<Package className="h-6 w-6" />}
            title={q ? 'No matching products' : 'No products yet'}
            description={q ? 'Try another search or filter.' : 'Add your first product to start billing.'}
            action={can('manageCatalog') && !q && <Button variant="primary" icon={<Plus className="h-4 w-4" />} onClick={() => setForm({ kind: 'new' })}>New product</Button>}
          />
        ) : (
          <>
            <div className="overflow-x-auto">
              <table className="table">
                <thead>
                  <tr>
                    <th>Product</th>
                    <th>SKU / Barcode</th>
                    <th>Category</th>
                    <th className="text-right">Stock</th>
                    {can('viewCosts') && <th className="text-right">Cost</th>}
                    <th className="text-right">Price</th>
                    <th className="w-28" />
                  </tr>
                </thead>
                <tbody>
                  {data.items.map((p) => (
                    <tr key={p.id} className={can('manageCatalog') ? 'clickable' : ''} onClick={() => can('manageCatalog') && setForm({ kind: 'edit', product: p })}>
                      <td>
                        <p className="font-medium text-slate-900">
                          {p.displayName} {!p.isActive && <Badge>Inactive</Badge>}
                        </p>
                        <p className="text-2xs text-slate-500">
                          {p.brandName ?? 'No brand'}
                          {p.trackBatches && ' · batch tracked'}
                          {p.variantGroupId && ' · has variants'}
                        </p>
                      </td>
                      <td>
                        <p className="font-mono text-xs text-slate-700">{p.sku}</p>
                        <p className="font-mono text-2xs text-slate-400">{p.barcode ?? '—'}</p>
                      </td>
                      <td className="text-slate-600">{p.categoryName ?? '—'}</td>
                      <td className="text-right">
                        <Badge tone={stockTone(p)}>
                          {qty(p.stockQty)} {p.unitShort}
                        </Badge>
                      </td>
                      {can('viewCosts') && <td className="num text-right text-slate-600">{money(p.costPrice)}</td>}
                      <td className="num text-right font-semibold text-slate-900">{money(p.salePrice)}</td>
                      <td className="text-right" onClick={(e) => e.stopPropagation()}>
                        {can('manageCatalog') && (
                          <div className="flex justify-end gap-0.5">
                            <Button size="sm" variant="ghost" iconOnly title="Edit" onClick={() => setForm({ kind: 'edit', product: p })}>
                              <Pencil className="h-4 w-4" />
                            </Button>
                            <Button size="sm" variant="ghost" iconOnly title="Add a variant (other size or colour)" onClick={() => setForm({ kind: 'variant', of: p })}>
                              <Copy className="h-4 w-4" />
                            </Button>
                            <Button size="sm" variant="ghost" iconOnly title={p.isActive ? 'Deactivate' : 'Reactivate'} onClick={() => toggleActive(p)}>
                              <Power className={`h-4 w-4 ${p.isActive ? '' : 'text-emerald-600'}`} />
                            </Button>
                          </div>
                        )}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
            <Pager total={data.total} limit={LIMIT} offset={offset} onChange={setOffset} />
          </>
        )}
      </div>
      {form && <ProductForm mode={form} onClose={() => setForm(null)} />}
    </div>
  );
}
