import { useRef, useState, type KeyboardEvent } from 'react';
import { useQuery } from '@tanstack/react-query';
import { ScanBarcode } from 'lucide-react';
import { api, type Product } from '@/lib/api';
import { money, qty } from '@/lib/format';
import { cx, useDebounced } from '@/lib/utils';

/** Search-as-you-type product chooser; Enter on a scanned code picks it directly. */
export function ProductSearch({
  onPick,
  placeholder = 'Scan or search a product to add…',
  autoFocus,
  showCost,
}: {
  onPick: (p: Product) => void;
  placeholder?: string;
  autoFocus?: boolean;
  showCost?: boolean;
}) {
  const [text, setText] = useState('');
  const [open, setOpen] = useState(false);
  const [hi, setHi] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  const blurTimer = useRef<number | undefined>(undefined);
  const q = useDebounced(text.trim(), 120);
  const { data } = useQuery({ queryKey: ['product-search', q], queryFn: () => api.products.search(q, 15), enabled: q.length > 0 });
  const results = q ? (data ?? []) : [];

  const pick = (p: Product) => {
    onPick(p);
    setText('');
    setOpen(false);
    setHi(0);
    inputRef.current?.focus();
  };

  const onKey = async (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      setHi((h) => Math.min(h + 1, results.length - 1));
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      setHi((h) => Math.max(h - 1, 0));
    } else if (e.key === 'Enter') {
      e.preventDefault();
      const code = text.trim();
      if (!code) return;
      const exact = await api.products.findByCode(code).catch(() => null);
      if (exact) pick(exact);
      else if (results[hi]) pick(results[hi]);
    } else if (e.key === 'Escape') {
      setOpen(false);
    }
  };

  return (
    <div className="relative">
      <ScanBarcode className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-slate-400" />
      <input
        ref={inputRef}
        autoFocus={autoFocus}
        className="input pl-9"
        placeholder={placeholder}
        value={text}
        onChange={(e) => {
          window.clearTimeout(blurTimer.current);
          setText(e.target.value);
          setOpen(true);
          setHi(0);
        }}
        onFocus={() => {
          // A close scheduled by an earlier blur must not hide the list now.
          window.clearTimeout(blurTimer.current);
          setOpen(true);
        }}
        onBlur={() => {
          blurTimer.current = window.setTimeout(() => setOpen(false), 150);
        }}
        onKeyDown={onKey}
      />
      {open && results.length > 0 && (
        <div className="absolute left-0 right-0 top-full z-30 mt-1 max-h-72 overflow-y-auto rounded-xl border border-slate-200 bg-white shadow-pop">
          {results.map((p, i) => (
            <button
              key={p.id}
              type="button"
              onMouseDown={(e) => e.preventDefault()}
              onClick={() => pick(p)}
              onMouseEnter={() => setHi(i)}
              className={cx('flex w-full items-center justify-between gap-3 px-3 py-2 text-left', i === hi ? 'bg-brand-50' : '')}
            >
              <span className="min-w-0">
                <span className="block truncate text-sm font-medium text-slate-900">{p.displayName}</span>
                <span className="text-2xs text-slate-500">
                  {p.sku} · stock {qty(p.stockQty)} {p.unitShort}
                </span>
              </span>
              <span className="num text-right text-xs text-slate-600">
                {showCost && p.costPrice !== null ? `cost ${money(p.costPrice)}` : money(p.salePrice)}
              </span>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
