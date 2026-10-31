//! Perpetual weighted-average costing (AVCO).
//!
//! Each product keeps its on-hand quantity and the total cost value of that
//! quantity. Receipts add their actual cost; issues remove a proportional
//! share of the value. The cost recorded on every sale line is therefore the
//! actual average cost at the moment of sale, which is what gross profit is
//! calculated from.
//!
//! If negative stock is allowed and goods are sold before they are received,
//! the uncovered quantity is costed at the last known unit cost.

use crate::money::{self, Money, Qty};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StockState {
    /// On-hand quantity in thousandths (may be negative if allowed).
    pub qty: Qty,
    /// Cost value of the positive on-hand quantity.
    pub value: Money,
    /// Current average cost per whole unit.
    pub unit_cost: Money,
}

/// Removes `qty` at average cost. Returns the new state and the cost of the
/// goods issued.
pub fn issue(s: StockState, qty: Qty) -> (StockState, Money) {
    let mut cost = 0;
    let mut value = s.value;
    let mut uncovered = qty;
    if s.qty > 0 {
        let portion = qty.min(s.qty);
        let portion_cost = if portion == s.qty { s.value } else { money::prorate(s.value, portion, s.qty) };
        cost += portion_cost;
        value -= portion_cost;
        uncovered = qty - portion;
    }
    cost += money::mul_qty(s.unit_cost, uncovered);
    let new_qty = s.qty - qty;
    let state = StockState { qty: new_qty, value: if new_qty > 0 { value.max(0) } else { 0 }, unit_cost: s.unit_cost };
    (state, cost)
}

/// Adds `qty` with a total cost of `total_value`.
pub fn receive(s: StockState, qty: Qty, total_value: Money) -> StockState {
    let new_qty = s.qty + qty;
    let value = if s.qty >= 0 {
        s.value + total_value
    } else if new_qty <= 0 {
        0
    } else {
        // Part of this receipt covers goods already sold; only the remainder
        // stays in inventory.
        money::prorate(total_value, new_qty, qty)
    };
    let unit_cost =
        if new_qty > 0 { money::unit_price_of(value, new_qty) } else { money::unit_price_of(total_value, qty) };
    StockState { qty: new_qty, value: if new_qty > 0 { value } else { 0 }, unit_cost }
}

/// Removes `qty` at a known value (e.g. returning goods to a supplier at the
/// price they were bought for). The average of what remains is recomputed.
pub fn issue_at_value(s: StockState, qty: Qty, value: Money) -> StockState {
    let new_qty = s.qty - qty;
    if new_qty <= 0 {
        return StockState { qty: new_qty, value: 0, unit_cost: s.unit_cost };
    }
    let new_value = (s.value - value).max(0);
    StockState { qty: new_qty, value: new_value, unit_cost: money::unit_price_of(new_value, new_qty) }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EMPTY: StockState = StockState { qty: 0, value: 0, unit_cost: 0 };

    #[test]
    fn weighted_average_across_two_receipts() {
        // 10 @ 100, then 10 @ 200 -> average 150
        let s = receive(EMPTY, 10_000, 100_000);
        let s = receive(s, 10_000, 200_000);
        assert_eq!(s.qty, 20_000);
        assert_eq!(s.value, 300_000);
        assert_eq!(s.unit_cost, 15_000);
        let (s, cost) = issue(s, 5_000);
        assert_eq!(cost, 75_000);
        assert_eq!(s.value, 225_000);
        assert_eq!(s.qty, 15_000);
    }

    #[test]
    fn issuing_everything_leaves_no_residual_value() {
        let s = receive(EMPTY, 3_000, 10_000); // 3 for Rs 100.00 (33.333 each)
        let (s, c1) = issue(s, 1_000);
        let (s, c2) = issue(s, 1_000);
        let (s, c3) = issue(s, 1_000);
        assert_eq!(c1 + c2 + c3, 10_000);
        assert_eq!(s.value, 0);
        assert_eq!(s.qty, 0);
    }

    #[test]
    fn negative_stock_uses_last_cost() {
        let s = receive(EMPTY, 1_000, 5_000);
        let (s, cost) = issue(s, 3_000);
        assert_eq!(cost, 15_000); // 1 from stock + 2 at last cost 50.00
        assert_eq!(s.qty, -2_000);
        assert_eq!(s.value, 0);
        let s = receive(s, 4_000, 24_000); // 4 @ 60, 2 cover the deficit
        assert_eq!(s.qty, 2_000);
        assert_eq!(s.value, 12_000);
        assert_eq!(s.unit_cost, 6_000);
    }

    #[test]
    fn supplier_return_at_purchase_value() {
        let s = receive(EMPTY, 10_000, 100_000);
        let s = receive(s, 10_000, 200_000);
        let s = issue_at_value(s, 10_000, 200_000);
        assert_eq!(s.qty, 10_000);
        assert_eq!(s.value, 100_000);
        assert_eq!(s.unit_cost, 10_000);
    }
}
