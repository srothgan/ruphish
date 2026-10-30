import test from 'node:test';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { fileURLToPath } from 'node:url';

const execute = promisify(execFile);
const fixture = fileURLToPath(new URL('./support/ui-interactions.mjs', import.meta.url));

// Runs the built application in JSDOM with the existing synthetic API, never a live server.
// Build the development distribution with `trunk build --locked` first.
for (const scenario of [
  'form-import',
  'form-webhook',
  'form-mailbox',
  'form-filters',
  'form-group',
  'form-email',
  'form-page',
  'form-learning',
  'form-smtp',
  'form-user',
  'form-domain',
  'form-password',
  'form-preferences',
  'form-settings',
  'form-campaign',
  'form-draft',
  'form-session',
  'results-chart',
  'results-refresh',
  'recipient-return',
  'recipient-error-return',
  'recipient-direct',
  'recipient-foreign-return',
  'completed-entry',
  'completed-direct',
  'active-entry',
  'queued-entry',
  'event-editor',
  'delivery-refresh',
  'delivery-missing',
  'event-editor-stale-403',
  'event-editor-stale-404',
  'bookmark',
  'dashboard',
  'directory',
  'missing',
  'forbidden',
  'retry',
  'invalid',
  'new',
]) {
  test(`UI interaction: ${scenario}`, async () => {
    await execute(process.execPath, [fixture, scenario], {
      maxBuffer: 1024 * 1024,
    });
  });
}
