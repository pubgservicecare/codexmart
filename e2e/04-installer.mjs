// End-to-end test 4: the production installer, both uninstall choices, and
// offline activation with the production signing key.
//
// Uses the REAL per-user locations (%APPDATA%\CODE X MART, the license record
// in the registry and the "CODE X MART Licensing" folders) because that is
// what the uninstaller works on. Before anything runs, the existing data,
// license records and installer registry keys are moved to <safekeeping>;
// at the end they are put back exactly as they were. If the script is
// interrupted, <safekeeping>\RESTORE.txt explains how to restore by hand.
//
//   node e2e/04-installer.mjs <setup.exe> <e2e app.exe> <generator.exe> <work dir> <safekeeping dir>
//
// Needs `cargo` (issues one production license with the pending owner key,
// see tools/license-generator: production_smoke_issue_with_pending_key).
import { chromium } from 'playwright-core';
import { spawn, spawnSync, execSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

// Absolute Windows paths: NSIS reads options case-insensitively anywhere in its
// command line, so a forward-slash path like "target/release/..." would look
// like the installer's "/R" (start the app after installing) option.
const [setup, e2eExe, genExe, work, safe] = process.argv.slice(2).map((p) => path.resolve(p));
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const APPDATA = process.env.APPDATA;
const LOCAL = process.env.LOCALAPPDATA;
const DATA = path.join(APPDATA, 'CODE X MART');
const installDir = path.join(work, 'install');
const exe = path.join(installDir, 'CODE X MART.exe');
const uninstaller = path.join(installDir, 'uninstall.exe');
const shots = path.join(work, 'screenshots');
const TITLE = 'CODE X MART Uninstall';
const OFFLINE = '--proxy-server=http://127.0.0.1:9 --proxy-bypass-list=<-loopback>';

const otherDisks = execSync(
  'powershell -NoProfile -Command "Get-CimInstance Win32_LogicalDisk -Filter \'DriveType=3\' | ForEach-Object { $_.DeviceID }"',
  { encoding: 'utf8' },
)
  .split(/\r?\n/)
  .map((d) => d.trim())
  .filter((d) => d && d.toUpperCase() !== (process.env.SystemDrive || 'C:').toUpperCase());
const LICENSE_DIRS = [
  path.join(APPDATA, 'CODE X MART Licensing'),
  path.join(LOCAL, 'CODE X MART Licensing'),
  path.join(process.env.ProgramData, 'CODE X MART Licensing'),
  ...otherDisks.map((d) => `${d}\\.codexmart-licensing`),
];
const USER_DIRS = [DATA, path.join(LOCAL, 'com.codexmart.pos'), path.join(APPDATA, 'com.codexmart.pos')];
const REG_KEYS = [
  'HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\CODE X MART',
  'HKCU\\Software\\CODE X MART',
  'HKCU\\Software\\CODE X MART Licensing',
];
const ANCHOR = path.join(APPDATA, 'CODE X MART Licensing', 'state.dat');

const results = [];
const check = (name, ok, detail = '') => {
  results.push(ok);
  console.log(`${ok ? 'PASS' : 'FAIL'}  ${name}${detail ? ' — ' + detail : ''}`);
};
const regExists = (key) => spawnSync('reg', ['query', key], { stdio: 'ignore' }).status === 0;
const running = () => execSync('tasklist /FI "IMAGENAME eq CODE X MART.exe" /NH', { encoding: 'utf8' }).includes('CODE X MART.exe');

// ------------------------------------------------------------ safekeeping
function safekeep() {
  if (fs.existsSync(safe) && fs.readdirSync(safe).length) {
    throw new Error(`${safe} is not empty: a previous run may not have restored real data. Restore it first.`);
  }
  if (running()) throw new Error('Close CODE X MART before running the installer tests.');
  fs.mkdirSync(path.join(safe, 'dirs'), { recursive: true });
  const manifest = { dirs: [], reg: [] };
  const save = () => fs.writeFileSync(path.join(safe, 'manifest.json'), JSON.stringify(manifest, null, 2));
  try {
    [...USER_DIRS, ...LICENSE_DIRS].forEach((dir, i) => {
      if (!fs.existsSync(dir)) return;
      const to = path.join(safe, 'dirs', String(i));
      // A WebView2 process that is still closing can hold a file for a moment.
      for (let attempt = 0; ; attempt++) {
        try {
          fs.renameSync(dir, to);
          break;
        } catch (e) {
          if (attempt >= 10) throw e;
          execSync('powershell -NoProfile -Command "Start-Sleep -Seconds 1"');
        }
      }
      manifest.dirs.push({ from: dir, to });
      save();
    });
  } catch (e) {
    // Put back whatever was already moved, then give up.
    for (const d of manifest.dirs.reverse()) fs.renameSync(d.to, d.from);
    fs.rmSync(safe, { recursive: true, force: true });
    throw new Error(`Could not move real data aside (${e.message}); nothing was changed.`);
  }
  REG_KEYS.forEach((key, i) => {
    const file = path.join(safe, `reg-${i}.reg`);
    const exists = regExists(key);
    if (exists) execSync(`reg export "${key}" "${file}" /y`, { stdio: 'ignore' });
    manifest.reg.push({ key, file: exists ? file : null });
  });
  fs.writeFileSync(path.join(safe, 'manifest.json'), JSON.stringify(manifest, null, 2));
  fs.writeFileSync(
    path.join(safe, 'RESTORE.txt'),
    `Real CODE X MART data moved here by e2e/04-installer.mjs.\r\nTo restore by hand: delete each "from" folder in manifest.json, move the matching "to" folder back, and double-click each reg-N.reg file.\r\n`,
  );
  return manifest;
}

/** Retries a file operation for up to 15 s (files of a closing app can be busy). */
function retry(op) {
  for (let attempt = 0; ; attempt++) {
    try {
      return op();
    } catch (e) {
      if (attempt >= 15) throw e;
      execSync('powershell -NoProfile -Command "Start-Sleep -Seconds 1"');
    }
  }
}

function restore(manifest) {
  const problems = [];
  const step = (what, op) => {
    try {
      retry(op);
    } catch (e) {
      problems.push(`${what}: ${e.message}`);
    }
  };
  for (const dir of [...USER_DIRS, ...LICENSE_DIRS]) step(`remove test copy ${dir}`, () => fs.rmSync(dir, { recursive: true, force: true }));
  for (const d of manifest.dirs) step(`move back ${d.from}`, () => fs.renameSync(d.to, d.from));
  for (const r of manifest.reg) {
    spawnSync('reg', ['delete', r.key, '/f'], { stdio: 'ignore' });
    if (r.file) step(`import ${r.key}`, () => execSync(`reg import "${r.file}"`, { stdio: 'ignore' }));
  }
  const ok =
    problems.length === 0 && manifest.dirs.every((d) => fs.existsSync(d.from)) && manifest.reg.every((r) => regExists(r.key) === !!r.file);
  if (ok) fs.rmSync(safe, { recursive: true, force: true });
  else console.log(`RESTORE PROBLEMS (real data is still safe in ${safe}):\n  ${problems.join('\n  ')}`);
  return ok;
}

// ------------------------------------------------------------ helpers
function install() {
  const r = spawnSync(setup, ['/S', '/NS', `/D=${installDir}`], { stdio: 'ignore', windowsVerbatimArguments: true });
  return r.status === 0 && fs.existsSync(exe);
}

async function waitGone(file, seconds = 60) {
  for (let i = 0; i < seconds * 2 && fs.existsSync(file); i++) await sleep(500);
  return !fs.existsSync(file);
}

function uia(action, opts = {}) {
  const args = ['-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', path.join(root, 'e2e', 'uia.ps1'), '-Action', action, '-Window', TITLE];
  if (opts.name) args.push('-Name', opts.name);
  if (opts.text) args.push('-Text', opts.text);
  args.push('-Timeout', String(opts.timeout ?? 60));
  const r = spawnSync('powershell', args, { encoding: 'utf8' });
  if (r.status !== 0) {
    const dump = spawnSync('powershell', [...args.slice(0, 5), '-Action', 'dump', '-Window', TITLE], { encoding: 'utf8' });
    console.log(`UI AUTOMATION DUMP:\n${dump.stdout}\n${dump.stderr}`);
    throw new Error(`UI automation failed: ${action} ${opts.name ?? ''} ${opts.text ?? ''}: ${r.stdout}`);
  }
  return r.stdout;
}

/** Closes uninstaller windows left open by an interrupted run. */
function closeUninstallers() {
  execSync(
    `powershell -NoProfile -Command "Get-Process | Where-Object { $_.MainWindowTitle -like '${TITLE}*' } | Stop-Process -Force"`,
    { stdio: 'ignore' },
  );
}

function startUninstallGui() {
  const p = spawn(uninstaller, [], { detached: true, stdio: 'ignore' });
  p.unref();
  return uia('wait', { text: 'Your CODE X MART data' });
}

async function launch(env = {}, port = 9361) {
  const proc = spawn(exe, [], {
    env: { ...process.env, ...env, WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${port} ${OFFLINE}` },
    stdio: 'ignore',
  });
  return connect(proc, port);
}

async function connect(proc, port) {
  let browser;
  let exitCode = null;
  proc.on('exit', (c) => (exitCode = c));
  for (let i = 0; i < 120 && !browser && exitCode === null; i++) {
    try {
      browser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`);
    } catch {
      await sleep(500);
    }
  }
  if (!browser) {
    const logDir = path.join(LOCAL, 'com.codexmart.pos', 'logs');
    const log = fs.existsSync(logDir)
      ? fs.readdirSync(logDir).map((f) => fs.readFileSync(path.join(logDir, f), 'utf8')).join('').split('\n').slice(-30).join(' | ')
      : 'no log folder';
    const webviews = execSync(
      'powershell -NoProfile -Command "Get-CimInstance Win32_Process -Filter \\"Name=\'msedgewebview2.exe\'\\" | ForEach-Object { $_.ProcessId.ToString() + \' \' + $_.CommandLine.Substring(0, [Math]::Min(300, $_.CommandLine.Length)) }"',
      { encoding: 'utf8' },
    )
      .split(/\r?\n/)
      .filter((l) => l.includes('codexmart'))
      .join(' || ');
    console.log(`DIAGNOSTICS webviews: ${webviews || 'none'}`);
    throw new Error(`no CDP connection on port ${port} (process ${exitCode === null ? 'still running' : 'exited with ' + exitCode}); log: ${log}`);
  }
  const ctx = browser.contexts()[0];
  let page = ctx.pages()[0];
  for (let i = 0; !page && i < 40; i++) {
    await sleep(250);
    page = ctx.pages()[0];
  }
  page.setDefaultTimeout(20000);
  return { proc, browser, page };
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

const invoke = (page, cmd, args = {}) =>
  page.evaluate(
    ([c, a]) =>
      window.__TAURI_INTERNALS__.invoke(c, a).then(
        (r) => ({ ok: true, r }),
        (e) => ({ ok: false, e }),
      ),
    [cmd, args],
  );

async function setupShop(page, user, pass, store, { shell = true } = {}) {
  await page.getByText('Welcome to CODE X MART').waitFor({ timeout: 30000 });
  await page.getByLabel('Store name').fill(store);
  await page.getByRole('button', { name: 'Continue' }).click();
  await page.getByLabel('Full name').fill('Owner');
  await page.getByLabel('Username').fill(user);
  await page.locator('input[type=password]').nth(0).fill(pass);
  await page.locator('input[type=password]').nth(1).fill(pass);
  await page.getByRole('button', { name: 'Create account' }).click();
  await page.getByLabel('I have written down the recovery key').check();
  await page.getByRole('button', { name: 'Start using CODE X MART' }).click();
  if (shell) await page.getByRole('link', { name: 'Settings' }).waitFor();
}

async function addProductAndSale(page, name) {
  await page.getByRole('link', { name: 'Products' }).click();
  await page.getByRole('button', { name: 'New product' }).first().click();
  await page.getByLabel('Product name').fill(name);
  await page.getByLabel('Selling price').fill('300');
  await page.getByLabel('Cost price').fill('200');
  await page.getByLabel('Quantity on hand').fill('5');
  await page.getByRole('button', { name: 'Add product' }).click();
  await page.getByRole('status').filter({ hasText: 'Product added' }).first().waitFor();
  await page.getByRole('link', { name: 'Billing' }).click();
  const search = page.getByPlaceholder(/Scan a barcode/);
  await search.fill(name.toLowerCase());
  await page.getByRole('button', { name: new RegExp(name) }).first().waitFor();
  await search.press('Enter');
  await page.getByRole('button', { name: /^Pay Rs/ }).waitFor();
  await page.keyboard.press('F9');
  await page.getByLabel('Cash received').fill('300');
  await page.keyboard.press('Enter');
  const done = await page.getByText('Sale completed').waitFor().then(() => true, () => false);
  await page.keyboard.press('Enter');
  return done;
}

async function login(page, user, pass) {
  await page.getByRole('heading', { name: 'Sign in' }).waitFor({ timeout: 30000 });
  await page.getByLabel('Username').fill(user);
  await page.getByLabel('Password').fill(pass);
  await page.getByRole('button', { name: 'Sign in', exact: true }).click();
  await page.getByRole('link', { name: 'Settings' }).waitFor();
}

async function activate(page, key) {
  await page.getByRole('link', { name: 'Settings' }).click();
  await page.getByRole('button', { name: 'License', exact: true }).click();
  await page.getByLabel('Activation key').fill(key);
  await sleep(400);
  await page.getByRole('button', { name: 'Activate License' }).click();
}

const anchorTrialStart = () => {
  if (!fs.existsSync(ANCHOR)) return null;
  return Buffer.from(fs.readFileSync(ANCHOR, 'utf8').trim(), 'hex').readBigInt64BE(4 + 1 + 16 + 10 + 48);
};
const licenseRecordsPresent = () => regExists('HKCU\\Software\\CODE X MART Licensing') && LICENSE_DIRS.slice(0, 3).every((d) => fs.existsSync(d));

// ------------------------------------------------------------ run
fs.rmSync(work, { recursive: true, force: true });
fs.mkdirSync(shots, { recursive: true });
let manifest = null;
let app;
let tempGen;
try {
  closeUninstallers();
  manifest = safekeep();
  console.log(`Real data moved to ${safe} (${manifest.dirs.length} folders, ${manifest.reg.filter((r) => r.file).length} registry keys)`);

  // ---------------------------------------------- A. production behaviour
  check('Install (silent, per-user, offline)', install());
  const probeDir = path.join(work, 'must-not-be-created');
  app = await launch({
    CODEXMART_LICENSE_DIR: probeDir,
    CODEXMART_TEST_CLOCK_OFFSET: String(40 * 86400),
    CODEXMART_TEST_DEVICE: 'not-honoured',
  });
  await setupShop(app.page, 'owner', 'owner123', 'Installer Test Mart');
  let st = (await invoke(app.page, 'license_status')).r;
  check('Production build ignores the test hooks (clock, license folder)', st.trialDay === 1 && !fs.existsSync(probeDir), `day ${st.trialDay}`);
  check('Trial started; license record written to registry, AppData, LocalAppData, ProgramData', st.state === 'trial' && licenseRecordsPresent());
  check('Shop data in %APPDATA%\\CODE X MART', fs.existsSync(path.join(DATA, 'data', 'codexmart.db')));
  check('Product and sale recorded', await addProductAndSale(app.page, 'Installer Cream'));
  const backup = await invoke(app.page, 'backup_create', {});
  check('Local backup created', backup.ok && fs.readdirSync(path.join(DATA, 'backups')).length > 0);
  const T0 = anchorTrialStart();

  // A key signed by any key other than the owner's is refused.
  tempGen = await connect(
    spawn(genExe, [], { env: { ...process.env, CXM_LICENSEGEN_DIR: path.join(work, 'other-generator'), WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=9362 ${OFFLINE}` }, stdio: 'ignore' }),
    9362,
  );
  await tempGen.page.getByText('Create a new signing key').waitFor({ timeout: 30000 });
  await invoke(tempGen.page, 'gen_create_key', { passphrase: 'someone else key', confirm: 'someone else key' });
  await invoke(tempGen.page, 'gen_backup_key', { path: path.join(work, 'other.cxkey') });
  const request = (await invoke(app.page, 'license_request_code')).r.requestCode;
  const foreign = await invoke(tempGen.page, 'gen_issue', {
    input: { requestCode: request, customerName: 'Pirate', edition: 'lifetime', issuedOn: new Date().toLocaleDateString('en-CA'), kind: 'new' },
  });
  await close(tempGen);
  tempGen = null;
  await activate(app.page, foreign.r.record.activationKey);
  await app.page.getByRole('alert').getByText('not signed by the CODE X MART license key').waitFor();
  check('Key signed by another (non-owner) key is refused', (await invoke(app.page, 'license_status')).r.state === 'trial');
  await close(app);
  app = null;

  // ---------------------------------------------- B. cancelling never deletes
  startUninstallGui();
  uia('shot', { name: path.join(shots, 'uninstall-01-choice.png') });
  uia('click', { name: 'Cancel' });
  uia('gone');
  check('Uninstall cancelled on the data page: nothing removed', fs.existsSync(exe) && fs.existsSync(path.join(DATA, 'data', 'codexmart.db')));

  startUninstallGui();
  uia('select', { name: 'Delete all my data and credentials' });
  uia('click', { name: 'Next >' });
  const q1 = uia('answer', { text: 'Are you sure', name: 'No' });
  check('First confirmation asks the exact question', q1.includes('Are you sure you want to permanently delete all CODE X MART data and credentials?'));
  uia('wait', { text: 'Your CODE X MART data' });
  uia('click', { name: 'Cancel' });
  uia('gone');
  check('"No" at the first confirmation: nothing removed', fs.existsSync(exe) && fs.existsSync(path.join(DATA, 'data', 'codexmart.db')));

  startUninstallGui();
  uia('select', { name: 'Delete all my data and credentials' });
  uia('click', { name: 'Next >' });
  uia('answer', { text: 'Are you sure', name: 'Yes' });
  uia('answer', { text: 'Final confirmation', name: 'No' });
  uia('wait', { text: 'Your CODE X MART data' });
  uia('click', { name: 'Cancel' });
  uia('gone');
  check('"No" at the second confirmation: nothing removed', fs.existsSync(exe) && fs.existsSync(path.join(DATA, 'data', 'codexmart.db')));

  // ---------------------------------------------- C. keep my data
  startUninstallGui();
  uia('click', { name: 'Next >' });
  uia('click', { name: 'Uninstall' });
  uia('click', { name: 'Close', timeout: 120 });
  uia('gone');
  await waitGone(exe);
  check('Keep: program removed', !fs.existsSync(exe) && !regExists(REG_KEYS[0]));
  check(
    'Keep: database, backups and license record kept',
    fs.existsSync(path.join(DATA, 'data', 'codexmart.db')) && fs.readdirSync(path.join(DATA, 'backups')).length > 0 && licenseRecordsPresent(),
  );
  check('Reinstall after keep', install());
  app = await launch();
  await login(app.page, 'owner', 'owner123');
  check('Keep: same login works, no new setup', true);
  const products = (await invoke(app.page, 'products_list', { filter: {} })).r;
  const sales = (await invoke(app.page, 'sales_list', { filter: {} })).r;
  const settings = (await invoke(app.page, 'settings_get')).r;
  const backups = (await invoke(app.page, 'backup_status')).r;
  check(
    'Keep: products, sales, settings and backups are back',
    products.items.some((p) => p.name === 'Installer Cream') && sales.total === 1 && settings.storeName === 'Installer Test Mart' && backups.files.length > 0,
  );
  st = (await invoke(app.page, 'license_status')).r;
  check('Keep: trial unchanged', st.state === 'trial' && anchorTrialStart() === T0);

  // ---------------------------------------------- D. delete everything (app running)
  startUninstallGui();
  uia('select', { name: 'Delete all my data and credentials' });
  uia('click', { name: 'Next >' });
  uia('answer', { text: 'Are you sure', name: 'Yes' });
  uia('answer', { text: 'Final confirmation', name: 'Yes' });
  uia('click', { name: 'Uninstall' });
  uia('answer', { text: 'is running', name: 'OK' });
  uia('click', { name: 'Close', timeout: 120 });
  uia('gone');
  try {
    await app.browser.close();
  } catch {}
  app = null;
  await waitGone(exe);
  check('Delete with the app open: app closed first', !running());
  check('Delete: shop database, accounts and local backups removed', !fs.existsSync(DATA));
  check('Delete: logs and WebView2 profile removed', !fs.existsSync(path.join(LOCAL, 'com.codexmart.pos')));
  check('Delete: license record kept, original trial start', licenseRecordsPresent() && anchorTrialStart() === T0);
  check('Reinstall after delete', install());
  app = await launch();
  await app.page.getByText('Welcome to CODE X MART').waitFor({ timeout: 30000 });
  const oldLogin = await invoke(app.page, 'auth_login', { username: 'owner', password: 'owner123' });
  check('After delete: first-run setup shown, old login refused', !oldLogin.ok);
  await setupShop(app.page, 'newowner', 'newpass123', 'Fresh Mart');
  const fresh = await Promise.all([
    invoke(app.page, 'products_list', { filter: {} }),
    invoke(app.page, 'sales_list', { filter: {} }),
    invoke(app.page, 'backup_status'),
  ]);
  check('After delete: empty database, no old backups', fresh[0].r.total === 0 && fresh[1].r.total === 0 && fresh[2].r.files.length === 0);
  st = (await invoke(app.page, 'license_status')).r;
  check('After delete: new account does not restart the trial', anchorTrialStart() === T0 && st.trialStartedOn !== null);

  // ---------------------------------------------- E. production signing key
  const req = (await invoke(app.page, 'license_request_code')).r.requestCode;
  const keyFile = path.join(work, 'production-key.txt');
  execSync('cargo test -p codexmart-license-generator -- --ignored production_smoke --nocapture', {
    cwd: root,
    env: { ...process.env, CXM_REQUEST_CODE: req, CXM_KEY_OUT: keyFile },
    stdio: 'ignore',
  });
  await activate(app.page, fs.readFileSync(keyFile, 'utf8').trim());
  await app.page.getByRole('status').filter({ hasText: 'License activated' }).first().waitFor();
  st = (await invoke(app.page, 'license_status')).r;
  check('Production key: lifetime license activates offline', st.state === 'licensed' && st.license.expiresOn === null, st.license?.serial);
  await close(app);
  app = await launch();
  await login(app.page, 'newowner', 'newpass123');
  check('Production key: license valid after restart', (await invoke(app.page, 'license_status')).r.state === 'licensed');
  await close(app);
  app = null;

  // Silent keep, then silent delete.
  spawnSync(uninstaller, ['/S'], { stdio: 'ignore' });
  await waitGone(exe);
  check('Silent uninstall keeps data by default', fs.existsSync(path.join(DATA, 'data', 'codexmart.db')));
  install();
  app = await launch();
  await login(app.page, 'newowner', 'newpass123');
  check('Reinstall: still licensed', (await invoke(app.page, 'license_status')).r.state === 'licensed');
  await close(app);
  app = null;
  spawnSync(uninstaller, ['/S', '/DELETEDATA'], { stdio: 'ignore' });
  await waitGone(exe);
  await sleep(1500);
  check('Silent uninstall with /DELETEDATA removes the data', !fs.existsSync(DATA));
  install();
  app = await launch();
  await setupShop(app.page, 'third', 'thirdpass1', 'Third Mart');
  check('License is a device record: survives data deletion', (await invoke(app.page, 'license_status')).r.state === 'licensed');
  await close(app);
  app = null;

  // ---------------------------------------------- F. locked database
  const db = path.join(DATA, 'data', 'codexmart.db');
  const holder = spawn(
    'powershell',
    ['-NoProfile', '-Command', `$f=[System.IO.File]::Open('${db}','Open','ReadWrite','None'); Start-Sleep -Seconds 25; $f.Close()`],
    { stdio: 'ignore' },
  );
  await sleep(3000);
  const locked = spawnSync(uninstaller, ['/S', '/DELETEDATA', `_?=${installDir}`], { stdio: 'ignore', windowsVerbatimArguments: true });
  check(
    'Locked database: deletion reported as incomplete (exit code 2), marker left',
    locked.status === 2 && fs.existsSync(path.join(DATA, 'DELETE-PENDING.txt')) && fs.existsSync(db),
    `exit ${locked.status}`,
  );
  await new Promise((r) => holder.on('exit', r));
  install();
  app = await launch();
  await app.page.getByText('Welcome to CODE X MART').waitFor({ timeout: 30000 });
  check('Next start finishes the deletion: setup shown, old data gone', !fs.existsSync(path.join(DATA, 'DELETE-PENDING.txt')));
  await setupShop(app.page, 'fourth', 'fourthpass', 'Fourth Mart');
  await invoke(app.page, 'backup_create', {});
  await close(app);
  app = null;

  // ---------------------------------------------- G. damaged database, read-only backup
  fs.writeFileSync(db, 'this is not a database');
  const bk = fs.readdirSync(path.join(DATA, 'backups')).find((f) => f.endsWith('.cxmbak'));
  execSync(`attrib +R "${path.join(DATA, 'backups', bk)}"`);
  spawnSync(uninstaller, ['/S', '/DELETEDATA'], { stdio: 'ignore' });
  await waitGone(exe);
  await sleep(1500);
  check('Damaged database and read-only backup are deleted too', !fs.existsSync(DATA));

  // ---------------------------------------------- H. expired trial stays expired after delete
  // The e2e build simulates day 31 while the real uninstaller deletes the data.
  const licDir = path.join(work, 'e2e-license');
  const e2e = (days, port) =>
    connect(
      spawn(e2eExe, [], {
        env: {
          ...process.env,
          CODEXMART_DATA_DIR: DATA,
          CODEXMART_LICENSE_DIR: licDir,
          CODEXMART_TEST_CLOCK_OFFSET: String(days * 86400),
          WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${port} ${OFFLINE}`,
        },
        stdio: 'ignore',
      }),
      port,
    );
  app = await e2e(0, 9363);
  await setupShop(app.page, 'owner', 'owner123', 'Expiring Mart');
  await close(app);
  app = null;
  install();
  spawnSync(uninstaller, ['/S', '/DELETEDATA'], { stdio: 'ignore' });
  await waitGone(exe);
  await sleep(1500);
  app = await e2e(30, 9364);
  await setupShop(app.page, 'brandnew', 'brandnew12', 'Brand New Mart', { shell: false });
  await app.page.getByText('Your free trial has expired.').first().waitFor();
  await app.page.screenshot({ path: path.join(shots, 'after-delete-day31.png') });
  check('Day 31 after "delete all data" and a new owner account: activation still required', true);
  await close(app);
  app = null;
} catch (e) {
  check('Unexpected error', false, e.message.split('\n')[0]);
} finally {
  await close(app);
  await close(tempGen);
  closeUninstallers();
  if (fs.existsSync(uninstaller)) {
    spawnSync(uninstaller, ['/S'], { stdio: 'ignore' });
    await waitGone(exe);
  }
  if (manifest) {
    const ok = restore(manifest);
    check('Real data, license records and registry keys restored', ok);
  }
  const passed = results.filter(Boolean).length;
  console.log(`\n${passed}/${results.length} checks passed`);
  process.exitCode = passed === results.length ? 0 : 1;
}
