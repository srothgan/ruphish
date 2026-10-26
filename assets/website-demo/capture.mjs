import assert from 'node:assert/strict';
import { randomBytes } from 'node:crypto';
import { chromium } from 'playwright';

export async function capture(base, password, destination) {
  const browser = await chromium.launch({ headless: true });
  try {
    const context = await browser.newContext({
      viewport: { width: 1512, height: 982 }, deviceScaleFactor: 2,
      locale: 'en-US', timezoneId: 'Europe/Berlin', colorScheme: 'light', reducedMotion: 'reduce',
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
    page.setDefaultTimeout(60_000);
    await page.goto(base + '/', { waitUntil: 'domcontentloaded', timeout: 60_000 });
    await page.locator('.dashboard-metric-value').first().waitFor();
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
