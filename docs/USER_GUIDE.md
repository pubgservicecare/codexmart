# CODE X MART — User guide

This guide is for the shop owner, managers and cashiers.

## Who can do what

| Role | Can |
|---|---|
| **Cashier** | Billing, sales history and reprints, customers, receiving customer payments |
| **Manager** | Everything a cashier can, plus returns, voiding today's sales, products, stock adjustments, purchases, suppliers, expenses, cash closing, reports and backups |
| **Administrator** | Everything, plus users, settings and restoring backups |

Everyone signs in with their own username, so every sale and change shows who made it.

## Keyboard shortcuts

| Key | Where | Action |
|---|---|---|
| `F1` | anywhere | Open billing |
| `F2` | billing | Focus the search / scan box |
| `F4` | billing | Choose a customer |
| `F6` | billing | Discount on the whole bill |
| `F9` | billing | Take payment |
| `↑` `↓` then `Enter` | billing search | Choose a product and add it |
| `3*` + name or scan | billing search | Add 3 units at once |
| `+` / `-` | billing, line selected | Change the quantity |
| `Delete` | billing, line selected | Remove the line |
| `Enter` | payment window | Complete the sale |
| `P` | after a sale | Print the receipt |
| `Enter` | after a sale | Start the next sale |
| `Esc` | any window | Close it |

## 1. Making a sale

1. Open **Billing** (`F1`).
2. Add products in any of these ways:
   - **scan** the barcode (no need to click first);
   - **type** part of the name, SKU, brand, size or colour and press `Enter`;
   - **click** a product tile (filter by category above the tiles).
3. Change quantities with `−` / `+` or type the number. Click the `%` button on a line for a line discount, or press `F6` for a discount on the whole bill.
4. Optional: press `F4` to attach a customer (needed for credit sales and purchase history).
5. Press `F9` (**Pay**):
   - for cash, type the amount received or click a quick amount; the **change** is shown;
   - for card, bank transfer or wallet, choose the method and add a reference;
   - **Split payment** combines methods (for example part card, part cash);
   - if a customer is attached, paying less than the total puts the rest **on credit (udhaar)**.
6. Press `Enter` to complete. The receipt appears: press `P` to print or `Enter` for the next customer.

The total on screen is calculated by the system, not typed, so it is always correct. If the app closes in the middle of a payment, the bill is still there when you reopen it. Pressing **Pay** again never charges the customer twice.

**"Not enough stock"** means the system has less stock recorded than you are selling. Record the stock with a purchase or a stock adjustment.

## 2. Sales history, reprint, void

*Sales history* lists every bill. Search by invoice number, customer name or phone, or **scan the barcode on a receipt**. Click a bill to see its details, then:

- **Reprint receipt** (marked "DUPLICATE RECEIPT") or print an **A4 invoice**;
- **Return items** (manager), see below;
- **Void sale** (manager): only for **today's** sales with no returns. The sale is cancelled completely and items go back into stock. A reason is required.

## 3. Returns and refunds

1. Open the bill in *Sales history* and click **Return items**.
2. Enter the quantity returned for each item. Untick **Back to stock** for damaged goods.
3. Choose how the money is given back: cash, card, bank, wallet, or **credit to the customer's account** (for credit customers).
4. Save. The refund amount is calculated from what the customer actually paid, including their share of any discount.

All returns are listed under **Returns**.

## 4. Customers and credit (udhaar)

- *Customers* lists customers with their balance. Click one to see the **account statement**, recent purchases, and **Receive payment**.
- Set a **credit limit** to stop a customer's balance going above a certain amount.
- Payments reduce the customer's total balance. Paying more than the balance keeps the extra as an advance.

## 5. Products

*Products* → **New product**:

