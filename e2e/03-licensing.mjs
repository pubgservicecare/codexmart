// End-to-end test 3: free trial, offline activation and the License Generator.
//
// Drives the real CODE X MART app (built with the `e2e` feature) and the real
// License Generator through WebView2's CDP port. The e2e build reads:
//   CODEXMART_LICENSE_DIR        all license state copies in one throwaway folder
//   CODEXMART_TEST_CLOCK_OFFSET  seconds added to the clock (day 25, day 31, ...)
//   CODEXMART_TEST_DEVICE        a simulated computer (device replacement)
//   CODEXMART_TEST_TRUSTED_KEY   the public key of the generator's test key
// The internet is blocked for every web view (dead proxy).
//
//   node e2e/03-licensing.mjs <app.exe> <generator.exe> <work dir> <screenshots dir>
import { chromium } from 'playwright-core';
import { spawn, execSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';

const [appExe, genExe, work, shots] = process.argv.slice(2);
const DAY = 86_400;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
fs.rmSync(work, { recursive: true, force: true });
fs.mkdirSync(shots, { recursive: true });
const dataDir = path.join(work, 'data');
const licenseDir = path.join(work, 'license');
const genDir = path.join(work, 'generator');

const results = [];
const check = (name, ok, detail = '') => {
  results.push(ok);
  console.log(`${ok ? 'PASS' : 'FAIL'}  ${name}${detail ? ' — ' + detail : ''}`);
};
const external = [];
const consoleErrors = [];

async function connect(proc, port) {
  let browser;
  for (let i = 0; i < 120 && !browser; i++) {
    try {
      browser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`);
    } catch {
      await sleep(500);
    }
  }
  if (!browser) throw new Error(`no CDP on ${port}`);
  const ctx = browser.contexts()[0];
  let page = ctx.pages()[0];
  for (let i = 0; !page && i < 40; i++) {
    await sleep(250);
    page = ctx.pages()[0];
  }
  page.setDefaultTimeout(20000);
  page.on('request', (r) => {
    const u = r.url();
    if (!/^(ipc:|http:\/\/ipc\.localhost|http:\/\/tauri\.localhost|tauri:|data:|blob:)/.test(u)) external.push(u);
  });
  page.on('console', (m) => m.type() === 'error' && consoleErrors.push(m.text()));
  page.on('pageerror', (e) => consoleErrors.push(e.message));
  return { proc, browser, page };
}

const OFFLINE = '--proxy-server=http://127.0.0.1:9 --proxy-bypass-list=<-loopback>';
let trustedKey = '';

async function launchApp({ days = 0, device = 'shop-pc-1', port = 9351 } = {}) {
  const proc = spawn(appExe, [], {
    env: {
      ...process.env,
      CODEXMART_DATA_DIR: dataDir,
      CODEXMART_LICENSE_DIR: licenseDir,
      CODEXMART_TEST_CLOCK_OFFSET: String(days * DAY),
      CODEXMART_TEST_DEVICE: device,
      CODEXMART_TEST_TRUSTED_KEY: trustedKey,
      WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${port} ${OFFLINE}`,
    },
    stdio: 'ignore',
  });
  return connect(proc, port);
}

