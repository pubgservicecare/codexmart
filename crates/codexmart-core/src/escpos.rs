//! ESC/POS rendering for direct printing to thermal receipt printers.
//!
//! Produces the raw byte stream understood by virtually all 58 mm and 80 mm
//! thermal printers (Epson TM, Xprinter, Rongta, Bixolon, ...). Text is
//! limited to ASCII so it prints identically on every code page.

use chrono::{DateTime, Local};

use crate::money::{format_amount, format_qty, Money};
use crate::repo::sales::SaleDetail;
use crate::services::settings::Settings;

const ESC: u8 = 0x1B;
const GS: u8 = 0x1D;

/// Characters per line for the paper width.
pub fn columns_for(paper: &str) -> usize {
    if paper == "58mm" {
        32
    } else {
        48
    }
}

struct Doc {
    buf: Vec<u8>,
    width: usize,
}

fn ascii(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '—' | '–' => '-',
            '‘' | '’' => '\'',
            '“' | '”' => '"',
            '→' => '>',
            c if c.is_ascii() && !c.is_ascii_control() => c,
            _ => '?',
        })
        .collect()
}

fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    for raw in text.lines() {
        let mut line = String::new();
        for word in raw.split_whitespace() {
            let mut word = word.to_string();
            while word.len() > width {
                if !line.is_empty() {
                    lines.push(std::mem::take(&mut line));
                }
                let rest = word.split_off(width);
                lines.push(word);
                word = rest;
            }
            if line.is_empty() {
                line = word;
            } else if line.len() + 1 + word.len() <= width {
                line.push(' ');
                line.push_str(&word);
            } else {
                lines.push(std::mem::replace(&mut line, word));
            }
        }
        lines.push(line);
    }
    lines
}

impl Doc {
    fn new(width: usize) -> Self {
        let mut d = Doc { buf: Vec::new(), width };
        d.buf.extend_from_slice(&[ESC, b'@']); // initialise
        d
    }

    fn align(&mut self, center: bool) {
        self.buf.extend_from_slice(&[ESC, b'a', if center { 1 } else { 0 }]);
    }

    fn bold(&mut self, on: bool) {
        self.buf.extend_from_slice(&[ESC, b'E', on as u8]);
    }

    fn size(&mut self, double: bool) {
        self.buf.extend_from_slice(&[GS, b'!', if double { 0x11 } else { 0x00 }]);
    }

    fn text(&mut self, s: &str) {
        self.buf.extend_from_slice(ascii(s).as_bytes());
        self.buf.push(b'\n');
    }

    fn wrapped(&mut self, s: &str) {
        for line in wrap(&ascii(s), self.width) {
            self.text(&line);
        }
    }

    fn rule(&mut self) {
        let line = "-".repeat(self.width);
        self.text(&line);
    }

    /// Left text and right-aligned value on one line.
    fn pair(&mut self, left: &str, right: &str) {
        let left = ascii(left);
        let right = ascii(right);
        let space = self.width.saturating_sub(right.len());
        if left.len() + 1 > space {
            self.text(&left);
            self.text(&format!("{right:>width$}", width = self.width));
        } else {
            self.text(&format!("{left:<space$}{right}"));
        }
    }

    fn feed_and_cut(&mut self) {
        self.buf.extend_from_slice(&[ESC, b'd', 4]);
        self.buf.extend_from_slice(&[GS, b'V', 66, 0]);
    }

    fn kick_drawer(&mut self) {
        self.buf.extend_from_slice(&[ESC, b'p', 0, 25, 250]);
    }
}

fn header(d: &mut Doc, s: &Settings) {
    d.align(true);
    d.bold(true);
    d.size(true);
    d.wrapped(&s.store_name);
    d.size(false);
    d.bold(false);
    for line in [&s.store_tagline, &s.store_address, &s.store_phone, &s.receipt_header] {
        if !line.trim().is_empty() {
            d.wrapped(line);
        }
    }
    if !s.store_tax_id.trim().is_empty() {
        d.wrapped(&format!("Tax ID: {}", s.store_tax_id));
    }
    d.align(false);
}

fn method_label(m: &str) -> &str {
    match m {
        "cash" => "Cash",
        "card" => "Card",
        "bank" => "Bank transfer",
        "wallet" => "Mobile wallet",
        "cheque" => "Cheque",
        other => other,
    }
}