- **Name**, category, brand and unit. Use a decimal unit (kg, litre, metre) for loose items.
- **Size / colour** (when *Sizes & colours* is on). For another shade or size of the same product, use the **copy** button on the product row ("Add a variant"). Each variant has its own barcode, price and stock.
- **SKU**: leave empty to number automatically (`CXM-00001`).
- **Barcode**: scan it from the product, or click **Generate** for products without one.
- **Selling price**, optional **wholesale price**, and **cost price** (manager).
- **Reorder level**: you are warned when stock falls to this number.
- **Opening stock**: the quantity you already have and what it cost.
- **Track batches and expiry dates**: for creams and cosmetics. Batches are sold earliest-expiry first.

Products are never deleted (their history must stay). Use the power button to **deactivate** a product so it no longer appears in billing.

## 6. Stock

*Inventory*:

- **Stock levels** — quantities, average cost and stock value; **Adjust** a product, or view its **History**.
- **Stock adjustment** — choose a reason:
  - *Stock count*: enter what you physically counted;
  - *Opening stock* / *Found*: adds stock;
  - *Damaged*, *Expired*, *Lost*, *Theft*, *Used in the shop*: removes stock and records the loss.
- **Stock history** — every change ever made: sales, purchases, returns and adjustments.
- **Batches & expiry** — what expires soon.
- **Verify stock records** — confirms that every product's stock matches its full history.

*Barcode labels*: choose products and the number of copies, then print price and barcode stickers.

## 7. Buying from suppliers

1. *Suppliers* → **New supplier**. Enter what you already owe as the opening balance.
2. Optional: *Purchases* → **New purchase order**, then **Save as ordered**. Print it for the supplier.
3. When goods arrive: *Purchases* → **Receive goods** (or **Receive goods** inside the purchase order):
   - choose the supplier and enter their invoice number and date;
   - add products with quantity and cost; enter batch and expiry for tracked products;
   - optionally enter a **new sale price**;
   - enter any invoice discount and any amount **paid now**;
   - **Save & add to stock**.
4. To pay a supplier later: **Pay supplier**.
5. To send goods back: open the purchase and click **Return to supplier**.

Paying a supplier is **not** an expense; it only reduces what you owe. Stock cost reaches your profit only when the goods are sold.

## 8. Expenses

*Expenses* → **Record expense**: category (rent, electricity, salaries, transport and so on, or add your own), amount, date, how it was paid, and who was paid. A wrong entry is **voided** with a reason and entered again; it is never deleted.

## 9. Closing the day

*Cash closing* shows the cash that should be in the drawer:

> opening float + cash sales + cash from customers − cash refunds − cash expenses − cash paid to suppliers

Count the drawer, enter the **counted cash**, and click **Close day & print slip**. The difference (over or short) is recorded permanently.

## 10. Reports

*Reports* (manager): sales summary, profit and loss, sales by product and category, payment methods, returns, purchases, expenses, customer and supplier balances, inventory valuation, low stock and expiry. Choose the dates at the top. **CSV** opens in Excel, and **Print** prints on A4.

How profit is calculated:

- **Net sales** = bills after discounts − refunds.
- **Gross profit** = net sales − the cost of the goods that were sold. That cost is recorded at the moment of each sale.
- **Net operating result** = gross profit − expenses ± stock adjustments.

## 11. Backups

See [BACKUP_AND_RESTORE.md](BACKUP_AND_RESTORE.md). In short: keep automatic backups on, set a USB drive for the second copy, and use **Back up to USB / folder** every week.

## 12. Settings (administrator)

- **Store**: name, address, phone and tax number on receipts; currency symbol; decimals.
- **Receipt & printer**: paper size, Windows dialog or direct thermal printing, printer, copies, automatic printing, cash drawer, header and footer, invoice prefix, test print.
- **Features & rules**: credit sales, batches and expiry, sizes and colours, wholesale prices, overselling, the cashier discount limit, expiry warning.
- **Users**: add staff, change roles, disable accounts, reset passwords.
- **Categories & units**: manage product categories, brands, units and expense categories.
- **Security & health**: database health check, new recovery key.
- **Audit log**: who did what, and when.
- **About**: version and where the data is stored.
