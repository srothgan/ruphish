// Establish a disposable administrator through the real browser-authentication API.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';

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
      '--max-time',
      '10',
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
    const response = spawnSync('docker', args, { encoding: 'utf8', timeout: 20_000 });
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