/// Raw bytes for a sale receipt.
pub fn sale_receipt(sale: &SaleDetail, s: &Settings, reprint: bool) -> Vec<u8> {
    let width = columns_for(&s.receipt_paper);
    let money = |m: Money| format_amount(m, s.currency_decimals);
    let mut d = Doc::new(width);
    header(&mut d, s);
    d.rule();
    if sale.summary.status == "void" {
        d.align(true);
        d.bold(true);
        d.text("*** VOID ***");
        d.bold(false);
        d.align(false);
    } else if reprint {
        d.align(true);
        d.text("DUPLICATE RECEIPT");
        d.align(false);
    }
    d.pair("Invoice", &sale.summary.invoice_no);
    let when = DateTime::parse_from_rfc3339(&sale.summary.created_at)
        .map(|t| t.with_timezone(&Local).format("%d-%b-%Y %H:%M").to_string())
        .unwrap_or_else(|_| sale.summary.business_date.clone());
    d.pair("Date", &when);
    d.pair("Cashier", &sale.summary.cashier_name);
    if let Some(c) = &sale.summary.customer_name {
        d.pair("Customer", c);
    }
    d.rule();
    for item in &sale.items {
        d.wrapped(&item.product_name);
        d.pair(&format!("  {} x {}", format_qty(item.qty), money(item.unit_price)), &money(item.gross_amount));
        if item.line_discount > 0 {
            d.pair("  Discount", &format!("-{}", money(item.line_discount)));
        }
    }
    d.rule();
    d.pair("Subtotal", &money(sale.summary.subtotal));
    if sale.summary.discount_total > 0 {
        d.pair("Discount", &format!("-{}", money(sale.summary.discount_total)));
    }
    d.bold(true);
    d.size(true);
    // Double-width text halves the available columns.
    let half = Doc { buf: Vec::new(), width: width / 2 };
    let mut total_line = half;
    total_line.pair("TOTAL", &format!("{} {}", s.currency_symbol, money(sale.summary.total)));
    d.buf.extend_from_slice(&total_line.buf);
    d.size(false);
    d.bold(false);
    for p in &sale.payments {
        d.pair(&format!("Paid ({})", method_label(&p.method)), &money(p.amount));
    }
    if sale.tendered_cash > 0 {
        d.pair("Cash received", &money(sale.tendered_cash));
        d.pair("Change", &money(sale.change_amount));
    }
    if sale.summary.credit_amount > 0 {
        d.bold(true);
        d.pair("Balance due (credit)", &money(sale.summary.credit_amount));
        d.bold(false);
    }
    let items: i64 = sale.items.len() as i64;
    d.text(&format!("Items: {items}"));
    d.rule();
    if !s.receipt_footer.trim().is_empty() {
        d.align(true);
        d.wrapped(&s.receipt_footer);
        d.align(false);
    }
    d.align(true);
    d.text("Powered by CODE X MART");
    d.align(false);
    d.feed_and_cut();
    if s.open_cash_drawer && sale.tendered_cash > 0 && !reprint {
        d.kick_drawer();
    }
    d.buf
}

/// A short page to confirm that a printer works.
pub fn test_page(s: &Settings) -> Vec<u8> {
    let width = columns_for(&s.receipt_paper);
    let mut d = Doc::new(width);
    header(&mut d, s);
    d.rule();
    d.align(true);
    d.bold(true);
    d.text("PRINTER TEST");
    d.bold(false);
    d.text(&format!("Paper: {} ({width} columns)", s.receipt_paper));
    d.text(&Local::now().format("%d-%b-%Y %H:%M:%S").to_string());
    d.align(false);
    d.rule();
    d.pair("Left aligned", "Right aligned");
    d.text(&"0123456789".repeat(width / 10 + 1)[..width]);
    d.feed_and_cut();
    d.buf
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrapping_respects_width() {
        let lines = wrap("Rose Glow Whitening Cream Extra Large Family Pack", 16);
        assert!(lines.iter().all(|l| l.len() <= 16));
        assert_eq!(lines.join(" "), "Rose Glow Whitening Cream Extra Large Family Pack");
        let long = wrap("ABCDEFGHIJKLMNOPQRSTUVWXYZ", 10);
        assert!(long.iter().all(|l| l.len() <= 10));
    }

    #[test]
    fn pairs_fill_the_line() {
        let mut d = Doc { buf: Vec::new(), width: 32 };
        d.pair("Subtotal", "1,250.00");
        let line = String::from_utf8(d.buf).unwrap();
        assert_eq!(line.trim_end_matches('\n').len(), 32);
        assert!(line.starts_with("Subtotal"));
        assert!(line.trim_end().ends_with("1,250.00"));
    }

    #[test]
    fn non_ascii_is_replaced() {
        assert_eq!(ascii("Lipstick — Red"), "Lipstick - Red");
        assert_eq!(ascii("کریم"), "????");
    }

    #[test]
    fn test_page_starts_with_init_and_ends_with_cut() {
        let bytes = test_page(&Settings::default());
        assert_eq!(&bytes[..2], &[ESC, b'@']);
        assert_eq!(&bytes[bytes.len() - 4..], &[GS, b'V', 66, 0]);
    }
}
