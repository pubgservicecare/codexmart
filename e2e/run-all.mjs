// Runs the end-to-end tests against the real desktop app.
//
// Builds a test copy of CODE X MART (production UI embedded, debug Rust),
// then runs both suites on a throwaway data folder. Your real shop data is
// never touched: the app is started with CODEXMART_DATA_DIR pointing to a
// temporary folder.
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

if (!process.argv.includes('--no-build')) {
  execSync('npm run build', { cwd: root, stdio: 'inherit' });
  execSync('cargo build -p codexmart --features tauri/custom-protocol', { cwd: root, stdio: 'inherit' });
}
const exe = path.join(root, 'target', 'debug', process.platform === 'win32' ? 'codexmart.exe' : 'codexmart');

let failedSuites = 0;
for (const suite of ['01-first-day.mjs', '02-restart-backup-restore.mjs']) {
  console.log(`\n=== ${suite}`);
  const r = spawnSync(process.execPath, [path.join(root, 'e2e', suite), exe, data, shots], { stdio: 'inherit' });
  if (r.status !== 0) failedSuites++;
}
console.log(`\nScreenshots and results: ${shots}`);
process.exitCode = failedSuites > 0 ? 1 : 0;
