// End-to-end test 2 (uses the data folder left by test 1): restart and
// persistence, purchasing, expenses, cash closing, users and roles,
// backup + restore, recovery key, and a corrupted-database start.
import { chromium } from 'playwright-core';
import { spawn, execSync } from 'node:child_process';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';

const [exe, dataDir, shots] = process.argv.slice(2);
let PORT = 9334;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const results = [];
const check = (name, ok, detail = '') => {
  results.push({ name, ok, detail });
  console.log(`${ok ? 'PASS' : 'FAIL'}  ${name}${detail ? ' — ' + detail : ''}`);
};

async function launch() {
  PORT += 1;
  const app = spawn(exe, [], {
    env: { ...process.env, CODEXMART_DATA_DIR: dataDir, WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${PORT}` },
    stdio: 'ignore',
  });
  let browser;
  for (let i = 0; i < 80 && !browser; i++) {
    try {
      browser = await chromium.connectOverCDP(`http://127.0.0.1:${PORT}`);
    } catch {
      await sleep(500);
    }
  }
  if (!browser) throw new Error('no CDP');
  const ctx = browser.contexts()[0];
  let page = ctx.pages()[0];
  for (let i = 0; !page && i < 40; i++) {
    await sleep(250);
    page = ctx.pages()[0];
  }
  page.setDefaultTimeout(15000);
  return { app, browser, page };
}

async function closeGracefully(app) {
  try {
    execSync(`taskkill /PID ${app.pid}`, { stdio: 'ignore' });
  } catch {}
  for (let i = 0; i < 20; i++) {
    await sleep(500);
    try {
      process.kill(app.pid, 0);
    } catch {
      return;
    }
  }
  try {
    execSync(`taskkill /F /PID ${app.pid}`, { stdio: 'ignore' });
  } catch {}
}

const key = fs.readFileSync(path.join(shots, 'recovery-key.txt'), 'utf8').trim();
let { app, browser, page } = await launch();
const consoleErrors = [];
const watch = (p) => {
  p.on('console', (m) => m.type() === 'error' && consoleErrors.push(m.text()));
  p.on('pageerror', (e) => consoleErrors.push(e.message));
};
watch(page);
const shot = (name) => page.screenshot({ path: path.join(shots, `${name}.png`) });
const toast = (text) => page.getByRole('status').filter({ hasText: text }).first().waitFor();
const stubPrint = () =>
  page.evaluate(() => {
    window.__prints = [];
    window.print = () => {
      const root = document.getElementById('print-root');
      window.__prints.push(root ? root.innerText : '');
      setTimeout(() => window.dispatchEvent(new Event('afterprint')), 10);
    };
  });
const login = async (user, pass) => {
  await page.getByLabel('Username').fill(user);
  await page.getByLabel('Password').fill(pass);
  await page.getByRole('button', { name: 'Sign in', exact: true }).click();
};

try {
  // ------------------------------------------------- restart and sign-in
  await page.getByRole('heading', { name: 'Sign in' }).waitFor({ timeout: 30000 });
  check('Restart opens the sign-in screen (data kept, setup not repeated)', true);
  await login('owner', 'wrong-password');
  await page.getByText('Incorrect username or password.').waitFor();
  check('Wrong password is rejected', true);
  await login('owner', 'owner123');
  await page.getByText("Today's sales (net)").waitFor();
  const dash = await page.locator('main').innerText();
  check('Sales from before the restart are still there (3 bills)', dash.includes('3 bill(s)'));

  // ----------------------------------------------------------- purchasing
  await page.getByRole('link', { name: 'Suppliers' }).click();
  await page.getByRole('button', { name: 'New supplier' }).click();
  await page.getByLabel('Supplier / company name').fill('Lahore Cosmetics Traders');
  await page.getByLabel('Phone').fill('042-35550000');
  await page.getByRole('button', { name: 'Save' }).click();
  await toast('Supplier added');
  await page.getByRole('link', { name: 'Purchases' }).click();
  await page.getByRole('button', { name: 'Receive goods' }).click();
  await page.getByRole('combobox', { name: /^Supplier/ }).selectOption({ label: 'Lahore Cosmetics Traders' });
  await page.getByLabel('Supplier invoice no.').fill('LCT-501');
  await page.getByPlaceholder(/Scan or search products on the invoice/).fill('rose');
  await page.getByRole('button', { name: /Rose Glow Whitening Cream/ }).first().waitFor();
  await page.getByPlaceholder(/Scan or search products on the invoice/).press('Enter');
  const row = page.locator('tbody tr').first();
  await row.locator('input').nth(0).fill('10');
  await row.locator('input').nth(1).fill('320');
  await page.getByPlaceholder('0', { exact: true }).fill('1000');
  await shot('20-receive-goods');
  await page.getByRole('button', { name: 'Save & add to stock' }).click();
  await toast('Purchase GRN-000001 saved');
  check('Goods received from supplier with partial payment', true);
  await page.getByRole('link', { name: 'Suppliers' }).click();
  await page.getByText('Who you buy from').waitFor();
  const sup = await page.locator('tbody tr').filter({ hasText: 'Lahore Cosmetics Traders' }).innerText();
  check('Supplier balance = 3,200 purchase − 1,000 paid = 2,200', sup.includes('2,200.00'), sup.replace(/\s+/g, ' '));

  // ------------------------------------------------------------- expenses
  await page.getByRole('link', { name: 'Expenses' }).click();
  await page.getByRole('button', { name: 'Record expense' }).click();
  await page.getByLabel('Category').selectOption({ label: 'Rent' });
  await page.getByLabel('Amount').fill('1500');
  await page.getByLabel('Paid to').fill('Landlord');
  await page.getByRole('button', { name: 'Save expense' }).click();
  await toast('recorded');
  check('Expense recorded', true);

  // -------------------------------------------------------- cash closing
  await page.getByRole('link', { name: 'Cash closing' }).click();
  await page.getByLabel('Opening float').fill('1000');
  await page.getByLabel('Counted cash').fill('5000');
  await shot('21-cash-closing');
  await stubPrint();
  await page.getByRole('button', { name: /Close day/ }).click();
  await toast('Day closed');
  await page.waitForTimeout(500);
  const slips = await page.evaluate(() => window.__prints);
  check('Day closed and closing slip printed', slips.length === 1 && slips[0].includes('CASH CLOSING'));

  // ---------------------------------------------------------------- users
  await page.getByRole('link', { name: 'Settings' }).click();
  await page.getByRole('button', { name: 'Users' }).click();
  await page.getByRole('button', { name: 'Add user' }).click();
  await page.getByLabel('Full name').fill('Ali Cashier');
  await page.getByLabel('Username').fill('ali');
  await page.getByLabel('Password').fill('1234');
  await page.getByRole('button', { name: 'Save' }).click();
  await toast('User added');
  check('Cashier account created', true);

  // ------------------------------------------------------ backup + restore
  await page.getByRole('link', { name: 'Backup & restore' }).click();
  await page.getByRole('button', { name: 'Back up now' }).click();
  await toast('Backup created and verified');
  await page.waitForTimeout(500);
  await shot('22-backup');
  check('Manual backup created and verified', true);
  await page.getByRole('button', { name: 'Restore…' }).first().click();
  await page.getByText('Backup verified').waitFor();
  await page.getByLabel('Type "RESTORE" to confirm').fill('RESTORE');
  await shot('23-restore-confirm');
  await page.getByRole('button', { name: 'Replace current data' }).click();
  await page.getByRole('heading', { name: 'Sign in' }).waitFor({ timeout: 30000 });
  check('Restore completes and signs everyone out', true);
  const hist = fs.readFileSync(path.join(dataDir, 'backup-history.jsonl'), 'utf8');
  check('A pre-restore safety backup was made first', hist.includes('"kind":"pre-restore"') && hist.includes('"action":"restore"'));

  // ------------------------------------------------------- roles (cashier)
  await login('ali', '1234');
  await page.getByPlaceholder(/Scan a barcode/).waitFor();
  const navText = await page.locator('aside nav').innerText();
  check('Cashier lands on billing and cannot see reports, purchases or backup', !/Reports|Purchases|Backup/.test(navText), navText.replace(/\s+/g, ' '));
  await page.getByTitle('Sign out').click();

  // -------------------------------------------------------- recovery key
  await page.getByRole('button', { name: /Forgot the administrator password/ }).click();
  await page.getByLabel('Recovery key').fill(key);
  await page.getByLabel('Administrator username').fill('owner');
  await page.getByLabel('New password').fill('newpass1');
  await page.getByLabel('Repeat password').fill('newpass1');
  await page.getByRole('button', { name: 'Reset password' }).click();
  await toast('Password reset');
  await login('owner', 'newpass1');
  await page.getByText("Today's sales (net)").waitFor();
  check('Administrator password reset with the recovery key', true);
  const dash2 = await page.locator('main').innerText();
  check('Data after restore matches the backup (3 bills, Rs 1,500 expenses)', dash2.includes('3 bill(s)') && dash2.includes('Rs 1,500.00'));

  await page.getByRole('link', { name: 'Barcode labels' }).click();
  await page.getByPlaceholder(/Scan or search a product/).fill('rose');
  await page.getByRole('button', { name: /Rose Glow Whitening Cream/ }).first().waitFor();
  await page.getByPlaceholder(/Scan or search a product/).press('Enter');
  await page.waitForTimeout(500);
  await shot('24-labels');
  await page.getByRole('link', { name: 'Billing' }).click();
  await page.waitForTimeout(500);
  await shot('25-billing-empty');
} catch (e) {
  check('Unexpected error', false, e.message.split('\n')[0]);
  await shot('zz-error2').catch(() => undefined);
}
check('No console errors in the UI', consoleErrors.length === 0, consoleErrors.slice(0, 3).join(' | '));
await browser.close().catch(() => undefined);
await closeGracefully(app);

// ------------------------------------------------- corrupted database start
const dbFile = path.join(dataDir, 'data', 'codexmart.db');
const original = fs.readFileSync(dbFile);
const walFile = dbFile + '-wal';
check('Normal close leaves no write-ahead log (clean shutdown)', !fs.existsSync(walFile));
for (const side of ['-wal', '-shm']) fs.rmSync(dbFile + side, { force: true });
const damaged = Buffer.from(original);
damaged.fill(0x41, 0, 4096); // destroy the SQLite header and first page
fs.writeFileSync(dbFile, damaged);
const hashBefore = crypto.createHash('sha256').update(damaged).digest('hex');
({ app, browser, page } = await launch());
try {
  await page.getByText('The shop data could not be opened').waitFor({ timeout: 30000 });
  await page.screenshot({ path: path.join(shots, '26-damaged-database.png') });
  check('A damaged database shows the recovery screen', true);
  const hashAfter = crypto.createHash('sha256').update(fs.readFileSync(dbFile)).digest('hex');
  check('The damaged file was left untouched (never reset)', hashBefore === hashAfter);
  // Put the good file back while the app waits, then retry without restarting.
  fs.writeFileSync(dbFile, original);
  await page.getByRole('button', { name: 'Try opening again' }).click();
  await page.getByRole('heading', { name: 'Sign in' }).waitFor();
  check('"Try opening again" recovers without restarting once the file is readable', true);
} catch (e) {
  check('Damaged database handling', false, e.message.split('\n')[0]);
  await page.screenshot({ path: path.join(shots, 'zz-damaged.png') }).catch(() => undefined);
}
await browser.close().catch(() => undefined);
await closeGracefully(app);
fs.writeFileSync(dbFile, original);

const failed = results.filter((r) => !r.ok).length;
console.log(`\n${results.length - failed}/${results.length} checks passed`);
process.exitCode = failed > 0 ? 1 : 0;
