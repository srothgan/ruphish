import { spawn } from 'node:child_process';
import { randomBytes } from 'node:crypto';
import { copyFile, mkdtemp, rename, rm, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { DatabaseSync } from 'node:sqlite';
import { setTimeout as delay } from 'node:timers/promises';
import { capture } from './capture.mjs';
import { seed } from './seed.mjs';

const base = 'http://127.0.0.1:3333';
const state = await mkdtemp(join(tmpdir(), 'ruphish-dashboard-'));
const password = randomBytes(24).toString('hex');
const config = join(state, 'config.json');
const database = join(state, 'ruphish.db');
const screenshot = join(state, 'dashboard.png');
const pending = `/output/.dashboard-${randomBytes(8).toString('hex')}.png`;
let child;
let exited;

async function start() {
  child = spawn('/app/ruphish', ['--config', config, '--mode', 'admin', '--disable-mailer'], {
    cwd: '/app',
    stdio: ['ignore', 'inherit', 'inherit'],
    env: { ...process.env, GOPHISH_INITIAL_ADMIN_PASSWORD: password },
  });
  let spawnError;
  exited = new Promise((resolve) => {
    child.once('error', (error) => {
      spawnError = error;
    });
    child.once('close', (code, signal) => resolve({ code, signal }));
  });
  const deadline = Date.now() + 60_000;
  let readiness = 'no response';
  while (Date.now() < deadline) {
    if (spawnError) {
      throw spawnError;
    }
    if (child.exitCode !== null || child.signalCode !== null) {
      throw new Error('Application exited before readiness.');
    }
    try {
      const response = await fetch(base + '/auth/session', { signal: AbortSignal.timeout(2000) });
      if (response.ok) {
        return;
      }
      readiness = `HTTP ${response.status}`;
    } catch (error) {
      readiness = error.cause?.code ?? error.message;
    }
    await delay(250);
  }
  throw new Error(`Application startup timed out: ${readiness}`);
}

async function stop() {
  if (!child) {
    return;
  }
  const current = child;
  child = undefined;
  current.kill('SIGTERM');
  const timeout = setTimeout(() => current.kill('SIGKILL'), 15_000);
  try {
    const result = await exited;
    if (result.code !== 0) {
      throw new Error(`Application shutdown failed: ${JSON.stringify(result)}`);
    }
  } finally {
    clearTimeout(timeout);
  }
}

try {
  await writeFile(
    config,
    JSON.stringify({
      admin_server: { listen_url: '127.0.0.1:3333', use_tls: false },
      db_name: 'sqlite3',
      db_path: database,
      logging: { level: 'warn' },
      contact_address: 'awareness@example.test',
    }),
    { mode: 0o600 },
  );
  console.log('Initializing a fresh disposable SQLite database.');
  await start();
  await stop();
  const db = new DatabaseSync(database);
  try {
    seed(db);
  } finally {
    db.close();
  }
  console.log('Seeded the complete dashboard dataset once.');
  await start();
  await capture(base, password, screenshot);
  await stop();
  // The final rename is on the output filesystem, so a failed capture/copy keeps the old image.
  await copyFile(screenshot, pending);
  await rename(pending, '/output/dashboard.png');
  console.log(
    'Replaced assets/dashboard.png (3024 x 1964). Copy it to website/public/dashboard.png manually when ready.',
  );
} finally {
  try {
    await stop();
  } finally {
    await rm(pending, { force: true });
    await rm(state, { recursive: true, force: true });
  }
}
