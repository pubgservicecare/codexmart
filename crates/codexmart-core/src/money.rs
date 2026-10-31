//! Exact money and quantity arithmetic.
//!
//! Money is an integer number of minor units (paisa). Quantities are an
//! integer number of thousandths of a unit, so 1 piece = 1000 and
//! 0.25 kg = 250. No floating point is ever used for business values.

pub type Money = i64;
pub type Qty = i64;

/// Thousandths per whole unit.
pub const QTY_SCALE: i64 = 1_000;
/// 100.00 % expressed in basis points.
pub const PERCENT_SCALE: i64 = 10_000;

/// Integer division rounding half away from zero.
pub fn div_round(num: i128, den: i128) -> i128 {
    assert!(den != 0, "division by zero");
    let (num, den) = if den < 0 { (-num, -den) } else { (num, den) };
    if num >= 0 {
        (num + den / 2) / den
    } else {
        -((-num + den / 2) / den)
    }
}

/// Amount for `qty` (thousandths) at `unit_price` per whole unit.
pub fn mul_qty(unit_price: Money, qty: Qty) -> Money {
    div_round(unit_price as i128 * qty as i128, QTY_SCALE as i128) as Money
}

/// `bp` basis points of `amount`.
pub fn percent(amount: Money, bp: i64) -> Money {
    div_round(amount as i128 * bp as i128, PERCENT_SCALE as i128) as Money
}

/// `amount * part / whole`, rounded. Returns 0 when `whole` is 0.
pub fn prorate(amount: Money, part: i64, whole: i64) -> Money {
    if whole == 0 {
        return 0;
    }
    div_round(amount as i128 * part as i128, whole as i128) as Money
}

/// Per-unit price for a total spread over `qty` thousandths.
pub fn unit_price_of(total: Money, qty: Qty) -> Money {
    if qty == 0 {
        return 0;
    }
    div_round(total as i128 * QTY_SCALE as i128, qty as i128) as Money
}

/// Splits `total` across `weights` proportionally so that the parts always
/// add up to exactly `total` (largest-remainder method; ties go to the
/// earlier line). Negative weights are treated as zero.
pub fn allocate(total: Money, weights: &[i64]) -> Vec<Money> {
    let n = weights.len();
    if n == 0 {
        return Vec::new();
    }
    let sum: i128 = weights.iter().map(|&w| w.max(0) as i128).sum();
    if sum == 0 {
        let mut parts = vec![0; n];
        parts[0] = total;
        return parts;
    }
    let total = total as i128;
    let mut parts: Vec<i128> = Vec::with_capacity(n);
    let mut remainders: Vec<(i128, usize)> = Vec::with_capacity(n);
    let mut assigned: i128 = 0;
    for (i, &w) in weights.iter().enumerate() {
        let exact = total * w.max(0) as i128;
        let share = exact.div_euclid(sum);
        remainders.push((exact.rem_euclid(sum), i));
        parts.push(share);
        assigned += share;
    }
    let mut left = total - assigned;
    remainders.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    for (_, i) in remainders {
        if left <= 0 {
            break;
        }
        parts[i] += 1;
        left -= 1;
    }
    parts.into_iter().map(|p| p as Money).collect()
}

fn group_thousands(mut n: i128) -> String {
    if n == 0 {
        return "0".into();
    }
    let mut groups = Vec::new();
    while n > 0 {
        groups.push(n % 1000);
        n /= 1000;
    }
    let mut out = groups.pop().unwrap().to_string();
    while let Some(g) = groups.pop() {
        out.push_str(&format!(",{g:03}"));
    }
    out
}

/// "1,250.50" (decimals = 2) or "1,251" (decimals = 0).
pub fn format_amount(minor: Money, decimals: u8) -> String {
    let negative = minor < 0;
    let abs = (minor as i128).abs();
    let body = if decimals == 0 {
        group_thousands(div_round(abs, 100))
    } else {
        format!("{}.{:02}", group_thousands(abs / 100), abs % 100)
    };
    if negative {
        format!("-{body}")
    } else {
        body
    }
}

/// "3" for whole quantities, "1.25" otherwise.
pub fn format_qty(qty: Qty) -> String {
    let negative = qty < 0;
    let abs = qty.abs();
    let whole = abs / QTY_SCALE;
    let frac = abs % QTY_SCALE;
    let body = if frac == 0 {
        whole.to_string()
    } else {
        let f = format!("{frac:03}");
        format!("{whole}.{}", f.trim_end_matches('0'))
    };
    if negative {
        format!("-{body}")
    } else {
        body
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounding_is_half_away_from_zero() {
        assert_eq!(div_round(5, 10), 1);
        assert_eq!(div_round(4, 10), 0);
        assert_eq!(div_round(-5, 10), -1);
        assert_eq!(div_round(-4, 10), 0);
        assert_eq!(div_round(2, 3), 1);
        assert_eq!(div_round(1, 3), 0);
    }

    #[test]
    fn quantity_amounts() {
        // Rs 150.00 x 2 pieces
        assert_eq!(mul_qty(15_000, 2_000), 30_000);
        // Rs 999.99 per kg x 0.333 kg = 333.00 (332.99667)
        assert_eq!(mul_qty(99_999, 333), 33_300);
        assert_eq!(percent(10_000, 1_250), 1_250);
        assert_eq!(unit_price_of(30_000, 2_000), 15_000);
    }

    #[test]
    fn allocation_always_sums_exactly() {
        let parts = allocate(100, &[1, 1, 1]);
        assert_eq!(parts, vec![34, 33, 33]);
        assert_eq!(allocate(-5, &[1, 1]).iter().sum::<i64>(), -5);
        assert_eq!(allocate(7, &[0, 0]), vec![7, 0]);
        let weights = [12_345, 678, 9_999, 1];
        for total in [0, 1, 999, 23_022, 123_457] {
            assert_eq!(allocate(total, &weights).iter().sum::<i64>(), total);
        }
    }

    #[test]
    fn formatting() {
        assert_eq!(format_amount(125_050, 2), "1,250.50");
        assert_eq!(format_amount(125_050, 0), "1,251");
        assert_eq!(format_amount(-99, 2), "-0.99");
        assert_eq!(format_amount(123_456_789, 2), "1,234,567.89");
        assert_eq!(format_qty(3_000), "3");
        assert_eq!(format_qty(1_250), "1.25");
        assert_eq!(format_qty(-500), "-0.5");
    }
}
