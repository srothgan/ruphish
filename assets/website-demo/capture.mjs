import { randomBytes } from 'node:crypto';
import { chromium } from 'playwright';

export async function capture(base, password, destination) {
  const browser = await chromium.launch({ headless: true });
  try {
    const context = await browser.newContext({
      viewport: { width: 1512, height: 982 },
      deviceScaleFactor: 2,
      locale: 'en-US',
      timezoneId: 'Europe/Berlin',
      colorScheme: 'light',
      reducedMotion: 'reduce',
      // This is loopback-only: avoid compressing the large development WASM on every capture.
      extraHTTPHeaders: { 'Accept-Encoding': 'identity' },
    });
    async function request(path, form) {
      const response = form
        ? await context.request.post(base + path, {
            headers: { Accept: 'application/json', Origin: base },
            form,
          })
        : await context.request.get(base + path);
      if (!response.ok()) {
        throw new Error(`${path}: HTTP ${response.status()}`);
      }
      return response.json();
    }
    const initial = await request('/auth/session');
    await request('/auth/login', { username: 'demo', password, csrf_token: initial.csrf_token });
    const signedIn = await request('/auth/session');
    if (signedIn.user?.password_change_required) {
      const replacement = randomBytes(24).toString('hex');
      await request('/auth/password/reset', {
        password: replacement,
        confirm_password: replacement,
        csrf_token: signedIn.csrf_token,
      });
    }

    const page = await context.newPage();
    const pendingRequests = new Set();
    page.on('request', (request) => pendingRequests.add(request));
    page.on('requestfinished', (request) => pendingRequests.delete(request));
    page.on('pageerror', (error) => console.error('Dashboard browser error:', error.message));
    page.on('requestfailed', (request) => {
      pendingRequests.delete(request);
      console.error(
        'Dashboard request failed:',
        new URL(request.url()).pathname,
        request.failure()?.errorText,
      );
    });
    page.on('response', (response) => {
      if (response.status() >= 400) {
        console.error('Dashboard HTTP error:', response.status(), new URL(response.url()).pathname);
      }
    });
    page.setDefaultTimeout(60_000);
    await page.goto(base + '/', { waitUntil: 'domcontentloaded', timeout: 60_000 });
    try {
      await page.getByRole('heading', { name: 'Dashboard', exact: true }).waitFor();
      await page.locator('.query-region[data-ready="true"][aria-busy="false"]').first().waitFor();
    } catch (error) {
      console.error(
        'Dashboard did not load:',
        new URL(page.url()).pathname,
        (await page.locator('body').innerText()).slice(0, 2000),
      );
      console.error(
        'Pending dashboard requests:',
        [...pendingRequests].map((request) => new URL(request.url()).pathname),
      );
      throw error;
    }
    await page.evaluate(() => document.fonts.ready);
    await page.screenshot({ path: destination, fullPage: false, animations: 'disabled' });
    const current = await request('/auth/session');
    await request('/auth/logout', { csrf_token: current.csrf_token });
    console.log('Dashboard loaded and captured.');
  } finally {
    await browser.close();
  }
}
