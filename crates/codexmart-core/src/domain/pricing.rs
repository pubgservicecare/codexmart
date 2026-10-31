//! Cart pricing: line amounts, line discounts and bill discounts.
//!
//! The bill discount is allocated across lines in proportion to their
//! amount after line discounts, so every line carries its exact net value.
//! That makes product/category reports and partial returns add up to the
//! invoice total to the paisa.

use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};
use crate::money::{self, Money, Qty, PERCENT_SCALE};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "lowercase")]
pub enum Discount {
    /// Fixed amount in minor units.
    Amount(Money),
    /// Percentage in basis points (1250 = 12.5 %).
    Percent(i64),
}

impl Discount {
    /// Discount value against `base`, validated to stay within it.
    pub fn resolve(&self, base: Money, what: &str) -> AppResult<Money> {
        match *self {
            Discount::Amount(a) => {
                if a < 0 {
                    Err(AppError::validation(format!("{what}: discount cannot be negative.")))
                } else if a > base {
                    Err(AppError::validation(format!("{what}: discount cannot be more than the amount.")))
                } else {
                    Ok(a)
                }
            }
            Discount::Percent(bp) => {
                if !(0..=PERCENT_SCALE).contains(&bp) {
                    Err(AppError::validation(format!("{what}: discount must be between 0% and 100%.")))
                } else {
                    Ok(money::percent(base, bp))
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct LineInput {
    pub unit_price: Money,
    pub qty: Qty,
    pub discount: Option<Discount>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PricedLine {
    pub gross: Money,
    pub line_discount: Money,
    pub bill_discount_share: Money,
    pub net: Money,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CartTotals {
    /// Sum of line amounts before any discount.
    pub subtotal: Money,
    pub line_discount_total: Money,
    pub bill_discount: Money,
    pub discount_total: Money,
    /// Amount payable.
    pub total: Money,
}

pub fn price_cart(lines: &[LineInput], bill_discount: Option<Discount>) -> AppResult<(Vec<PricedLine>, CartTotals)> {
    let mut priced = Vec::with_capacity(lines.len());
    for (i, line) in lines.iter().enumerate() {
        let what = format!("Line {}", i + 1);
        if line.qty <= 0 {
            return Err(AppError::validation(format!("{what}: quantity must be more than zero.")));
        }
        if line.unit_price < 0 {
            return Err(AppError::validation(format!("{what}: price cannot be negative.")));
        }
        let gross = money::mul_qty(line.unit_price, line.qty);
        let line_discount = match &line.discount {
            Some(d) => d.resolve(gross, &what)?,
            None => 0,
        };
        priced.push(PricedLine { gross, line_discount, bill_discount_share: 0, net: gross - line_discount });
    }

    let after_line: Money = priced.iter().map(|l| l.net).sum();
    let bill = match &bill_discount {
        Some(d) => d.resolve(after_line, "Bill")?,
        None => 0,
    };
    let weights: Vec<Money> = priced.iter().map(|l| l.net).collect();
    for (line, share) in priced.iter_mut().zip(money::allocate(bill, &weights)) {
        line.bill_discount_share = share;
        line.net -= share;
    }

    let subtotal: Money = priced.iter().map(|l| l.gross).sum();
    let line_discount_total: Money = priced.iter().map(|l| l.line_discount).sum();
    let total: Money = priced.iter().map(|l| l.net).sum();
    Ok((
        priced,
        CartTotals {
            subtotal,
            line_discount_total,
            bill_discount: bill,
            discount_total: line_discount_total + bill,
            total,
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(price: Money, qty: Qty, discount: Option<Discount>) -> LineInput {
        LineInput { unit_price: price, qty, discount }
    }

    #[test]
    fn simple_cart_without_discounts() {
        let (lines, t) = price_cart(&[line(15_000, 2_000, None), line(5_050, 1_000, None)], None).unwrap();
        assert_eq!(lines[0].net, 30_000);
        assert_eq!(t.subtotal, 35_050);
        assert_eq!(t.total, 35_050);
        assert_eq!(t.discount_total, 0);
    }

    #[test]
    fn line_and_bill_discounts_add_up() {
        let lines = [
            line(10_000, 3_000, Some(Discount::Percent(1_000))), // 300 - 30 = 270
            line(9_999, 1_000, Some(Discount::Amount(999))),     // 99.99 - 9.99 = 90
            line(3_333, 1_000, None),                            // 33.33
        ];
        let (priced, t) = price_cart(&lines, Some(Discount::Amount(1_000))).unwrap();
        assert_eq!(t.subtotal, 30_000 + 9_999 + 3_333);
        assert_eq!(t.line_discount_total, 3_000 + 999);
        assert_eq!(t.bill_discount, 1_000);
        assert_eq!(t.total, t.subtotal - t.discount_total);
        assert_eq!(priced.iter().map(|l| l.net).sum::<i64>(), t.total);
        assert_eq!(priced.iter().map(|l| l.bill_discount_share).sum::<i64>(), 1_000);
    }

    #[test]
    fn percent_bill_discount_and_decimal_quantities() {
        // 0.75 kg at Rs 480/kg = 360.00; 10% off = 324.00
        let (_, t) = price_cart(&[line(48_000, 750, None)], Some(Discount::Percent(1_000))).unwrap();
        assert_eq!(t.subtotal, 36_000);
        assert_eq!(t.total, 32_400);
    }

    #[test]
    fn rejects_invalid_discounts_and_quantities() {
        assert!(price_cart(&[line(1_000, 1_000, Some(Discount::Amount(1_001)))], None).is_err());
        assert!(price_cart(&[line(1_000, 1_000, Some(Discount::Percent(10_001)))], None).is_err());
        assert!(price_cart(&[line(1_000, 1_000, Some(Discount::Amount(-1)))], None).is_err());
        assert!(price_cart(&[line(1_000, 0, None)], None).is_err());
        assert!(price_cart(&[line(-1, 1_000, None)], None).is_err());
        assert!(price_cart(&[line(1_000, 1_000, None)], Some(Discount::Amount(1_001))).is_err());
    }

    #[test]
    fn full_discount_gives_zero_total() {
        let (_, t) = price_cart(&[line(1_000, 1_000, None)], Some(Discount::Percent(10_000))).unwrap();
        assert_eq!(t.total, 0);
    }
}