async function launchGenerator(dir = genDir, port = 9352) {
  const proc = spawn(genExe, [], {
    env: { ...process.env, CXM_LICENSEGEN_DIR: dir, WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${port} ${OFFLINE}` },
    stdio: 'ignore',
  });
  return connect(proc, port);
}

async function close(h) {
  if (!h) return;
  try {
    await h.browser.close();
  } catch {}
  try {
    execSync(`taskkill /PID ${h.proc.pid}`, { stdio: 'ignore' });
  } catch {}
  for (let i = 0; i < 40; i++) {
    try {
      process.kill(h.proc.pid, 0);
      await sleep(250);
    } catch {
      return;
    }
  }
  try {
    execSync(`taskkill /F /PID ${h.proc.pid}`, { stdio: 'ignore' });
  } catch {}
  await sleep(500);
}

/** Calls a backend command directly (for steps behind native file dialogs). */
const invoke = (page, cmd, args = {}) =>
  page.evaluate(
    ([c, a]) =>
      window.__TAURI_INTERNALS__.invoke(c, a).then(
        (r) => ({ ok: true, r }),
        (e) => ({ ok: false, e }),
      ),
    [cmd, args],
  );

const shot = (page, name) => page.screenshot({ path: path.join(shots, `licensing-${name}.png`) });

async function login(page) {
  await page.getByRole('heading', { name: 'Sign in' }).waitFor({ timeout: 30000 });
  await page.getByLabel('Username').fill('owner');
  await page.getByLabel('Password').fill('owner123');
  await page.getByRole('button', { name: 'Sign in', exact: true }).click();
}

// Re-computes check symbols so a changed key passes the typing checks and
// only the signature can catch it.
const ALPHABET = '23456789ABCDEFGHJKLMNPQRSTUVWXYZ';
function luhn32(values) {
  let factor = 2;
  let sum = 0;
  for (let i = values.length - 1; i >= 0; i--) {
    const addend = factor * values[i];
    sum += Math.floor(addend / 32) + (addend % 32);
    factor = 3 - factor;
  }
  return (32 - (sum % 32)) % 32;
}
function forge(key, groupNo) {
  const parts = key.split('-');
  const g = parts[groupNo].split('').map((c) => ALPHABET.indexOf(c));
  g[0] = (g[0] + 1) % 32;
  g[4] = luhn32([(19 + groupNo) % 32, ...g.slice(0, 4)]);
  parts[groupNo] = g.map((v) => ALPHABET[v]).join('');
  return parts.join('-');
}

async function issue(gen, { request, name, edition = 'year1', kind = 'new', issuedOn, replaces }) {
  const page = gen.page;
  await page.getByRole('button', { name: 'Issue license', exact: true }).click();
  await page.getByLabel('Request code').fill(request);
  await page.getByRole('button', { name: 'Check request' }).click();
  await page.getByTestId('request-device').waitFor();
  await page.getByLabel('Customer name or reference').fill(name);
  await page.getByLabel('Customer phone').fill('0300-1234567');
  await page.getByLabel('License type').selectOption(edition);
  if (issuedOn) await page.getByLabel('Issue date').fill(issuedOn);
  await page.getByLabel('Kind').selectOption(kind);
  if (replaces) await page.getByLabel('License being moved (serial)').fill(replaces);
  await page.getByRole('button', { name: 'Review license' }).click();
  await page.getByRole('button', { name: 'Generate activation key' }).click();
  const key = (await page.getByTestId('activation-key').innerText()).trim();
  const serial = (await page.getByTestId('serial').innerText()).trim();
  await page.getByRole('button', { name: 'Issue another license' }).click();
  return { key, serial };
}

async function typeKey(page, key) {
  const box = page.getByLabel('Activation key');
  await box.fill(key);
  await sleep(400);
}

let app;
let gen;
try {
  // ------------------------------------------------ owner sets up the generator
  gen = await launchGenerator();
  await gen.page.getByText('Create a new signing key').waitFor({ timeout: 30000 });
  await gen.page.getByLabel('Owner passphrase').fill('short');
  await gen.page.getByLabel('Repeat passphrase').fill('short');
  await gen.page.getByRole('button', { name: 'Create signing key' }).click();
  await gen.page.getByText('at least 10 characters').first().waitFor();
  check('Generator refuses a short passphrase', true);
  await gen.page.getByLabel('Owner passphrase').fill('e2e owner passphrase');
  await gen.page.getByLabel('Repeat passphrase').fill('e2e owner passphrase');
  await gen.page.getByRole('button', { name: 'Create signing key' }).click();
  await gen.page.getByText('Save a backup of your signing key before issuing licenses').waitFor();
  check('Signing key created; generator asks for a key backup first', true);
  await gen.page.getByRole('button', { name: 'Signing key', exact: true }).click();
  trustedKey = (await gen.page.getByTestId('public-key').innerText()).trim();
  check('Public key shown for building into CODE X MART', /^[0-9a-f]{64}$/.test(trustedKey));
  const notTrusted = await gen.page.getByText('is not built into this version of CODE X MART').isVisible();
  check('Generator warns that a new key is not trusted by this build', notTrusted);
  const backupFile = path.join(work, 'owner-key-backup.cxkey');
  const b = await invoke(gen.page, 'gen_backup_key', { path: backupFile });
  check('Key backup saved and verified', b.ok && fs.existsSync(backupFile), b.ok ? '' : JSON.stringify(b.e));
  const backupText = fs.readFileSync(backupFile, 'utf8');
  check('Backup file holds only encrypted key material', backupText.includes('argon2id') && backupText.includes('ciphertext'));
  await gen.page.getByRole('button', { name: 'Lock' }).click();
  await gen.page.getByLabel('Owner passphrase').fill('wrong passphrase!');
  await gen.page.getByRole('button', { name: 'Unlock' }).click();
  await gen.page.getByText('Wrong passphrase').waitFor();
  await gen.page.getByLabel('Owner passphrase').fill('e2e owner passphrase');
  await gen.page.getByRole('button', { name: 'Unlock' }).click();
  await gen.page.getByRole('button', { name: 'Issue license', exact: true }).waitFor();
  check('Lock, wrong passphrase refused, unlock', true);

  // ------------------------------------------------ day 1: setup starts the trial
  app = await launchApp({ days: 0 });
  await app.page.getByText('Welcome to CODE X MART').waitFor({ timeout: 30000 });
  // The WebView2 profile is shared with earlier runs: forget any reminder date.
  await app.page.evaluate(() => localStorage.removeItem('cxm.licenseReminderShownOn'));
  await app.page.getByLabel('Store name').fill('Licensing Test Mart');
  await app.page.getByRole('button', { name: 'Continue' }).click();
  await app.page.getByLabel('Full name').fill('Owner');
  await app.page.getByLabel('Username').fill('owner');
  await app.page.locator('input[type=password]').nth(0).fill('owner123');
  await app.page.locator('input[type=password]').nth(1).fill('owner123');
  await app.page.getByRole('button', { name: 'Create account' }).click();
  await app.page.getByLabel('I have written down the recovery key').check();
  await app.page.getByRole('button', { name: 'Start using CODE X MART' }).click();
  await app.page.getByRole('link', { name: 'Products' }).waitFor();
  let st = (await invoke(app.page, 'license_status')).r;
  check('Day 1: trial started at setup', st.state === 'trial' && st.trialDay === 1, `${st.state} day ${st.trialDay}, ends ${st.trialLastDay}`);
  check('Day 1: no reminder', !st.reminder && !(await app.page.getByTestId('license-chip').isVisible()));
  // Some shop data to check later.
  await app.page.getByRole('link', { name: 'Products' }).click();
  await app.page.getByRole('button', { name: 'New product' }).first().click();
  await app.page.getByLabel('Product name').fill('Trial Cream');
  await app.page.getByLabel('Selling price').fill('300');
  await app.page.getByLabel('Cost price').fill('200');
  await app.page.getByLabel('Quantity on hand').fill('5');
  await app.page.getByRole('button', { name: 'Add product' }).click();
  await app.page.getByRole('status').filter({ hasText: 'Product added' }).first().waitFor();
  await app.page.getByRole('link', { name: 'Billing' }).click();
  const search = app.page.getByPlaceholder(/Scan a barcode/);
  await search.fill('trial cream');
  await app.page.getByRole('button', { name: /Trial Cream/ }).first().waitFor();
  await search.press('Enter');
  await app.page.getByRole('button', { name: /^Pay Rs/ }).waitFor();
  await app.page.keyboard.press('F9');
  await app.page.getByLabel('Cash received').fill('300');
  await app.page.keyboard.press('Enter');
  await app.page.getByText('Sale completed').waitFor();
  await app.page.keyboard.press('Enter');
  check('Day 1: full access (product added, sale completed)', true);
  await close(app);

  // ------------------------------------------------ day 24: still quiet
  app = await launchApp({ days: 23 });
  await login(app.page);
  await app.page.getByRole('link', { name: 'Billing' }).waitFor();
  st = (await invoke(app.page, 'license_status')).r;
  check('Day 24: no reminder yet', st.trialDay === 24 && !st.reminder && !(await app.page.getByRole('dialog').isVisible()));
  await close(app);

  // ------------------------------------------------ day 25: reminder once a day
  app = await launchApp({ days: 24 });
  await login(app.page);
  await app.page.getByRole('dialog').getByText('Your free trial expires in 5 days').first().waitFor();
  await shot(app.page, '01-day25-reminder');
  const dialogText = await app.page.getByRole('dialog').innerText();
  check('Day 25: reminder shows days left, end date and owner', dialogText.includes('Hamad Mansha') && dialogText.includes('+92 348 6825408') && dialogText.includes('Contact on WhatsApp'));
  await app.page.getByRole('button', { name: 'Remind me tomorrow' }).click();
  check('Day 25: header chip', (await app.page.getByTestId('license-chip').innerText()).includes('5 days left'));
  await app.page.getByRole('link', { name: 'Billing' }).click();
  await app.page.getByPlaceholder(/Scan a barcode/).waitFor();
  check('Day 25: billing still works', true);
  await app.page.getByRole('button', { name: 'Sign out' }).click();
  await login(app.page);
  await app.page.getByRole('link', { name: 'Billing' }).waitFor();
  await sleep(800);
  check('Day 25: reminder not repeated the same day', !(await app.page.getByRole('dialog').isVisible()));
  await close(app);

  for (const [days, label] of [
    [25, '4 days'],
    [28, 'tomorrow'],
    [29, 'today'],
  ]) {
    app = await launchApp({ days });
    await login(app.page);
    st = (await invoke(app.page, 'license_status')).r;
    await app.page.getByRole('dialog').waitFor();
    const text = await app.page.getByRole('dialog').innerText();
    check(`Day ${days + 1}: reminder "${label}"`, st.allowed && text.includes(`expires ${label === '4 days' ? 'in 4 days' : label}`), st.headline);
    await close(app);
  }

  // ------------------------------------------------ day 31: activation required
  app = await launchApp({ days: 30 });
  await login(app.page);
  await app.page.getByText('Your free trial has expired.').first().waitFor();
  await shot(app.page, '02-day31-activation-screen');
  const screen = await app.page.locator('body').innerText();
  check(
    'Day 31: activation screen with owner details',
    screen.includes('Contact the software owner to activate your license.') && screen.includes('Hamad Mansha') && screen.includes('+92 348 6825408'),
  );
  check('Day 31: billing is not reachable', !(await app.page.getByRole('link', { name: 'Billing' }).isVisible()));
  const blocked = await invoke(app.page, 'products_list', { filter: {} });
  check('Day 31: backend refuses business commands', !blocked.ok && blocked.e.code === 'LICENSE_REQUIRED', JSON.stringify(blocked.e?.code));
  await app.page.getByRole('button', { name: 'Back up now' }).click();
  await app.page.getByRole('status').filter({ hasText: 'Backup created and verified' }).first().waitFor();
  check('Day 31: backup still works', true);
  const backups = (await invoke(app.page, 'backup_status')).r.files;
  const day31Backup = backups[0].path;
  await app.page.getByRole('button', { name: 'Generate / View License Request Code' }).click();
  const requestA = (await app.page.getByTestId('request-code').innerText()).trim();
  const deviceA = (await app.page.getByTestId('device-id').innerText()).trim();
  check('Request code shown with device ID', /^CXMR(-[2-9A-HJ-NP-Z]{5}){8}$/.test(requestA), `${requestA} · ${deviceA}`);

  // Owner issues keys.
  const good = await issue(gen, { request: requestA, name: 'Licensing Test Mart' });
  check('Generator decoded the request and issued a key', good.key.startsWith('CXMA-') && good.serial.startsWith('CXMT-'), good.serial);
  const old = await issue(gen, {
    request: requestA,
    name: 'Licensing Test Mart',
    edition: 'month1',
    kind: 'reissue',
    issuedOn: new Date(Date.now() - 60 * DAY * 1000).toLocaleDateString('en-CA'),
  });
  await shot(gen.page, '03-generator-issue');

  // Typing mistake.
  const parts = good.key.split('-');
  parts[10] = parts[10][0] === 'K' ? 'L' + parts[10].slice(1) : 'K' + parts[10].slice(1);
  await typeKey(app.page, parts.join('-'));
  const typo = await app.page.getByText('Typing mistake in group 10').isVisible();
  const disabled = await app.page.getByRole('button', { name: 'Activate License' }).isDisabled();
  check('Typing mistake found by the checksum (group 10), Activate disabled', typo && disabled);
  // Valid checksums, forged signature.
  await typeKey(app.page, forge(good.key, 10));
  await app.page.getByRole('button', { name: 'Activate License' }).click();
  await app.page.getByRole('alert').getByText('not genuine').waitFor();
  check('Forged key with valid checksums rejected (signature)', true);
  // Expired key.
  await typeKey(app.page, old.key);
  await app.page.getByRole('button', { name: 'Activate License' }).click();
  await app.page.getByRole('alert').getByText('expired on').waitFor();
  check('Expired key rejected', true);
  // A request code typed as a key.
  await typeKey(app.page, requestA);
  check('Request code typed as key is explained', await app.page.getByText('This is a request code, not an activation key').isVisible());
  // The real key.
  await typeKey(app.page, good.key.toLowerCase().replaceAll('-', ' '));
  await app.page.getByRole('button', { name: 'Activate License' }).click();
  await app.page.getByRole('link', { name: 'Billing' }).waitFor();
  await shot(app.page, '04-activated');
  st = (await invoke(app.page, 'license_status')).r;
  check('Valid key activates offline', st.state === 'licensed' && st.license.serial === good.serial, `${st.license?.editionLabel} until ${st.license?.expiresOn}`);
  const sales = (await invoke(app.page, 'sales_list', { filter: {} })).r;
  const products = (await invoke(app.page, 'products_list', { filter: {} })).r;
  check('Shop data intact after expiry and activation', sales.total === 1 && products.items.some((p) => p.name === 'Trial Cream'));
  await close(app);

  // ------------------------------------------------ restart, restore, clock
  app = await launchApp({ days: 31 });
  await login(app.page);
  await app.page.getByRole('link', { name: 'Billing' }).waitFor();
  check('License still valid after restart', (await invoke(app.page, 'license_status')).r.state === 'licensed');
  const restore = await invoke(app.page, 'backup_restore', { path: day31Backup, confirm: 'RESTORE' });
  check('Restored the day-31 backup (made before activation)', restore.ok, restore.ok ? '' : JSON.stringify(restore.e));
  // Restore signs everyone out; the UI notices on its next call.
  await app.page.reload();
  await login(app.page);
  await app.page.getByRole('link', { name: 'Billing' }).waitFor();
  st = (await invoke(app.page, 'license_status')).r;
  check('Restore does not remove the license or restart the trial', st.state === 'licensed' && st.trialStartedOn !== null);
  await close(app);

  app = await launchApp({ days: 20 });
  await login(app.page);
  await app.page.getByRole('link', { name: 'Billing' }).waitFor();
  st = (await invoke(app.page, 'license_status')).r;
  check('Clock set back: detected, warned, no time gained', st.clockWarning !== null && st.state === 'licensed' && (await app.page.getByText('Check date & time').isVisible()), st.clockWarning);
  await close(app);

  // ------------------------------------------------ device replacement
  app = await launchApp({ days: 32, device: 'shop-pc-2' });
  await login(app.page);
  await app.page.getByText('Your free trial has expired.').first().waitFor();
  check('New motherboard: license does not follow, trial not restarted', true);
  await typeKey(app.page, good.key);
  await app.page.getByRole('button', { name: 'Activate License' }).click();
  await app.page.getByRole('alert').getByText('different computer').waitFor();
  check('Key of the old computer refused on the new one', true);
  await app.page.getByRole('button', { name: 'Generate / View License Request Code' }).click();
  const requestB = (await app.page.getByTestId('request-code').innerText()).trim();
  const deviceB = (await app.page.getByTestId('device-id').innerText()).trim();
  check('New computer has a new device ID', deviceB !== deviceA, `${deviceA} → ${deviceB}`);
  await gen.page.getByRole('button', { name: 'License history', exact: true }).click();
  await gen.page.getByPlaceholder(/Search name/).fill(good.serial);
  await gen.page.getByRole('cell', { name: good.serial }).click();
  await gen.page.getByRole('button', { name: 'Transfer to new computer' }).click();
  await gen.page.getByLabel('Request code').fill(requestB);
  await gen.page.getByRole('button', { name: 'Check request' }).click();
  await gen.page.getByTestId('request-device').waitFor();
  await gen.page.getByRole('button', { name: 'Review license' }).click();
  await gen.page.getByRole('button', { name: 'Generate activation key' }).click();
  const moved = (await gen.page.getByTestId('activation-key').innerText()).trim();
  await gen.page.getByRole('button', { name: 'Issue another license' }).click();
  await typeKey(app.page, moved);
  await app.page.getByRole('button', { name: 'Activate License' }).click();
  await app.page.getByRole('link', { name: 'Billing' }).waitFor();
  check('Transferred license activates the new computer', (await invoke(app.page, 'license_status')).r.state === 'licensed');
  await close(app);

  // Lifetime.
  app = await launchApp({ days: 33, device: 'shop-pc-2' });
  await login(app.page);
  await app.page.getByRole('link', { name: 'Settings' }).click();
  await app.page.getByRole('button', { name: 'License', exact: true }).click();
  await app.page.getByRole('button', { name: 'Generate / View License Request Code' }).click();
  const requestB2 = (await app.page.getByTestId('request-code').innerText()).trim();
  const lifetime = await issue(gen, { request: requestB2, name: 'Licensing Test Mart', edition: 'lifetime', kind: 'reissue' });
  await typeKey(app.page, lifetime.key);
  await app.page.getByRole('button', { name: 'Activate License' }).click();
  await app.page.getByRole('status').filter({ hasText: 'License activated' }).first().waitFor();
  await close(app);
  app = await launchApp({ days: 4000, device: 'shop-pc-2' });
  await login(app.page);
  await app.page.getByRole('link', { name: 'Billing' }).waitFor();
  st = (await invoke(app.page, 'license_status')).r;
  check('Lifetime license still valid 11 years later', st.state === 'licensed' && st.license.expiresOn === null);
  await close(app);

  // ------------------------------------------------ generator history and restore
  await gen.page.getByRole('button', { name: 'License history', exact: true }).click();
  await gen.page.getByPlaceholder(/Search name/).fill('Licensing Test');
  await sleep(500);
  const rows = await gen.page.locator('tbody tr').count();
  const statuses = await gen.page.locator('tbody').innerText();
  check('History lists every issued license with status', rows === 4 && statuses.includes('transferred') && statuses.includes('superseded'), `${rows} rows`);
  const csv = path.join(work, 'history.csv');
  const exp = await invoke(gen.page, 'gen_export_history', { path: csv });
  check('History exported to CSV', exp.ok && exp.r === 4 && fs.readFileSync(csv, 'utf8').includes(good.serial));
  await close(gen);
  gen = await launchGenerator(path.join(work, 'generator-new-pc'), 9353);
  await gen.page.getByText('Restore from a key backup').waitFor({ timeout: 30000 });
  const restored = await invoke(gen.page, 'gen_restore_key', { path: backupFile, passphrase: 'e2e owner passphrase' });
  const after = (await invoke(gen.page, 'gen_status')).r;
  check('Key restored from backup on another computer', restored.ok && after.key.publicKey === trustedKey);

  check('No request left the computer', external.length === 0, external.slice(0, 3).join(' '));
  check('No console errors', consoleErrors.length === 0, consoleErrors.slice(0, 3).join(' | '));
} catch (e) {
  check('Unexpected error', false, e.message.split('\n')[0]);
  if (app) await shot(app.page, 'zz-failure-app').catch(() => {});
  if (gen) await shot(gen.page, 'zz-failure-generator').catch(() => {});
} finally {
  await close(app);
  await close(gen);
  const passed = results.filter(Boolean).length;
  console.log(`\n${passed}/${results.length} checks passed`);
  process.exitCode = passed === results.length ? 0 : 1;
}
