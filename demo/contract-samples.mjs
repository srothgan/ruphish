// Exercise the real demo dispatcher for Rust contract validation; stdout is synthetic JSON only.
import { readFileSync } from 'node:fs';
import { createDemoApi } from '../crates/ruphish-ui/web/demo-api.mjs';
const memory = new Map();
const api = createDemoApi({
  now: 1793016000,
  storage: {
    getItem: (k) => memory.get(k),
    setItem: (k, v) => memory.set(k, v),
    removeItem: (k) => memory.delete(k),
  },
});
const responses = {};
async function read(path, method = 'GET', body) {
  const response = await api.execute(
    new Request(`https://demo.example.test${path}`, {
      method,
      ...(body === undefined
        ? {}
        : { body: JSON.stringify(body), headers: { 'content-type': 'application/json' } }),
    }),
  );
  if (!response.ok) {
    throw new Error(`${method} ${path}: ${response.status}`);
  }
  return response.json();
}
await read('/auth/login', 'POST', { username: 'demo', password: 'demodemo' });
for (const path of [
  '/auth/session',
  '/api/overview',
  '/api/overview/posture?window=30d',
  '/api/campaigns/directory',
  '/api/campaigns/3/delivery',
  '/api/campaigns/3/results',
  '/api/campaigns/3/recipients/30001/history',
  '/api/campaign-drafts/00000000-0000-4000-8000-000000000001',
  '/api/settings',
  '/api/users/',
  '/api/users/1',
  '/api/webhooks/',
  '/api/reporting-mailboxes/',
  '/api/domains/',
  '/api/starter-templates/',
]) {
  responses[path] = await read(path);
}
for (const entry of responses['/api/starter-templates/']) {
  const path = `/api/starter-templates/${entry.id}`;
  responses[path] = await read(path);
}
for (const family of ['groups', 'templates', 'pages', 'learning-pages', 'smtp']) {
  for (const suffix of ['summary', '1']) {
    responses[`/api/${family}/${suffix}`] = await read(`/api/${family}/${suffix}`);
  }
}
responses.review = await read(
  '/api/campaign-drafts/prepare',
  'POST',
  responses['/api/campaign-drafts/00000000-0000-4000-8000-000000000001'].configuration,
);
responses.closure = await read('/api/campaigns/complete-all', 'POST', {
  token: 'demo-confirmation',
  confirmation: 'CLOSE ALL',
});
const csvCases = JSON.parse(
  readFileSync(new URL('./recipient-csv-cases.json', import.meta.url), 'utf8'),
);
responses.csv_reviews = [];
for (const scenario of csvCases.filter((c) => c.status === 200)) {
  const body = {
    ...scenario.request,
    ...(scenario.template
      ? {
          content: readFileSync(
            new URL('../crates/ruphish-ui/assets/recipient-template.csv', import.meta.url),
            'utf8',
          ),
        }
      : {}),
  };
  responses.csv_reviews.push(await read('/api/import/group/review', 'POST', body));
}
process.stdout.write(JSON.stringify(responses));
