import { useCallback, useEffect, useMemo, useRef, useState, type KeyboardEvent as ReactKeyboardEvent } from 'react';
import { keepPreviousData, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  Barcode as BarcodeIcon,
  Minus,
  Percent,
  Plus,
  ScanBarcode,
  ShoppingCart,
  Tag,
  Trash,
  TriangleAlert,
  UserRound,
  X,
} from 'lucide-react';
import { api, ApiError, errorMessage, type Product, type SaleDetail, type Tender } from '@/lib/api';
import { money, parseQty, qty as fmtQty } from '@/lib/format';
import { useSession, useSettings } from '@/lib/session';
import { useBarcodeScanner } from '@/lib/scanner';
import { cx, useDebounced } from '@/lib/utils';
import { Badge, Button, ConfirmDialog, EmptyState, QtyInput, Spinner, useToast } from '@/components/ui';
import { useReceiptPrinter } from '@/components/useReceiptPrinter';
import { usePosCart } from './usePosCart';
import { DiscountDialog } from './DiscountDialog';
import { CustomerPicker } from './CustomerPicker';
import { PaymentDialog } from './PaymentDialog';
import { ReceiptDialog } from './ReceiptDialog';

function StockBadge({ p }: { p: Product }) {
  if (p.stockQty <= 0) return <Badge tone="red">Out of stock</Badge>;
  if (p.reorderLevel > 0 && p.stockQty <= p.reorderLevel) return <Badge tone="amber">Low · {fmtQty(p.stockQty)}</Badge>;
  return <Badge tone="gray">{fmtQty(p.stockQty)} {p.unitShort}</Badge>;
}

