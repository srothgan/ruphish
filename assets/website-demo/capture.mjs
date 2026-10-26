import assert from 'node:assert/strict';
import { randomBytes } from 'node:crypto';
import { chromium } from 'playwright';

export async function capture(base, password, destination) {
  const browser = await chromium.launch({ headless: true });
  try {
    const context = await browser.newContext({
      viewport: { width: 1512, height: 982 }, deviceScaleFactor: 2,
      locale: 'en-US', timezoneId: 'Europe/Berlin', colorScheme: 'light', reducedMotion: 'reduce',
      // This is loopback-only: avoid compressing the large development WASM on every capture.
      extraHTTPHeaders: { 'Accept-Encoding': 'identity' },
    });
    async function request(path, form) {
      const response = form
        ? await context.request.post(base + path, { headers: { Accept: 'application/json', Origin: base }, form })
        : await context.request.get(base + path);
      if (!response.ok()) throw new Error(`${path}: HTTP ${response.status()}`);
      return response.json();
    }
    const initial = await request('/auth/session');
    await request('/login', { username: 'demo', password, csrf_token: initial.csrf_token });
    const signedIn = await request('/auth/session');
    if (signedIn.user?.password_change_required) {
      const replacement = randomBytes(24).toString('hex');
      await request('/reset_password', { password: replacement, confirm_password: replacement, csrf_token: signedIn.csrf_token });
    }

    const overview = await request('/api/overview');
    const metrics = await request('/api/overview/posture?window=30d');
    assert.equal(overview.campaigns.total, 4);
    assert.equal(overview.inventory.groups, 6);
    assert.equal(overview.inventory.reporting_mailboxes, 2);
    assert.equal(overview.attention.delivery_problems, 12);
    assert.equal(metrics.delivered, 937);

    const page = await context.newPage();
    const pendingRequests = new Set();
    page.on('request', (request) => pendingRequests.add(request));
    page.on('requestfinished', (request) => pendingRequests.delete(request));
    page.on('pageerror', (error) => console.error('Dashboard browser error:', error.message));
    page.on('requestfailed', (request) => {
      pendingRequests.delete(request);
      console.error('Dashboard request failed:', new URL(request.url()).pathname, request.failure()?.errorText);
    });
    page.on('response', (response) => {
      if (response.status() >= 400) console.error('Dashboard HTTP error:', response.status(), new URL(response.url()).pathname);
    });
    page.setDefaultTimeout(60_000);
    await page.goto(base + '/', { waitUntil: 'domcontentloaded', timeout: 60_000 });
    try {
      await page.locator('.dashboard-metric-value').first().waitFor();
    } catch (error) {
      console.error('Dashboard did not load:', new URL(page.url()).pathname, (await page.locator('body').innerText()).slice(0, 2000));
      console.error('Pending dashboard requests:', [...pendingRequests].map((request) => new URL(request.url()).pathname));
      throw error;
    }
    await page.locator('.dashboard-attention').waitFor();
    const displayedCounts = await page.locator('.dashboard-metric-value').evaluateAll((nodes) => nodes.map((node) =>
      [...node.childNodes].filter((child) => child.nodeType === Node.TEXT_NODE).map((child) => child.textContent).join('').trim()));
    assert.ok(displayedCounts.includes(String(metrics.delivered)), 'The dashboard must display the loaded sent total.');
    await page.evaluate(() => document.fonts.ready);
    await page.screenshot({ path: destination, fullPage: false, animations: 'disabled' });
    const current = await request('/auth/session');
    await request('/logout', { csrf_token: current.csrf_token });
    console.log('Dashboard loaded: four campaigns, six groups, two reporting mailboxes, twelve delivery failures, 937 emails sent.');
  } finally {
    await browser.close();
  }
}
