// End-to-end test 1: first start, setup, catalog, sales, returns, reports.
// Drives the real CODE X MART executable through WebView2's CDP port.
import { chromium } from 'playwright-core';
import { spawn, execSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';

const [exe, dataDir, shots] = process.argv.slice(2);
const PORT = 9333;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
fs.rmSync(dataDir, { recursive: true, force: true });
fs.mkdirSync(shots, { recursive: true });

const app = spawn(exe, [], {
  env: { ...process.env, CODEXMART_DATA_DIR: dataDir, WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${PORT}` },
  stdio: 'ignore',
  detached: false,
});
console.log('app pid', app.pid);

const results = [];
const check = (name, ok, detail = '') => {
  results.push({ name, ok, detail });
  console.log(`${ok ? 'PASS' : 'FAIL'}  ${name}${detail ? ' — ' + detail : ''}`);
};

let browser;
for (let i = 0; i < 80 && !browser; i++) {
  try {
    browser = await chromium.connectOverCDP(`http://127.0.0.1:${PORT}`);
  } catch {
    await sleep(500);
  }
}
if (!browser) throw new Error('Could not connect to the WebView2 debug port');
const ctx = browser.contexts()[0];
let page = ctx.pages()[0];
for (let i = 0; !page && i < 40; i++) {
  await sleep(250);
  page = ctx.pages()[0];
}
page.setDefaultTimeout(15000);

const consoleErrors = [];
const external = [];
page.on('console', (m) => m.type() === 'error' && consoleErrors.push(m.text()));
page.on('pageerror', (e) => consoleErrors.push(e.message));
page.on('request', (r) => {
  const u = r.url();
  if (!/^(ipc:|http:\/\/ipc\.localhost|http:\/\/tauri\.localhost|tauri:|data:|blob:)/.test(u)) external.push(u);
});
const shot = (name) => page.screenshot({ path: path.join(shots, `${name}.png`) });
const toast = (text) => page.getByRole('status').filter({ hasText: text }).first().waitFor();
// Printing opens the native dialog, which automation cannot close; record
// what would have been printed instead.
const stubPrint = () =>
  page.evaluate(() => {
    window.__prints = [];
    window.print = () => {
      const root = document.getElementById('print-root');
      window.__prints.push({ printing: document.body.classList.contains('printing'), html: root ? root.innerHTML.length : 0, text: root ? root.innerText : '' });
      setTimeout(() => window.dispatchEvent(new Event('afterprint')), 10);
    };
  });

try {
  // ---------------------------------------------------------------- setup
  await page.getByText('Welcome to CODE X MART').first().waitFor({ timeout: 30000 });
  await shot('01-setup-store');
  await page.getByLabel('Store name').fill('Al-Noor Mart');
  await page.getByLabel('Phone').fill('0300-1234567');
  await page.getByLabel('Address').fill('Main Bazaar, Lahore');
  await page.getByRole('button', { name: 'Continue' }).click();
  await page.getByLabel('Full name').fill('Hamad Owner');
  await page.getByLabel('Username').fill('owner');
  await page.locator('input[type=password]').nth(0).fill('owner123');
  await page.locator('input[type=password]').nth(1).fill('owner123');
  await page.getByRole('button', { name: 'Create account' }).click();
  await page.getByText('Save your recovery key').first().waitFor();
  const key = (await page.locator('code').first().innerText()).trim();
  check('Recovery key shown once in XXXXX-XXXXX-XXXXX-XXXXX format', /^[A-Z0-9]{5}(-[A-Z0-9]{5}){3}$/.test(key), key);
  fs.writeFileSync(path.join(shots, 'recovery-key.txt'), key);
  await shot('02-setup-recovery-key');
  await page.getByLabel('I have written down the recovery key').check();
  await page.getByRole('button', { name: 'Start using CODE X MART' }).click();
  await page.getByText('Good', { exact: false }).first().waitFor();
  await shot('03-dashboard-empty');
  check('Setup completes and opens the dashboard', true);

  // ------------------------------------------------------------- products
  const newProduct = async (p) => {
    await page.getByRole('link', { name: 'Products' }).click();
    await page.getByRole('button', { name: 'New product' }).first().click();
    await page.getByLabel('Product name').fill(p.name);
    await page.getByLabel('Category').selectOption({ label: p.category });
    if (p.color) await page.getByLabel('Colour / shade').fill(p.color);
    if (p.size) await page.getByLabel('Size').fill(p.size);
    await page.getByLabel('Selling price').fill(p.price);
    await page.getByLabel('Cost price').fill(p.cost);
    if (p.generate) await page.getByRole('button', { name: 'Generate' }).click();
    if (p.barcode) await page.getByLabel('Barcode').fill(p.barcode);
    if (p.batches) await page.getByRole('switch', { name: /Track batches/ }).click();
    await page.getByLabel('Quantity on hand').fill(p.qty);
    if (p.batches) {
      await page.getByLabel('Batch no.').fill(p.batchNo);
      await page.getByLabel('Expiry', { exact: true }).fill(p.expiry);
    }
    if (p.shot) await shot(p.shot);
    await page.getByRole('button', { name: 'Add product' }).click();
    await toast('Product added');
  };
  await newProduct({ name: 'Rose Glow Whitening Cream 50ml', category: 'Skincare & Creams', price: '450', cost: '300', qty: '24', generate: true, shot: '04-product-form' });
  await newProduct({ name: 'Leather Handbag', category: 'Bags', color: 'Black', price: '2500', cost: '1800', qty: '5', barcode: '4006381333931' });
  await newProduct({ name: 'Vitamin C Serum 30ml', category: 'Skincare & Creams', price: '1200', cost: '700', qty: '10', batches: true, batchNo: 'VC-01', expiry: '2027-03-31' });
  await newProduct({ name: 'Matte Lipstick', category: 'Makeup & Cosmetics', color: 'Ruby Red', price: '850', cost: '500', qty: '12' });
  await page.getByRole('link', { name: 'Products' }).click();
  await page.getByText('Rose Glow Whitening Cream 50ml').first().waitFor();
  await shot('05-products');
  check('Four products created (one with generated EAN-13, one batch tracked)', (await page.locator('tbody tr').count()) === 4);

  // Duplicate barcode must be rejected by the backend.
  await page.getByRole('button', { name: 'New product' }).first().click();
  await page.getByLabel('Product name').fill('Duplicate');
  await page.getByLabel('Barcode').fill('4006381333931');
  await page.getByRole('button', { name: 'Add product' }).click();
  await toast('already used');
  check('Duplicate barcode rejected with a clear message', true);
  await page.keyboard.press('Escape');

  // -------------------------------------------------------------- billing
  await page.getByRole('link', { name: 'Billing' }).click();
  const search = page.getByPlaceholder(/Scan a barcode/);
  await search.fill('rose');
  await page.getByRole('button', { name: /Rose Glow Whitening Cream/ }).first().waitFor();
  await search.press('Enter');
  await search.fill('2*4006381333931');
  await search.press('Enter');
  await page.getByText('Leather Handbag — Black').first().waitFor();
  await page.keyboard.press('F6');
  await page.getByRole('dialog').getByRole('textbox').fill('5');
  await page.getByRole('button', { name: 'Apply' }).click();
  await page.waitForTimeout(600);
  await shot('06-billing-cart');
  const totalText = await page.getByRole('button', { name: /^Pay / }).innerText();
  // (450 + 2 x 2500) = 5450, minus 5% = 5177.50
  check('Bill total computed by backend (2 lines, 5% bill discount)', totalText.includes('5,177.50'), totalText.replace(/\s+/g, ' '));
  await page.getByRole('button', { name: /^Pay Rs/ }).waitFor();
  await page.keyboard.press('F9');
  await page.getByText('Amount due').first().waitFor();
  await page.getByLabel('Cash received').fill('6000');
  await page.getByText('Change to return').first().waitFor();
  await shot('07-payment');
  await page.keyboard.press('Enter');
  await page.getByText('Sale completed').first().waitFor();
  await shot('08-receipt');
  const receipt = await page.getByRole('dialog').innerText();
  check('Receipt shows invoice INV-000001 and change 822.50', receipt.includes('INV-000001') && receipt.includes('822.50'));
  await stubPrint();
  await page.keyboard.press('p');
  await page.waitForTimeout(500);
  const prints = await page.evaluate(() => window.__prints);
  check('Print renders the receipt into the print area', prints.length === 1 && prints[0].printing && prints[0].text.includes('INV-000001'), JSON.stringify(prints.map((p) => p.html)));
  await page.keyboard.press('Enter');

  // Split payment (card + cash).
  await search.fill('lipstick');
  await page.getByRole('button', { name: /Matte Lipstick/ }).first().waitFor();
  await search.press('Enter');
  await page.getByRole('button', { name: /^Pay Rs/ }).waitFor();
  await page.keyboard.press('F9');
  await page.getByRole('button', { name: 'Split payment' }).click();
  await page.getByLabel('Cash received').fill('500');
  await page.getByLabel('Amount', { exact: true }).last().fill('350');
  await page.getByRole('dialog').getByRole('button', { name: 'Complete sale' }).click();
  await page.getByText('Sale completed').first().waitFor();
  await page.keyboard.press('Enter');
  check('Split card + cash payment completes', true);

  // Credit sale to a new customer.
  await page.keyboard.press('F4');
  await page.getByRole('button', { name: 'New' }).click();
  await page.getByLabel('Customer name').fill('Ayesha Khan');
  await page.getByLabel('Phone').fill('0311-7654321');
  await page.getByRole('button', { name: 'Add customer' }).click();
  await page.getByText('Ayesha Khan').first().waitFor();
  await search.fill('serum');
  await page.getByRole('button', { name: /Vitamin C Serum/ }).first().waitFor();
  await search.press('Enter');
  await page.getByRole('button', { name: /^Pay Rs/ }).waitFor();
  await page.keyboard.press('F9');
  await page.getByLabel('Cash received').fill('500');
  await page.getByText('On credit to Ayesha Khan').first().waitFor();
  await shot('09-credit-payment');
  await page.keyboard.press('Enter');
  await page.getByText('Sale completed').first().waitFor();
  await page.keyboard.press('Enter');
  check('Credit (udhaar) sale with partial cash completes', true);

  // Overselling is refused (3 handbags left, 4 wanted).
  await search.fill('4*4006381333931');
  await search.press('Enter');
  await page.getByText(/in stock/).first().waitFor();
  check('Stock warning shown when the bill exceeds stock', true);
  await page.getByRole('button', { name: /^Pay Rs/ }).waitFor();
  await page.keyboard.press('F9');
  await page.getByLabel('Cash received').fill('10000');
  await page.keyboard.press('Enter');
  await toast('Not enough stock');
  check('Checkout refused when stock is insufficient', true);
  await page.keyboard.press('Escape');
  await page.getByRole('button', { name: 'Clear' }).click();
  await page.getByRole('button', { name: 'Clear bill' }).click();

  // -------------------------------------------------------- sales history
  await page.getByRole('link', { name: 'Sales history' }).click();
  await page.getByText('INV-000003').first().waitFor();
  await shot('10-sales-history');
  check('Sales history lists three invoices', (await page.locator('tbody tr').count()) === 3);
  await page.getByText('INV-000001').first().click();
  await page.getByRole('button', { name: 'Return items' }).click();
  await page.getByRole('dialog').last().locator('tbody tr').first().locator('input[inputmode=decimal]').fill('1');
  await page.waitForTimeout(300);
  await shot('11-return');
  await page.getByRole('button', { name: 'Save return' }).click();
  await toast('saved');
  check('Partial return saved from the sale detail', true);
  await page.keyboard.press('Escape');

  // ------------------------------------------------------------- reports
  await page.getByRole('link', { name: 'Reports' }).click();
  await page.getByRole('button', { name: 'Profit & loss' }).click();
  await page.getByText('Profit & loss statement').first().waitFor();
  await page.waitForTimeout(500);
  await shot('12-profit-loss');
  const pl = await page.locator('.card').filter({ hasText: 'Profit & loss statement' }).innerText();
  // Net sales: 5177.50 + 850 + 1200 - refund(427.50) = 6800.00; COGS: 300 + 3600 + 500 + 700 - 300 = 4800
  check('P&L net sales 6,800.00 after the return', pl.includes('6,800.00'), '');
  check('P&L gross profit 2,000.00 from recorded cost', pl.includes('2,000.00'), '');

  // ----------------------------------------------------------- inventory
  await page.getByRole('link', { name: 'Inventory' }).click();
  await page.getByRole('button', { name: 'Verify stock records' }).click();
  await toast('consistent');
  check('Stock ledger verification passes after all operations', true);
  await shot('13-inventory');

  // ------------------------------------------------------------ dashboard
  await page.getByRole('link', { name: 'Dashboard' }).click();
  await page.getByText("Today's sales (net)").waitFor();
  await page.waitForTimeout(800);
  await shot('14-dashboard');

  // ------------------------------------------------------------ settings
  await page.getByRole('link', { name: 'Settings' }).click();
  await page.getByRole('button', { name: 'Receipt & printer' }).click();
  await page.waitForTimeout(500);
  await shot('15-settings-receipt');

  // Customer balance after the credit sale: 1200 - 500 = 700.
  await page.getByRole('link', { name: 'Customers' }).click();
  const cust = await page.locator('tbody tr').filter({ hasText: 'Ayesha Khan' }).innerText();
  check('Customer balance shows 700.00 owed', cust.includes('700.00'), cust.replace(/\s+/g, ' '));
} catch (e) {
  check('Unexpected error', false, e.message.split('\n')[0]);
  await shot('zz-error').catch(() => undefined);
} finally {
  check('No console errors in the UI', consoleErrors.length === 0, consoleErrors.slice(0, 3).join(' | '));
  check('No requests left the computer (offline)', external.length === 0, external.slice(0, 3).join(' '));
  fs.writeFileSync(path.join(shots, 'run1-results.json'), JSON.stringify(results, null, 2));
  await browser.close().catch(() => undefined);
  // Wait for the scheduled automatic backup (runs ~30 s after start), then close gracefully.
  await sleep(35000);
  try {
    execSync(`taskkill /PID ${app.pid}`);
  } catch {}
  await sleep(4000);
  const hist = fs.existsSync(path.join(dataDir, 'backup-history.jsonl')) ? fs.readFileSync(path.join(dataDir, 'backup-history.jsonl'), 'utf8') : '';
  check('Scheduled automatic backup ran and succeeded', hist.includes('"kind":"auto"') && hist.includes('"status":"success"'));
  try {
    execSync(`taskkill /F /PID ${app.pid}`, { stdio: 'ignore' });
  } catch {}
  const failed = results.filter((r) => !r.ok).length;
  console.log(`\n${results.length - failed}/${results.length} checks passed`);
  process.exitCode = failed > 0 ? 1 : 0;
}
