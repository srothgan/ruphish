// Establish a disposable administrator through the real browser-authentication API.
import assert from 'node:assert/strict';
import { spawn, spawnSync } from 'node:child_process';

export function establishContainerAccount(container, initialPassword) {
  function request(path, fields) {
    const args = [
      'exec',
      container,
      'curl',
      '--noproxy',
      '*',
      '--fail',
      '--silent',
      '--show-error',
      '--cookie',
      '/tmp/account-cookies',
      '--cookie-jar',
      '/tmp/account-cookies',
      '-H',
      'Accept: application/json',
      '-H',
      'Origin: http://127.0.0.1:3333',
    ];
    for (const [key, value] of Object.entries(fields ?? {})) {
      args.push('--data-urlencode', `${key}=${value}`);
    }
    args.push(`http://127.0.0.1:3333${path}`);
    const response = spawnSync('docker', args, { encoding: 'utf8' });
    assert.ifError(response.error);
    assert.equal(response.status, 0, response.stderr);
    return JSON.parse(response.stdout);
  }
  request('/auth/login', {
    username: 'admin',
    password: initialPassword,
    csrf_token: request('/auth/session').csrf_token,
  });
  const session = request('/auth/session');
  assert.equal(session.user.password_change_required, true);
  request('/auth/password/reset', {
    password: 'changed-container-password',
    confirm_password: 'changed-container-password',
    csrf_token: session.csrf_token,
  });
}

// Follow the application's readiness event; container exit is an immediate failure.
export async function waitContainerReady(container) {
  const logs = spawn('docker', ['logs', '--follow', container], {
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  const closed = new Promise((resolve) => logs.once('close', resolve));
  try {
    await new Promise((resolve, reject) => {
      const buffers = new Map();
      for (const stream of [logs.stdout, logs.stderr]) {
        stream.setEncoding('utf8');
        stream.on('data', (chunk) => {
          const text = (buffers.get(stream) ?? '') + chunk;
          if (/RUPHISH_READY \{[^\n]*\}/.test(text)) {
            resolve();
          }
          buffers.set(stream, text.slice(-4096));
        });
      }
      logs.once('error', reject);
      logs.once('close', () => reject(new Error(`Container ${container} exited before readiness`)));
    });
  } finally {
    logs.kill();
    await closed;
  }
}
