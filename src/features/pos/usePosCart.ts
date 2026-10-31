import { useCallback, useEffect, useState } from 'react';
import type { Cart, CustomerBrief, Discount, Product } from '@/lib/api';
import { readStorage, writeStorage } from '@/lib/utils';

export interface PosLine {
  productId: string;
  qty: number;
  discount: Discount | null;
  // Shown immediately while the backend quote is refreshed.
  name: string;
  sku: string;
  unitShort: string;
  allowDecimal: boolean;
  price: number;
}

export interface PosState {
  lines: PosLine[];
  billDiscount: Discount | null;
  customer: CustomerBrief | null;
  tier: 'retail' | 'wholesale';
  /**
   * Idempotency key of the checkout in progress. It survives a crash or
   * restart, so finishing the sale afterwards can never charge twice.
   */
  txnId: string | null;
}

const EMPTY: PosState = { lines: [], billDiscount: null, customer: null, tier: 'retail', txnId: null };

/**
 * The in-progress bill, persisted locally so a crash never loses it.
 * Stored per database (`deviceId`), so a different or replaced database
 * never inherits a bill that refers to products it does not have.
 */
export function usePosCart(deviceId: string | null) {
  const KEY = `cxm.pos.cart.v1.${deviceId ?? 'unknown'}`;
  const [state, setState] = useState<PosState>(() => readStorage(KEY, EMPTY));

  useEffect(() => writeStorage(KEY, state), [KEY, state]);

  // Any change to what is being sold starts a new transaction.
  const update = useCallback((fn: (s: PosState) => PosState) => setState((s) => ({ ...fn(s), txnId: null })), []);

  const add = useCallback(
    (p: Product, qty = 1000) =>
      update((s) => {
        const idx = s.lines.findIndex((l) => l.productId === p.id && !l.discount);
        if (idx >= 0) {
          const lines = [...s.lines];
          lines[idx] = { ...lines[idx], qty: lines[idx].qty + qty };
          return { ...s, lines };
        }
        const price = s.tier === 'wholesale' && p.wholesalePrice !== null ? p.wholesalePrice : p.salePrice;
        return {
          ...s,
          lines: [
            ...s.lines,
            { productId: p.id, qty, discount: null, name: p.displayName, sku: p.sku, unitShort: p.unitShort, allowDecimal: p.allowDecimal, price },
          ],
        };
      }),
    [update],
  );

  const setQty = useCallback(
    (index: number, qty: number) =>
      update((s) => ({ ...s, lines: s.lines.map((l, i) => (i === index ? { ...l, qty: Math.max(qty, 0) } : l)) })),
    [update],
  );

  const setLineDiscount = useCallback(
    (index: number, discount: Discount | null) =>
      update((s) => ({ ...s, lines: s.lines.map((l, i) => (i === index ? { ...l, discount } : l)) })),
    [update],
  );

  const remove = useCallback((index: number) => update((s) => ({ ...s, lines: s.lines.filter((_, i) => i !== index) })), [update]);

  const setBillDiscount = useCallback((billDiscount: Discount | null) => update((s) => ({ ...s, billDiscount })), [update]);
  const setCustomer = useCallback((customer: CustomerBrief | null) => update((s) => ({ ...s, customer })), [update]);
  const setTier = useCallback((tier: 'retail' | 'wholesale') => update((s) => ({ ...s, tier })), [update]);
  const clear = useCallback(() => setState(EMPTY), []);

  /** Returns the transaction id for this checkout, creating it once. */
  const ensureTxnId = useCallback((): string => {
    const current = readStorage<PosState>(KEY, EMPTY).txnId;
    if (current) return current;
    const id = crypto.randomUUID();
    setState((s) => {
      const next = { ...s, txnId: id };
      writeStorage(KEY, next);
      return next;
    });
    return id;
  }, [KEY]);

  const cart: Cart = {
    lines: state.lines.filter((l) => l.qty > 0).map((l) => ({ productId: l.productId, qty: l.qty, discount: l.discount })),
    billDiscount: state.billDiscount,
    priceTier: state.tier,
    customerId: state.customer?.id ?? null,
  };

  return { state, cart, add, setQty, setLineDiscount, remove, setBillDiscount, setCustomer, setTier, clear, ensureTxnId };
}
