// Runs the end-to-end tests against the real desktop app.
//
// Builds a test copy of CODE X MART (production UI embedded, debug Rust,
// `e2e` test hooks) and of the License Generator, then runs the suites on
// throwaway folders. Your real shop data, license state and signing key are
// never touched: CODEXMART_DATA_DIR, CODEXMART_LICENSE_DIR and
// CXM_LICENSEGEN_DIR point to temporary folders. The e2e hooks are a build
// feature that the installer build never enables.
//
//   npm run test:e2e              build, then test
//   npm run test:e2e -- --no-build   test the existing build
//
// Close any running CODE X MART first (only one copy can run at a time).
import { execSync, spawnSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const work = fs.mkdtempSync(path.join(os.tmpdir(), 'codexmart-e2e-'));
const data = path.join(work, 'data');
const shots = path.join(work, 'screenshots');
// Suites 1 and 2 keep their trial state here, away from this computer's own.
process.env.CODEXMART_LICENSE_DIR = path.join(work, 'license');

if (!process.argv.includes('--no-build')) {
  execSync('npm run build', { cwd: root, stdio: 'inherit' });
  execSync('cargo build -p codexmart --features tauri/custom-protocol,e2e', { cwd: root, stdio: 'inherit' });
  execSync('npm run licensegen:ui', { cwd: root, stdio: 'inherit' });
  execSync('cargo build -p codexmart-license-generator --features tauri/custom-protocol', { cwd: root, stdio: 'inherit' });
}
const exe = path.join(root, 'target', 'debug', 'codexmart.exe');
const generator = path.join(root, 'target', 'debug', 'codexmart-license-generator.exe');

let failedSuites = 0;
const run = (suite, args) => {
  console.log(`\n=== ${suite}`);
  const r = spawnSync(process.execPath, [path.join(root, 'e2e', suite), ...args], { stdio: 'inherit' });
  if (r.status !== 0) failedSuites++;
};
run('01-first-day.mjs', [exe, data, shots]);
run('02-restart-backup-restore.mjs', [exe, data, shots]);
run('03-licensing.mjs', [exe, generator, path.join(work, 'licensing'), shots]);
console.log(`\nScreenshots and results: ${shots}`);
process.exitCode = failedSuites > 0 ? 1 : 0;