export function PosPage() {
  const settings = useSettings();
  const { can, status } = useSession();
  const toast = useToast();
  const qc = useQueryClient();
  const printSale = useReceiptPrinter();
  const pos = usePosCart(status?.deviceId ?? null);
  const { state, cart } = pos;

  const searchRef = useRef<HTMLInputElement>(null);
  const [query, setQuery] = useState('');
  const [highlight, setHighlight] = useState(0);
  const [category, setCategory] = useState<string>('');
  const [selected, setSelected] = useState<number | null>(null);
  const [lineDiscountFor, setLineDiscountFor] = useState<number | null>(null);
  const [billDiscountOpen, setBillDiscountOpen] = useState(false);
  const [customerOpen, setCustomerOpen] = useState(false);
  const [payOpen, setPayOpen] = useState(false);
  const [paying, setPaying] = useState(false);
  const [completed, setCompleted] = useState<SaleDetail | null>(null);
  const [clearOpen, setClearOpen] = useState(false);
  const [resumeNotice] = useState(() => !!state.txnId && state.lines.length > 0);
  const anyModal = lineDiscountFor !== null || billDiscountOpen || customerOpen || payOpen || !!completed || clearOpen;

  // "3*" before a search or scan sets the quantity of the next item.
  const multiplier = /^(\d+(?:\.\d{1,3})?)\*(.*)$/.exec(query);
  const nextQty = multiplier ? (parseQty(multiplier[1]) ?? 1000) : 1000;
  const term = (multiplier ? multiplier[2] : query).trim();
  const debounced = useDebounced(term, 120);

  const results = useQuery({
    queryKey: ['pos-search', debounced],
    queryFn: () => api.products.search(debounced, 40),
    enabled: debounced.length > 0,
    placeholderData: keepPreviousData,
  });
  const browse = useQuery({
    queryKey: ['pos-browse', category],
    queryFn: () => api.products.list({ categoryId: category || undefined, limit: 120, sort: 'name' }),
    enabled: term.length === 0,
  });
  const categories = useQuery({ queryKey: ['lookups', 'category'], queryFn: () => api.lookups.list('category') });

  const activeIdx = useMemo(() => state.lines.map((l, i) => (l.qty > 0 ? i : -1)).filter((i) => i >= 0), [state.lines]);
  const cartKey = JSON.stringify(cart);
  const quote = useQuery({
    queryKey: ['quote', cartKey],
    queryFn: () => api.sales.quote(cart),
    enabled: cart.lines.length > 0,
    placeholderData: keepPreviousData,
    staleTime: 0,
    retry: false,
  });
  const quoteFresh = quote.data && !quote.isPlaceholderData && cart.lines.length > 0;
  const totals = cart.lines.length > 0 ? quote.data?.totals : undefined;

  // F9 pressed while the total is still being calculated: open payment as
  // soon as the up-to-date total arrives (fast scan-then-pay cashiers).
  const [payRequested, setPayRequested] = useState(false);
  useEffect(() => {
    if (!payRequested) return;
    if (quote.error || cart.lines.length === 0) setPayRequested(false);
    else if (quoteFresh && !quote.isFetching) {
      setPayRequested(false);
      setPayOpen(true);
    }
  }, [payRequested, quoteFresh, quote.isFetching, quote.error, cart.lines.length]);

  const focusSearch = useCallback(() => searchRef.current?.focus(), []);
  useEffect(() => {
    if (!anyModal) focusSearch();
  }, [anyModal, focusSearch]);

  /** `typed`: the search text this add came from. The box is only cleared if
   * it still holds that text, so anything typed meanwhile is kept. */
  const addProduct = useCallback(
    (p: Product, q = nextQty, typed?: string) => {
      if (!p.isActive) {
        toast.error(`${p.displayName} is not for sale`, 'The product is deactivated.');
        return;
      }
      pos.add(p, q);
      setQuery((cur) => (typed === undefined || cur === typed ? '' : cur));
      setHighlight(0);
      setSelected(null);
    },
    [pos, nextQty, toast],
  );

  const addByCode = useCallback(
    async (code: string, q = 1000, typed?: string) => {
      try {
        const p = await api.products.findByCode(code);
        if (p) {
          addProduct(p, q, typed);
          return true;
        }
      } catch (e) {
        toast.error(e);
        return true;
      }
      return false;
    },
    [addProduct, toast],
  );

  useBarcodeScanner(
    (code) => {
      void addByCode(code).then((found) => {
        if (!found) toast.error('Barcode not found', `No product has the code ${code}.`);
      });
    },
    !anyModal,
  );

  const onSearchKey = async (e: ReactKeyboardEvent<HTMLInputElement>) => {
    const list = results.data ?? [];
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      setHighlight((h) => Math.min(h + 1, Math.max(list.length - 1, 0)));
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      setHighlight((h) => Math.max(h - 1, 0));
    } else if (e.key === 'Enter' && term) {
      e.preventDefault();
      const typed = query;
      if (await addByCode(term, nextQty, typed)) return;
      const p = list[highlight];
      if (p && debounced === term) addProduct(p, nextQty, typed);
      else if (!results.isFetching && debounced === term) toast.info('No product matches', term);
    } else if (e.key === 'Escape') {
      setQuery('');
    }
  };

  // Global till shortcuts.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (anyModal) return;
      const inField = e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement;
      switch (e.key) {
        case 'F2':
          e.preventDefault();
          focusSearch();
          break;
        case 'F4':
          e.preventDefault();
          setCustomerOpen(true);
          break;
        case 'F6':
          e.preventDefault();
          if (cart.lines.length) setBillDiscountOpen(true);
          break;
        case 'F9':
          e.preventDefault();
          if (cart.lines.length) setPayRequested(true);
          break;
        case 'Delete':
          if (!inField && selected !== null) {
            pos.remove(selected);
            setSelected(null);
          }
          break;
        case '+':
        case '-':
          if (!inField && selected !== null && state.lines[selected]) {
            e.preventDefault();
            const l = state.lines[selected];
            pos.setQty(selected, l.qty + (e.key === '+' ? 1000 : -1000));
          }
          break;
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [anyModal, cart.lines.length, selected, state.lines, pos, focusSearch]);

  const checkout = async (tenders: Tender[]) => {
    setPaying(true);
    const clientTxnId = pos.ensureTxnId();
    try {
      const sale = await api.sales.checkout({ clientTxnId, cart, payments: tenders });
      pos.clear();
      setPayOpen(false);
      setCompleted(sale);
      void qc.invalidateQueries({ queryKey: ['pos-browse'] });
      void qc.invalidateQueries({ queryKey: ['pos-search'] });
      void qc.invalidateQueries({ queryKey: ['dashboard'] });
      void qc.invalidateQueries({ queryKey: ['sales'] });
      void qc.invalidateQueries({ queryKey: ['products'] });
      if (settings.autoPrintReceipt) void printSale(sale);
    } catch (e) {
      toast.error('Sale not completed', errorMessage(e));
    } finally {
      setPaying(false);
    }
  };

  const lineBase = (i: number) => {
    const k = activeIdx.indexOf(i);
    const ql = k >= 0 && quote.data ? quote.data.lines[k] : undefined;
    return ql ? ql.gross : Math.round((state.lines[i].price * state.lines[i].qty) / 1000);
  };

  const showing = term ? (results.data ?? []) : (browse.data?.items ?? []);

  return (
    <div className="flex h-full">
      {/* ------------------------------------------------ product finder */}
      <section className="flex min-w-0 flex-1 flex-col">
        <div className="border-b border-slate-200 bg-white px-5 py-4">
          <div className="flex items-center gap-3">
            <div className="relative flex-1">
              <ScanBarcode className="pointer-events-none absolute left-3.5 top-1/2 h-5 w-5 -translate-y-1/2 text-slate-400" />
              <input
                ref={searchRef}
                value={query}
                onChange={(e) => {
                  setQuery(e.target.value);
                  setHighlight(0);
                }}
                onKeyDown={onSearchKey}
                placeholder="Scan a barcode or type a product name, SKU or brand…"
                className="input h-12 rounded-xl pl-11 pr-20 text-base"
                autoFocus
              />
              <span className="kbd absolute right-3 top-1/2 -translate-y-1/2">F2</span>
            </div>
            {settings.enableWholesale && (
              <div className="flex rounded-xl bg-slate-100 p-1">
                {(['retail', 'wholesale'] as const).map((t) => (
                  <button
                    key={t}
                    className={cx('rounded-lg px-3 py-2 text-xs font-semibold capitalize', state.tier === t ? 'bg-white text-slate-900 shadow-sm' : 'text-slate-500')}
                    onClick={() => pos.setTier(t)}
                  >
                    {t}
                  </button>
                ))}
              </div>
            )}
          </div>
          <p className="mt-2 text-2xs text-slate-500">
            Tip: type <span className="kbd">3*</span> before a name or scan to add 3 at once. <span className="kbd">↑</span>
            <span className="kbd ml-0.5">↓</span> to choose, <span className="kbd">Enter</span> to add.
            {multiplier && <b className="ml-2 text-brand-700">Next item quantity: {fmtQty(nextQty)}</b>}
          </p>
        </div>

        {!term && (
          <div className="flex gap-2 overflow-x-auto border-b border-slate-200 bg-white/60 px-5 py-2.5">
            {[{ id: '', name: 'All products' }, ...(categories.data ?? [])].map((c) => (
              <button
                key={c.id}
                onClick={() => setCategory(c.id)}
                className={cx(
                  'whitespace-nowrap rounded-full px-3 py-1 text-xs font-medium ring-1 ring-inset transition',
                  category === c.id ? 'bg-brand-600 text-white ring-brand-600' : 'bg-white text-slate-600 ring-slate-200 hover:ring-slate-300',
                )}
              >
                {c.name}
              </button>
            ))}
          </div>
        )}

        <div className="min-h-0 flex-1 overflow-y-auto p-5">
          {resumeNotice && state.txnId && (
            <div className="mb-4 flex items-start gap-3 rounded-xl border border-amber-200 bg-amber-50 px-4 py-3 text-sm text-amber-900">
              <TriangleAlert className="mt-0.5 h-4 w-4 shrink-0" />
              <p>
                A sale was being paid when the app closed. Press <b>Pay</b> to finish it. If it had already been saved, the saved receipt is
                shown instead — the customer is never charged twice.
              </p>
            </div>
          )}
          {(term ? results.isLoading : browse.isLoading) ? (
            <Spinner />
          ) : showing.length === 0 ? (
            term ? (
              <EmptyState icon={<BarcodeIcon className="h-6 w-6" />} title="No matching products" description={`Nothing matches “${term}”. Check the spelling or add the product first.`} />
            ) : (
              <EmptyState
                icon={<Tag className="h-6 w-6" />}
                title="No products yet"
                description={can('manageCatalog') ? 'Add products in Products → New product, then they appear here.' : 'Ask a manager to add products.'}
              />
            )
          ) : term ? (
            <div className="card overflow-hidden">
              {showing.map((p, i) => (
                <button
                  key={p.id}
                  onClick={() => addProduct(p)}
                  onMouseEnter={() => setHighlight(i)}
                  className={cx(
                    'flex w-full items-center gap-4 border-b border-slate-100 px-4 py-3 text-left last:border-0',
                    i === highlight ? 'bg-brand-50' : 'hover:bg-slate-50',
                  )}
                >
                  <div className="min-w-0 flex-1">
                    <p className="truncate text-sm font-semibold text-slate-900">{p.displayName}</p>
                    <p className="truncate text-xs text-slate-500">
                      {p.sku}
                      {p.barcode && ` · ${p.barcode}`}
                      {p.brandName && ` · ${p.brandName}`}
                    </p>
                  </div>
                  <StockBadge p={p} />
                  <span className="num w-28 text-right text-sm font-semibold text-slate-900">{money(p.salePrice)}</span>
                </button>
              ))}
            </div>
          ) : (
            <div className="grid grid-cols-[repeat(auto-fill,minmax(168px,1fr))] gap-3">
              {showing.map((p) => (
                <button
                  key={p.id}
                  onClick={() => addProduct(p)}
                  className="card group flex flex-col p-3.5 text-left transition hover:-translate-y-0.5 hover:border-brand-300 hover:shadow-pop"
                >
                  <p className="line-clamp-2 min-h-[2.5rem] text-sm font-semibold leading-5 text-slate-900">{p.displayName}</p>
                  <p className="mt-0.5 truncate text-2xs text-slate-500">{p.brandName ?? p.categoryName ?? p.sku}</p>
                  <div className="mt-3 flex items-end justify-between gap-2">
                    <span className="num text-base font-semibold text-brand-700">{money(p.salePrice)}</span>
                    <StockBadge p={p} />
                  </div>
                </button>
              ))}
            </div>
          )}
        </div>
      </section>

      {/* ---------------------------------------------------------- bill */}
      <aside className="flex w-[440px] shrink-0 flex-col border-l border-slate-200 bg-white">
        <div className="flex items-center justify-between border-b border-slate-100 px-5 py-3.5">
          <div>
            <h2 className="text-sm font-semibold text-slate-900">Current bill</h2>
            <p className="text-2xs text-slate-500">
              {cart.lines.length
                ? `${cart.lines.length} line(s) · ${fmtQty(cart.lines.reduce((s, l) => s + l.qty, 0))} item(s)`
                : 'Empty'}
            </p>
          </div>
          {state.lines.length > 0 && (
            <Button size="sm" variant="ghost" icon={<Trash className="h-3.5 w-3.5" />} onClick={() => setClearOpen(true)}>
              Clear
            </Button>
          )}
        </div>

        <button
          onClick={() => setCustomerOpen(true)}
          className="mx-5 mt-3 flex items-center gap-3 rounded-xl border border-dashed border-slate-300 px-3.5 py-2.5 text-left hover:border-brand-300 hover:bg-brand-50/40"
        >
          <div className="flex h-8 w-8 items-center justify-center rounded-full bg-slate-100 text-slate-500">
            <UserRound className="h-4 w-4" />
          </div>
          <div className="min-w-0 flex-1">
            <p className="truncate text-sm font-medium text-slate-900">{state.customer?.name ?? 'Walk-in customer'}</p>
            <p className="text-2xs text-slate-500">
              {state.customer
                ? state.customer.balance > 0
                  ? `Owes ${money(state.customer.balance)}`
                  : state.customer.phone ?? 'No balance due'
                : 'Add a customer for credit or history'}
            </p>
          </div>
          {state.customer ? (
            <span
              role="button"
              className="rounded-md p-1 text-slate-400 hover:bg-slate-100 hover:text-slate-600"
              onClick={(e) => {
                e.stopPropagation();
                pos.setCustomer(null);
              }}
            >
              <X className="h-4 w-4" />
            </span>
          ) : (
            <span className="kbd">F4</span>
          )}
        </button>

        <div className="mt-2 min-h-0 flex-1 overflow-y-auto px-3">
          {state.lines.length === 0 ? (
            <EmptyState icon={<ShoppingCart className="h-6 w-6" />} title="No items yet" description="Scan a barcode or pick a product to start the bill." />
          ) : (
            state.lines.map((l, i) => {
              const k = activeIdx.indexOf(i);
              const ql = k >= 0 && quote.data && !quote.isPlaceholderData ? quote.data.lines[k] : undefined;
              return (
                <div
                  key={`${l.productId}-${i}`}
                  onClick={() => setSelected(i)}
                  className={cx('mb-1 rounded-xl px-3 py-2.5 transition', selected === i ? 'bg-brand-50 ring-1 ring-brand-200' : 'hover:bg-slate-50')}
                >
                  <div className="flex items-start justify-between gap-3">
                    <div className="min-w-0">
                      <p className="truncate text-sm font-semibold text-slate-900">{l.name}</p>
                      <p className="text-2xs text-slate-500">
                        {money(ql?.unitPrice ?? l.price)} / {l.unitShort}
                        {ql && !ql.stockOk && (
                          <span className="ml-1.5 font-semibold text-amber-700">· only {fmtQty(ql.stockQty)} in stock</span>
                        )}
                      </p>
                    </div>
                    <div className="text-right">
                      <p className="num text-sm font-semibold text-slate-900">{money(ql ? ql.net : Math.round((l.price * l.qty) / 1000))}</p>
                      {ql && ql.lineDiscount + ql.billDiscountShare > 0 && (
                        <p className="num text-2xs text-emerald-700">−{money(ql.lineDiscount + ql.billDiscountShare)}</p>
                      )}
                    </div>
                  </div>
                  <div className="mt-2 flex items-center gap-1.5">
                    <Button size="sm" iconOnly aria-label="Less" onClick={() => pos.setQty(i, l.qty - 1000)} disabled={l.qty <= 1000 && !l.allowDecimal}>
                      <Minus className="h-3.5 w-3.5" />
                    </Button>
                    <QtyInput
                      value={l.qty}
                      allowDecimal={l.allowDecimal}
                      onChange={(v) => pos.setQty(i, v ?? 0)}
                      onBlur={() => l.qty <= 0 && pos.remove(i)}
                      className="h-8 w-20 text-center"
                    />
                    <Button size="sm" iconOnly aria-label="More" onClick={() => pos.setQty(i, l.qty + 1000)}>
                      <Plus className="h-3.5 w-3.5" />
                    </Button>
                    <span className="flex-1" />
                    <Button size="sm" variant={l.discount ? 'primary' : 'ghost'} iconOnly aria-label="Line discount" title="Line discount" onClick={() => setLineDiscountFor(i)}>
                      <Percent className="h-3.5 w-3.5" />
                    </Button>
                    <Button size="sm" variant="ghost" iconOnly aria-label="Remove" onClick={() => pos.remove(i)}>
                      <Trash className="h-3.5 w-3.5" />
                    </Button>
                  </div>
                </div>
              );
            })
          )}
        </div>

        <div className="border-t border-slate-200 bg-slate-50/70 px-5 py-4">
          {quote.error && cart.lines.length > 0 && (
            <p className="mb-2 rounded-lg bg-rose-50 px-3 py-2 text-xs text-rose-700">
              {ApiError.from(quote.error).code === 'NOT_FOUND'
                ? 'An item on this bill no longer exists (for example after restoring a backup). Remove it or clear the bill.'
                : errorMessage(quote.error)}
            </p>
          )}
          {quote.data?.warnings.map((w) => (
            <p key={w} className="mb-2 flex items-start gap-1.5 text-xs text-amber-800">
              <TriangleAlert className="mt-0.5 h-3.5 w-3.5 shrink-0" />
              {w}
            </p>
          ))}
          <div className="space-y-1 text-sm">
            <div className="flex justify-between text-slate-600">
              <span>Subtotal</span>
              <span className="num">{money(totals?.subtotal ?? 0)}</span>
            </div>
            <div className="flex items-center justify-between text-slate-600">
              <button className="inline-flex items-center gap-1.5 hover:text-brand-700 disabled:opacity-50" disabled={!cart.lines.length} onClick={() => setBillDiscountOpen(true)}>
                Discount <span className="kbd">F6</span>
              </button>
              <span className="num text-emerald-700">{totals?.discountTotal ? `−${money(totals.discountTotal)}` : money(0)}</span>
            </div>
          </div>
          <div className="mt-3 flex items-end justify-between border-t border-dashed border-slate-300 pt-3">
            <span className="text-sm font-semibold uppercase tracking-wide text-slate-500">Total</span>
            <span className="num text-3xl font-bold tracking-tight text-slate-900">{money(totals?.total ?? 0)}</span>
          </div>
          <Button
            variant="primary"
            size="lg"
            className="mt-4 h-14 w-full text-lg"
            disabled={!cart.lines.length || !quoteFresh || !!quote.error}
            loading={quote.isFetching && cart.lines.length > 0 && !quoteFresh}
            onClick={() => setPayOpen(true)}
          >
            Pay {totals ? money(totals.total) : ''} <span className="kbd ml-1 border-brand-400 bg-brand-500 text-white">F9</span>
          </Button>
        </div>
      </aside>

      <DiscountDialog
        open={lineDiscountFor !== null}
        onClose={() => setLineDiscountFor(null)}
        title={lineDiscountFor !== null ? `Discount on ${state.lines[lineDiscountFor]?.name ?? 'line'}` : 'Line discount'}
        base={lineDiscountFor !== null && state.lines[lineDiscountFor] ? lineBase(lineDiscountFor) : 0}
        value={lineDiscountFor !== null ? (state.lines[lineDiscountFor]?.discount ?? null) : null}
        onApply={(d) => lineDiscountFor !== null && pos.setLineDiscount(lineDiscountFor, d)}
      />
      <DiscountDialog
        open={billDiscountOpen}
        onClose={() => setBillDiscountOpen(false)}
        title="Discount on the whole bill"
        base={(totals?.subtotal ?? 0) - (totals?.lineDiscountTotal ?? 0)}
        value={state.billDiscount}
        onApply={pos.setBillDiscount}
      />
      <CustomerPicker open={customerOpen} onClose={() => setCustomerOpen(false)} onPick={pos.setCustomer} />
      <PaymentDialog
        open={payOpen}
        onClose={() => setPayOpen(false)}
        total={totals?.total ?? 0}
        customer={state.customer}
        creditAllowed={settings.enableCreditSales}
        busy={paying}
        onConfirm={checkout}
      />
      <ReceiptDialog sale={completed} onClose={() => setCompleted(null)} />
      <ConfirmDialog
        open={clearOpen}
        onClose={() => setClearOpen(false)}
        title="Clear this bill?"
        message="All items on the current bill will be removed. Nothing has been charged yet."
        confirmLabel="Clear bill"
        danger
        onConfirm={() => {
          pos.clear();
          setClearOpen(false);
        }}
      />
    </div>
  );
}
