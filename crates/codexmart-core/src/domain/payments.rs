//! Settling a bill against the tenders the customer offers.

use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};
use crate::money::Money;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PaymentMethod {
    Cash,
    Card,
    Bank,
    Wallet,
    Cheque,
}

impl PaymentMethod {
    pub fn as_str(self) -> &'static str {
        match self {
            PaymentMethod::Cash => "cash",
            PaymentMethod::Card => "card",
            PaymentMethod::Bank => "bank",
            PaymentMethod::Wallet => "wallet",
            PaymentMethod::Cheque => "cheque",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            PaymentMethod::Cash => "Cash",
            PaymentMethod::Card => "Card",
            PaymentMethod::Bank => "Bank transfer",
            PaymentMethod::Wallet => "Mobile wallet",
            PaymentMethod::Cheque => "Cheque",
        }
    }

    pub fn parse(s: &str) -> AppResult<Self> {
        match s {
            "cash" => Ok(PaymentMethod::Cash),
            "card" => Ok(PaymentMethod::Card),
            "bank" => Ok(PaymentMethod::Bank),
            "wallet" => Ok(PaymentMethod::Wallet),
            "cheque" => Ok(PaymentMethod::Cheque),
            other => Err(AppError::validation(format!("Unknown payment method '{other}'."))),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tender {
    pub method: PaymentMethod,
    /// For cash this is the amount handed over (it may exceed the bill).
    pub amount: Money,
    #[serde(default)]
    pub reference: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppliedPayment {
    pub method: PaymentMethod,
    pub amount: Money,
    pub reference: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settlement {
    /// Amounts actually applied to the bill (cash net of change).
    pub applied: Vec<AppliedPayment>,
    pub tendered_cash: Money,
    pub change: Money,
    pub paid: Money,
    /// Unpaid remainder charged to the customer's account.
    pub credit: Money,
}

/// Change is only ever given from cash; card, bank and wallet payments
/// cannot exceed what is still due.
pub fn settle(total: Money, tenders: &[Tender]) -> AppResult<Settlement> {
    let mut non_cash = Vec::new();
    let mut tendered_cash: Money = 0;
    for t in tenders {
        if t.amount < 0 {
            return Err(AppError::validation("Payment amounts cannot be negative."));
        }
        if t.amount == 0 {
            continue;
        }
        if t.method == PaymentMethod::Cash {
            tendered_cash += t.amount;
        } else {
            non_cash.push(AppliedPayment {
                method: t.method,
                amount: t.amount,
                reference: t.reference.as_ref().map(|r| r.trim().to_string()).filter(|r| !r.is_empty()),
            });
        }
    }
    let non_cash_total: Money = non_cash.iter().map(|p| p.amount).sum();
    if non_cash_total > total {
        return Err(AppError::validation(
            "Card, bank and wallet payments cannot be more than the bill. Give change only from cash.",
        ));
    }
    let due_in_cash = total - non_cash_total;
    let cash_applied = tendered_cash.min(due_in_cash);
    let change = tendered_cash - cash_applied;
    let mut applied = non_cash;
    if cash_applied > 0 {
        applied.insert(0, AppliedPayment { method: PaymentMethod::Cash, amount: cash_applied, reference: None });
    }
    let paid = non_cash_total + cash_applied;
    Ok(Settlement { applied, tendered_cash, change, paid, credit: total - paid })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tender(method: PaymentMethod, amount: Money) -> Tender {
        Tender { method, amount, reference: None }
    }

    #[test]
    fn cash_with_change() {
        let s = settle(85_000, &[tender(PaymentMethod::Cash, 100_000)]).unwrap();
        assert_eq!(s.change, 15_000);
        assert_eq!(s.paid, 85_000);
        assert_eq!(s.credit, 0);
        assert_eq!(s.applied[0].amount, 85_000);
    }

    #[test]
    fn split_card_and_cash() {
        let s = settle(85_000, &[tender(PaymentMethod::Card, 50_000), tender(PaymentMethod::Cash, 50_000)]).unwrap();
        assert_eq!(s.change, 15_000);
        assert_eq!(s.paid, 85_000);
        assert_eq!(s.applied.iter().map(|p| p.amount).sum::<i64>(), 85_000);
    }

    #[test]
    fn partial_payment_becomes_credit() {
        let s = settle(85_000, &[tender(PaymentMethod::Cash, 30_000)]).unwrap();
        assert_eq!(s.credit, 55_000);
        assert_eq!(s.change, 0);
    }

    #[test]
    fn card_overpayment_is_rejected() {
        assert!(settle(1_000, &[tender(PaymentMethod::Card, 1_001)]).is_err());
        assert!(settle(1_000, &[tender(PaymentMethod::Cash, -1)]).is_err());
    }
}
