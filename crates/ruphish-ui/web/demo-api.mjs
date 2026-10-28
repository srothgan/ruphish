import { starterCatalog } from '../../ruphish-http/src/starter_templates/catalog.mjs';
// Browser-only simulation of the existing wire contracts. No HTTP fallback or persistent resources.
import { dataset, DAY, iso, policy, features } from '../../../assets/website-demo/dataset.mjs';
import { exportResponse } from './demo-export.mjs';
import { namedCounts, eventKinds, recordedKind, eventLabel } from './demo-reporting.mjs';
import { reviewCsv } from './demo-csv.mjs';
import { recipientOrder, eventOrder, keyset } from './demo-sorting.mjs';

const kinds = ['Email Sent', 'Email Opened', 'Clicked Link', 'Submitted Data', 'Email Reported'];
const families = [
  'groups',
  'templates',
  'pages',
  'learning-pages',
  'smtp',
  'domains',
  'users',
  'webhooks',
  'reporting-mailboxes',
];
export const preferences = {
  language: null,
  format_locale: null,
  time_zone: null,
  show_utc_offset: true,
};
const copy = (value) => structuredClone(value);
const json = (value, status = 200) =>
  new Response(JSON.stringify(value), {
    status,
    headers: { 'content-type': 'application/json', etag: '"1"' },
  });
const success = (data = null) => json({ success: true, message: 'Demo action simulated', data });
const failure = (status, code = 'REQUEST_FAILED') =>
  json({ success: false, code, message: 'Demo request rejected', data: null }, status);
const counts = (events) => kinds.map((kind) => events.filter((e) => e.kind === kind).length);
const sum = (rows) => rows.reduce((a, b) => a.map((v, i) => v + b[i]), [0, 0, 0, 0, 0]);
const byTime = (a, b) => a.time.localeCompare(b.time) || a.id - b.id;

const eventFact = (e) => ({
  time: e.time,
  kind: recordedKind(e.kind),
  presence: {
    username_present: e.username_present ?? null,
    password_present: e.password_present ?? null,
  },
});
const eventRecord = (e) => ({
  id: e.id,
  source: e.source,
  revision: e.revision ?? 0,
  edited: e.edited ?? false,
  current: eventFact(e),
});

function posture(results, since = 0, until = Infinity) {
  const rows = results.filter((r) => r.state === 'accepted' && r.due >= since && r.due < until);
  const flags = sum(
    rows.map((r) =>
      counts(r.events.filter((e) => Date.parse(e.time) / 1000 < until)).map((n) => Number(n > 0)),
    ),
  );
  return {
    delivered: rows.length,
    opened: flags[1],
    clicked: flags[2],
    credentials_entered: flags[3],
    reported: flags[4],
  };
}

export function overview(data) {
  const headers = data.campaigns.map((c) => c.header);
  const scheduled = headers
    .filter((c) => c.status === 'Queued')
    .sort((a, b) => a.launch_date.localeCompare(b.launch_date));
  return {
    as_of: data.now,
    recent: [...data.campaigns].reverse().map((c) => ({
      id: c.header.id,
      name: c.header.name,
      status: c.header.status,
      launch: c.header.launch_date,
      recipients: c.results.length,
      outcomes: posture(c.results),
    })),
    inventory: {
      templates: data.templates.length,
      pages: data.pages.length,
      groups: data.groups.length,
      profiles: data.smtp.length,
      domains: data.domains.length,
      domains_verified: data.domains.filter((d) => d.eligible).length,
      reporting_mailboxes: data['reporting-mailboxes'].length,
    },
    attention: {
      unverified_enabled_domains: data.domains.filter((d) => d.enabled && !d.eligible).length,
      delivery_problems: data.campaigns
        .filter((c) => c.header.status !== 'Completed')
        .flatMap((c) => c.results)
        .filter((r) => r.state === 'failed').length,
      campaign_ids: data.campaigns
        .filter(
          (c) =>
            c.header.status !== 'Completed' &&
            c.results.some((r) => ['failed', 'blocked', 'unknown'].includes(r.state)),
        )
        .slice(0, 10)
        .map((c) => c.header.id),
      next_launch: scheduled.length
        ? { id: scheduled[0].id, name: scheduled[0].name, launch: scheduled[0].launch_date }
        : null,
    },
  };
}

export function analysis(campaign, query, now, paginate = true) {
  const selectedEvents = (query.get('event') || '').split(',').filter(Boolean);
  if (selectedEvents.some((kind) => !Object.values(eventKinds).includes(kind))) {
    throw new Error('Invalid event');
  }
  if (
    query.get('source') &&
    !['legacy', 'application', 'public', 'mailbox', 'operator'].includes(query.get('source'))
  ) {
    throw new Error('Invalid source');
  }
  const from = Number(query.get('from') ?? 0);
  const until = Number(query.get('until') ?? now);
  const selected = new Set((query.get('recipients') ?? '').split(',').filter(Boolean).map(Number));
  const search = (query.get('search') ?? '').toLocaleLowerCase();
  const results = campaign.results.filter(
    (r) =>
      (!query.has('recipient') || r.id === Number(query.get('recipient'))) &&
      (!selected.size || selected.has(r.id)) &&
      (!search || `${r.email} ${r.first_name} ${r.last_name}`.toLocaleLowerCase().includes(search)),
  );
  const eligibleEvents = results
    .flatMap((r) => r.events)
    .filter((e) => {
      const at = Date.parse(e.time) / 1000;
      return at >= from && at < until && (!query.get('source') || query.get('source') === e.source);
    })
    .sort(byTime);
  const matchingRecipients = new Set(
    eligibleEvents
      .filter((e) => selectedEvents.includes(eventKinds[e.kind]))
      .map((e) => e.recipient_id),
  );
  const events = selectedEvents.length
    ? eligibleEvents.filter((e) => matchingRecipients.has(e.recipient_id))
    : eligibleEvents;
  const eventMap = new Map();
  for (const e of events) {
    if (!eventMap.has(e.recipient_id)) {
      eventMap.set(e.recipient_id, []);
    }
    eventMap.get(e.recipient_id).push(e);
  }
  const unorderedRows = results
    .filter((r) => (!query.get('event') && !query.get('source')) || eventMap.has(r.id))
    .sort((a, b) => (a.email < b.email ? -1 : a.email > b.email ? 1 : a.id - b.id))
    .map((r) => ({
      id: r.id,
      email: r.email,
      name: `${r.first_name ?? ''} ${r.last_name ?? ''}`.trim(),
      first_name: r.first_name,
      last_name: r.last_name,
      position: r.position,
      send_date: r.send_date,
      modified_date: r.modified_date,
      delivery: r.state ?? null,
      counts: counts(eventMap.get(r.id) ?? []),
      historical_reported: false,
    }));
  const rows = recipientOrder(unorderedRows, query);
  const sortedEvents = eventOrder(events, query);
  const unique = sum(rows.map((r) => r.counts.map((n) => Number(n > 0))));
  const progression = sum(
    rows.map((r) => [
      Number(r.counts[0] > 0),
      Number(r.counts[1] + r.counts[2] + r.counts[3] > 0),
      Number(r.counts[2] + r.counts[3] > 0),
      Number(r.counts[3] > 0),
      Number(r.counts[4] > 0),
    ]),
  );
  const buckets = new Map();
  for (const e of events) {
    const time = Math.floor(Date.parse(e.time) / 1000 / 3600) * 3600;
    if (!buckets.has(time)) {
      buckets.set(time, [0, 0, 0, 0, 0]);
    }
    const index = kinds.indexOf(e.kind);
    if (index >= 0) {
      buckets.get(time)[index]++;
    }
  }
  const firstActions = new Map();
  for (const e of events) {
    const index = kinds.indexOf(e.kind);
    const at = Date.parse(e.time) / 1000;
    if (e.recipient_id == null || index < 0 || !Number.isFinite(at)) {
      continue;
    }
    const key = `${e.recipient_id}:${index}`;
    const previous = firstActions.get(key);
    if (!previous || at < previous.at) {
      firstActions.set(key, { index, at });
    }
  }
  const increments = new Map();
  for (const { index, at } of firstActions.values()) {
    const time = Math.floor(at / 3600) * 3600;
    if (!increments.has(time)) {
      increments.set(time, [0, 0, 0, 0, 0]);
    }
    increments.get(time)[index]++;
  }
  const participantBuckets = [];
  const cumulative = [0, 0, 0, 0, 0];
  if (increments.size) {
    const end = Math.max(...buckets.keys());
    for (let time = Math.min(...increments.keys()); time <= end; time += 3600) {
      const added = increments.get(time) ?? [0, 0, 0, 0, 0];
      added.forEach((count, index) => {
        cumulative[index] += count;
      });
      participantBuckets.push({ time, counts: namedCounts([...cumulative]) });
    }
  }
  const offset = Math.max(0, Number(query.get('offset') ?? 0));
  const limit = Math.max(1, Math.min(200, Number(query.get('limit') ?? 50)));
  return {
    first_observed: namedCounts(
      kinds.map((kind) => {
        const times = events.filter((e) => e.kind === kind).map((e) => Date.parse(e.time) / 1000);
        return times.length ? Math.min(...times) : null;
      }),
    ),
    last_observed: namedCounts(
      kinds.map((kind) => {
        const times = events.filter((e) => e.kind === kind).map((e) => Date.parse(e.time) / 1000);
        return times.length ? Math.max(...times) : null;
      }),
    ),
    id: campaign.header.id,
    name: campaign.header.name,
    status: campaign.header.status,
    as_of: now,
    audience: campaign.results.length,
    unique: namedCounts(unique),
    progression: namedCounts(progression),
    totals: namedCounts(counts(events)),
    recipient_total: rows.length,
    event_total: events.length,
    recipients: (paginate ? rows.slice(offset, offset + limit) : rows).map((r) => ({
      ...r,
      counts: namedCounts(r.counts),
    })),
    events: (paginate ? sortedEvents.slice(offset, offset + limit) : sortedEvents).map((e) => ({
      id: e.id,
      recipient_id: e.recipient_id,
      email: e.email,
      time: e.time,
      source: e.source,
      password_present: e.password_present ?? null,
      username_present: e.username_present ?? null,
      edited: e.edited ?? false,
      kind: recordedKind(e.kind),
    })),
    participant_buckets: participantBuckets,
    buckets: [...buckets].map(([time, values]) => ({ time, counts: namedCounts(values) })),
    bucket_seconds: 3600,
  };
}

function summaries(data, family) {
  const rows = data[family].map((r) => ({
    id: r.id,
    name: r.name,
    modified_date: r.modified_date,
    ...(family === 'groups'
      ? { num_targets: r.targets.length }
      : {
          active_campaigns: data.campaigns.filter(
            (c) =>
              c.header.status !== 'Completed' &&
              c.header[{ templates: 'template', pages: 'page', smtp: 'smtp' }[family]]?.id === r.id,
          ).length,
        }),
  }));
  return { total: rows.length, [family === 'groups' ? 'groups' : 'items']: rows };
}

function delivery(campaign, query, owner) {
  const after = Number(query.get('after') ?? 0);
  const requestedLimit = Number(query.get('limit') ?? 50);
  if (
    !Number.isSafeInteger(after) ||
    !Number.isSafeInteger(requestedLimit) ||
    (query.has('sort') && (requestedLimit < 1 || requestedLimit > 200))
  ) {
    throw new Error('Invalid page');
  }
  const limit = Math.max(1, Math.min(200, requestedLimit));
  const matches = campaign.results.filter((r) => r.id > after);
  let page = matches.slice(0, limit);
  let nextCursor = null;
  if (query.has('sort')) {
    if (after !== 0) {
      throw new Error('Mixed cursors');
    }
    const selected = keyset(
      campaign.results.map((row) => ({ ...row, attempt_count: row.attempts.length })),
      query,
      ['email', 'state', 'due', 'attempt_count'],
      `delivery:${owner}:${campaign.header.id}`,
      limit,
      'cursor',
    );
    page = selected.items;
    nextCursor = selected.cursor;
  } else if (query.has('order') || query.get('cursor')) {
    throw new Error('Missing sort');
  }
  const counts = {};
  for (const r of campaign.results) {
    counts[r.state] = (counts[r.state] ?? 0) + 1;
  }
  return {
    campaign: campaign.header,
    counts,
    items: page.map((r) => ({
      id: r.id,
      rid: r.rid,
      email: r.email,
      attempt_count: r.attempts.length,
      state: r.state,
      due: r.due,
      retries: 0,
      generation: 1,
      code: r.state === 'failed' ? 'smtp_permanent_rejection' : '',
      attempts: r.attempts,
    })),
    next_after: query.has('sort')
      ? nextCursor
        ? page.at(-1).id
        : null
      : matches.length > page.length
        ? page.at(-1).id
        : null,
    next_cursor: nextCursor,
    policy: campaign.header.policy,
    mailer_enabled: false,
    activity: campaign.results
      .flatMap((r) => r.events)
      .sort(byTime)
      .slice(-25)
      .reverse()
      .map((e) => ({
        id: e.id,
        time: e.time,
        email: e.email,
        message: e.kind,
        details: JSON.stringify(
          e.password_present === null ? {} : { password_present: e.password_present },
        ),
      })),
  };
}

function review(data, input) {
  const select = (family, ref) => {
    const r = data[family].find((r) => r.id === ref?.id);
    if (!r) {
      throw new Error('missing_selection');
    }
    return { id: r.id, name: r.name, revision: '1' };
  };
  const selection = {
    groups: (input.selection?.groups ?? []).map((r) => select('groups', r)),
    template: select('templates', input.selection?.template),
    page:
      input.selection?.page?.id === 0
        ? { id: 0, name: '', revision: '' }
        : select('pages', input.selection?.page),
    smtp: select('smtp', input.selection?.smtp),
  };
  const targets = selection.groups.flatMap((g) => data.groups.find((r) => r.id === g.id).targets);
  const total = new Set(targets.map((r) => r.email)).size;
  if (!total) {
    throw new Error('empty_audience');
  }
  const template = data.templates.find((r) => r.id === selection.template.id);
  const smtp = data.smtp.find((r) => r.id === selection.smtp.id);
  return {
    warnings: [],
    total,
    duplicate_count: targets.length - total,
    launch_date: data.now,
    send_by_date: null,
    selection,
    policy: { ...policy, ...(input.policy ?? {}) },
    mailer_enabled: false,
    conflict_count: 0,
    conflicts: [],
    preview: {
      subject: template.subject,
      html: '<h1>Demo campaign preview</h1><p>This is a sample preview. No messages will be sent.</p>',
      text: template.text,
      visible_sender: smtp.from_address,
      transport_sender: smtp.from_address,
    },
  };
}

export function createDemoApi({
  now = Math.floor(Date.now() / 1000),
  storage,
  data = dataset(now),
} = {}) {
  const key = 'ruphish-demo-session-v1';
  let nextEventId =
    Math.max(
      0,
      ...data.campaigns.flatMap((c) => c.results.flatMap((r) => r.events.map((e) => e.id))),
    ) + 1;
  const readIdentity = () => {
    try {
      return Number(storage?.getItem(key)) || 0;
    } catch {
      return 0;
    }
  };
  const saveIdentity = (id) => {
    try {
      if (!storage) {
        return false;
      }
      if (id) {
        storage.setItem(key, String(id));
      } else {
        storage.removeItem(key);
      }
      return readIdentity() === id;
    } catch {
      return false;
    }
  };
  const settings = {
    revision: 1,
    policy: {
      ...features,
      verification_expires: true,
      verification_validity_seconds: DAY,
      verification_recheck_seconds: 21600,
    },
    updated_at: now - DAY,
    updated_by: 1,
  };

  async function execute(request, { signal } = {}) {
    if (signal?.aborted) {
      throw new Error('aborted');
    }
    const url = new URL(request.url);
    const path = url.pathname.replace(/\/$/, '');
    const method = request.method;
    const current = data.users.find(
      (u) => u.id === readIdentity() && !u.account_locked && !u.password_change_required,
    );
    let body = {};
    if (!['GET', 'HEAD'].includes(method)) {
      try {
        const raw = await request.text();
        if (new TextEncoder().encode(raw).length > 8 * 1024 * 1024) {
          return failure(413);
        }
        body = raw
          ? request.headers.get('content-type')?.includes('application/x-www-form-urlencoded')
            ? Object.fromEntries(new URLSearchParams(raw))
            : JSON.parse(raw)
          : {};
        if (!body || Array.isArray(body) || typeof body !== 'object') {
          return failure(400);
        }
      } catch {
        return failure(400);
      }
    }
    if (path === '/auth/session' && method === 'GET') {
      return json({
        features: current ? features : null,
        preferences,
        csrf_token: 'demo-csrf',
        user: current
          ? {
              id: current.id,
              username: current.username,
              role: current.role,
              password_change_required: false,
              permissions:
                current.role.slug === 'admin'
                  ? ['modify_objects', 'modify_system']
                  : ['modify_objects'],
            }
          : null,
        actor: current && current.id !== 1 ? 'demo' : null,
      });
    }
    if (path === '/auth/login' && method === 'POST') {
      if (body.username !== 'demo' || body.password !== 'demodemo') {
        return failure(401, 'AUTH_INVALID_CREDENTIALS');
      }
      if (!saveIdentity(1)) {
        return failure(503, 'DEMO_STORAGE_REQUIRED');
      }
      const next =
        typeof body.next === 'string' &&
        /^\/(?:$|(?:groups|templates|pages|learning-pages|smtp|campaigns|domains|webhooks|reporting-mailboxes|users|user|settings)(?:[/?]|$))/.test(
          body.next,
        ) &&
        !/[\\\r\n]/.test(body.next)
          ? body.next
          : '/';
      return success(next);
    }
    if (!current) {
      return failure(401);
    }
    if (path === '/auth/logout' && method === 'POST') {
      return saveIdentity(0) ? success('/login') : failure(503, 'DEMO_STORAGE_REQUIRED');
    }
    if (path === '/auth/impersonate' && method === 'POST') {
      const target = data.users.find(
        (u) => u.username === body.username && !u.account_locked && !u.password_change_required,
      );
      return current.id === 1 && target && saveIdentity(target.id) ? success('/') : failure(403);
    }
    if (path === '/auth/impersonate/exit' && method === 'POST') {
      return saveIdentity(1) ? success('/') : failure(503);
    }
    if (path === '/internal/preferences' && method === 'PUT') {
      return json(preferences);
    }
    if (['/internal/password', '/auth/password/reset'].includes(path) && method === 'POST') {
      return success('/user');
    }
    if (path === '/api/reset' && method === 'POST') {
      return success('demo-key-not-a-real-credential');
    }
    if (path === '/api/settings' && ['GET', 'PUT'].includes(method)) {
      return json(settings);
    }
    if (path === '/api/overview' && method === 'GET') {
      return json(overview(data));
    }
    if (path === '/api/overview/posture' && method === 'GET') {
      const window = url.searchParams.get('window') ?? '30d';
      const days = { '7d': 7, '30d': 30, '365d': 365 }[window];
      if (!days && window !== 'all') {
        return failure(400);
      }
      const since = days ? now - days * DAY : null;
      const results = data.campaigns.flatMap((c) => c.results);
      const previous =
        since === null
          ? null
          : {
              since: since - days * DAY,
              until: since,
              counts: posture(results, since - days * DAY, since),
            };
      return json({
        window,
        since,
        as_of: now,
        ...posture(results, since ?? 0, now + 1),
        previous,
      });
    }
    if (path === '/api/campaigns/directory' && method === 'GET') {
      const all = [
        ...data.drafts.map((d) => ({
          id: d.id,
          kind: 'draft',
          revision: d.revision,
          name: d.configuration.name,
          status: 'draft',
          total: null,
          launch_date: null,
        })),
        ...data.campaigns.map((c) => ({
          id: String(c.header.id),
          kind: 'campaign',
          revision: null,
          name: c.header.name,
          status: c.header.status,
          total: c.header.total,
          launch_date: c.header.launch_date,
        })),
      ];
      const filter = url.searchParams.get('filter') ?? '';
      const filtered = all.filter(
        (r) =>
          !filter ||
          filter === 'all' ||
          (filter === 'drafts'
            ? r.kind === 'draft'
            : filter === 'executions'
              ? r.kind === 'campaign'
              : filter === 'active'
                ? ['In progress', 'Queued'].includes(r.status)
                : filter === 'completed'
                  ? r.status === 'Completed'
                  : false),
      );
      try {
        if (!['', 'all', 'drafts', 'executions', 'active', 'completed'].includes(filter)) {
          return failure(400);
        }
        if (url.searchParams.has('sort')) {
          const draft = !['executions', 'active', 'completed'].includes(filter);
          const execution = filter !== 'drafts';
          const completed = filter === 'completed' ? 1 : filter === 'active' ? 2 : 0;
          const page = keyset(
            filtered.map((row) => ({ ...row, launch: row.launch_date })),
            url.searchParams,
            ['name', 'status', 'total', 'launch'],
            `directory:${current.id}:${draft}:${execution}:${completed}`,
            50,
          );
          return json({
            items: page.items.map(({ launch: _launch, ...row }) => row),
            next_after: page.cursor,
          });
        }
        if (url.searchParams.has('order')) {
          return failure(400);
        }
        const after = url.searchParams.get('after');
        const start = after
          ? filtered.findIndex((row) => `${row.kind}:${row.id}` === after) + 1
          : 0;
        if (after && !start) {
          return failure(400);
        }
        const items = filtered.slice(start, start + 50);
        return json({
          items,
          next_after:
            filtered.length > start + 50 ? `${items.at(-1).kind}:${items.at(-1).id}` : null,
        });
      } catch {
        return failure(400);
      }
    }
    if (path === '/api/campaigns/complete-all/preview' && method === 'POST') {
      return json({
        token: 'demo-confirmation',
        expires_at: now + 600,
        campaigns: 2,
        pending_deliveries: 12,
      });
    }
    if (path === '/api/campaigns/complete-all' && method === 'POST') {
      return body.token === 'demo-confirmation' && body.confirmation === 'CLOSE ALL'
        ? json({ campaigns: 2, cancelled_deliveries: 12, completed_at: now })
        : failure(400);
    }
    if (path === '/api/campaign-drafts/prepare' && method === 'POST') {
      try {
        return json(review(data, body));
      } catch {
        return failure(400);
      }
    }
    const draftMatch = path.match(/^\/api\/campaign-drafts\/([^/]+)(\/launch)?$/);
    if (draftMatch) {
      const draft =
        data.drafts.find((d) => d.id === draftMatch[1]) ??
        (method === 'PUT' &&
        !draftMatch[2] &&
        /^[\da-f]{8}-[\da-f]{4}-4[\da-f]{3}-[89ab][\da-f]{3}-[\da-f]{12}$/i.test(draftMatch[1])
          ? data.drafts[0]
          : null);
      if (!draft) {
        return failure(404);
      }
      if (draftMatch[2]) {
        return method === 'POST' ? json({ id: 3 }) : failure(405);
      }
      if (method === 'GET') {
        return json(draft);
      }
      if (method === 'DELETE') {
        return success();
      }
      if (method === 'PUT' && body.configuration && Number.isInteger(body.step)) {
        return json({
          ...draft,
          revision: Number(body.revision ?? 0) + 1,
          step: body.step,
          modified: iso(now),
        });
      }
      return failure(400);
    }
    if (path === '/api/campaigns/test' && method === 'POST') {
      return success();
    }
    const campaignMatch = path.match(/^\/api\/campaigns\/(\d+)(?:\/(.*))?$/);
    if (campaignMatch) {
      const c = data.campaigns.find((c) => c.header.id === Number(campaignMatch[1]));
      const operation = campaignMatch[2] ?? '';
      if (!c) {
        return failure(404);
      }
      if (method === 'GET' && operation === 'context') {
        return json(c.header);
      }
      if (method === 'GET' && operation === 'delivery') {
        try {
          return json(delivery(c, url.searchParams, current.id));
        } catch {
          return failure(400);
        }
      }
      if (method === 'GET' && operation === 'results') {
        try {
          return json(analysis(c, url.searchParams, now));
        } catch {
          return failure(400, 'ANALYSIS_FILTER_INVALID');
        }
      }
      if (method === 'GET' && operation === 'draft') {
        return json({
          name: c.header.name,
          url: c.header.url,
          selection: {
            groups: [{ id: 1 }],
            template: { id: c.header.template.id },
            page: { id: 1 },
            smtp: { id: 1 },
          },
          policy,
        });
      }
      const eventRoute = operation.match(/^recipients\/(\d+)\/events$/);
      if (eventRoute) {
        const r = c.results.find((r) => r.id === Number(eventRoute[1]));
        if (!r || method !== 'POST') {
          return failure(404);
        }
        if (
          !/^[a-zA-Z0-9-]{16,64}$/.test(body.request_id ?? '') ||
          !Number.isSafeInteger(body.revision) ||
          body.revision < 0 ||
          Object.keys(body).some(
            (k) => !['request_id', 'event_id', 'revision', 'action', 'value'].includes(k),
          )
        ) {
          return failure(400);
        }
        const add = body.action === 'add';
        const edit = body.action === 'edit';
        if (
          !['add', 'edit', 'delete'].includes(body.action) ||
          add !== (body.event_id === null) ||
          (add && body.revision !== 0) ||
          (add || edit) !== (body.value !== null)
        ) {
          return failure(400);
        }
        const allowed = ['email_opened', 'link_clicked', 'form_submitted', 'email_reported'];
        let next;
        if (add || edit) {
          const v = body.value;
          const at = Date.parse(v.time) / 1000;
          if (
            !allowed.includes(v.kind) ||
            !Number.isFinite(at) ||
            !/(Z|[+-]\d{2}:\d{2})$/.test(v.time) ||
            at > now ||
            at < Date.parse(c.header.launch_date) / 1000 ||
            (c.header.status === 'Completed' && at > c.end) ||
            Object.keys(v).some((k) => !['kind', 'time', 'presence'].includes(k)) ||
            !v.presence ||
            Object.keys(v.presence).some(
              (k) => !['username_present', 'password_present'].includes(k),
            ) ||
            Object.values(v.presence).some((v) => v !== null && typeof v !== 'boolean') ||
            (v.kind !== 'form_submitted' && Object.values(v.presence).some((v) => v !== null))
          ) {
            return failure(400);
          }
          next = {
            time: new Date(v.time).toISOString(),
            kind: eventLabel({ type: 'known', value: v.kind }),
            username_present: v.presence.username_present ?? null,
            password_present: v.presence.password_present ?? null,
          };
        }
        const same = (e) =>
          next &&
          Date.parse(e.time) === Date.parse(next.time) &&
          e.kind === next.kind &&
          (e.username_present ?? null) === next.username_present &&
          (e.password_present ?? null) === next.password_present;
        const result = (e) => json({ id: e.id, event: eventRecord(e) });
        if (add) {
          const prior = r.events.find((e) => e.create_key === body.request_id);
          if (prior) {
            return (prior.revision ?? 0) === 0 && same(prior)
              ? result(prior)
              : failure(412, 'RESOURCE_CHANGED');
          }
          const id = nextEventId++;
          const e = {
            id,
            recipient_id: r.id,
            email: r.email,
            source: 'operator',
            revision: 0,
            edited: false,
            create_key: body.request_id,
            ...next,
          };
          r.events.push(e);
          return result(e);
        }
        const e = r.events.find((e) => e.id === body.event_id);
        if (!e) {
          return failure(404);
        }
        if (!allowed.includes(eventKinds[e.kind])) {
          return failure(400);
        }
        if (body.revision !== (e.revision ?? 0)) {
          return edit && body.revision + 1 === e.revision && same(e)
            ? result(e)
            : failure(412, 'RESOURCE_CHANGED');
        }
        if (body.action === 'delete') {
          r.events = r.events.filter((row) => row.id !== e.id);
          return json({ id: e.id, event: null });
        }
        if (!same(e)) {
          Object.assign(e, next, { edited: true, revision: (e.revision ?? 0) + 1 });
        }
        return result(e);
      }
      const history = operation.match(/^recipients\/(\d+)\/history$/);
      if (method === 'GET' && history) {
        const r = c.results.find((r) => r.id === Number(history[1]));
        if (!r) {
          return failure(404);
        }
        return json({
          result_id: r.id,
          email: r.email,
          attempts: r.attempts,
          events: r.events.map((e) => ({
            record: eventRecord(e),
            id: e.id,
            time: e.time,
            at: Date.parse(e.time) / 1000,
            message: e.kind,
            details: JSON.stringify(
              e.password_present === null ? {} : { password_present: e.password_present },
            ),
          })),
          next_attempt: null,
          next_event: null,
        });
      }
      if (
        (method === 'DELETE' && !operation) ||
        (method === 'POST' && (operation === 'complete' || /^delivery\/\d+$/.test(operation)))
      ) {
        return success();
      }
      if (method === 'GET' && operation === 'export') {
        const selected = url.searchParams.get('recipients')
          ? url.searchParams.get('recipients').split(',')
          : [];
        const ids = [
          ...selected,
          ...(url.searchParams.has('recipient') ? [url.searchParams.get('recipient')] : []),
        ];
        const invalidNumber = ['from', 'until', 'offset', 'limit'].some(
          (key) =>
            url.searchParams.has(key) &&
            (!/^-?\d+$/.test(url.searchParams.get(key)) ||
              !Number.isSafeInteger(Number(url.searchParams.get(key)))),
        );
        const limit = Number(url.searchParams.get('limit') ?? 50);
        const offset = Number(url.searchParams.get('offset') ?? 0);
        const invalidWindow =
          url.searchParams.has('from') &&
          url.searchParams.has('until') &&
          Number(url.searchParams.get('from')) >= Number(url.searchParams.get('until'));
        if (
          invalidNumber ||
          invalidWindow ||
          limit < 1 ||
          limit > 200 ||
          offset < 0 ||
          offset > 1000000 ||
          selected.length > 200 ||
          ids.some((id) => !/^\d+$/.test(id) || !c.results.some((r) => r.id === Number(id)))
        ) {
          return failure(400, 'ANALYSIS_FILTER_INVALID');
        }
        try {
          return exportResponse(analysis(c, url.searchParams, now, false), url.searchParams);
        } catch {
          return failure(400, 'ANALYSIS_FILTER_INVALID');
        }
      }
      return failure(404);
    }
    if (
      ['/api/util/preview', '/api/pages/preview', '/api/learning-pages/preview'].includes(path) &&
      method === 'POST'
    ) {
      return json({
        html: '<h1>Sample preview</h1><p>The public demo displays sample content. Submitted content is not sent to a server.</p>',
        warnings: [],
      });
    }
    if (
      ['/api/pages/validate', '/api/learning-pages/validate'].includes(path) &&
      method === 'POST'
    ) {
      return json({ warnings: [] });
    }
    if (
      ['/api/util/test_connection', '/api/util/send_test_email'].includes(path) &&
      method === 'POST'
    ) {
      return success();
    }
    if (path === '/api/import/site' && method === 'POST') {
      return json({ html: data.pages[0].html });
    }
    if (path === '/api/import/email' && method === 'POST') {
      return json({ ...data.templates[0], id: 0 });
    }
    if (path === '/api/import/group/review' && method === 'POST') {
      try {
        return json(reviewCsv(body));
      } catch {
        return failure(400, 'CSV_INVALID');
      }
    }
    if (path.startsWith('/api/starter-templates') && method === 'GET') {
      const entries = starterCatalog;
      if (path === '/api/starter-templates') {
        return json(entries.map((e) => e.summary));
      }
      const entry = entries.find((e) => e.summary.id === path.split('/').at(-1));
      return entry ? json(entry) : failure(404);
    }
    const match = path.match(/^\/api\/([^/]+)(?:\/([^/]+))?(?:\/([^/]+))?$/);
    if (match && families.includes(match[1])) {
      const [, family, identity, operation] = match;
      const rows =
        family === 'learning-pages'
          ? data[family].map((row) => ({ ...row, preview_html: row.html }))
          : data[family];
      if (
        method === 'GET' &&
        identity === 'summary' &&
        !operation &&
        ['groups', 'templates', 'pages', 'learning-pages', 'smtp'].includes(family)
      ) {
        return json(summaries(data, family));
      }
      if (method === 'GET' && family === 'domains' && identity === 'validated') {
        return json(rows.filter((r) => r.eligible));
      }
      const row = identity ? rows.find((r) => r.id === Number(identity)) : null;
      if (identity && !row) {
        return failure(404);
      }
      if (operation) {
        if (
          method === 'POST' &&
          ((family === 'domains' && operation === 'verify') ||
            (family === 'webhooks' && operation === 'test') ||
            (family === 'reporting-mailboxes' &&
              ['validate', 'check', 'resume'].includes(operation)))
        ) {
          return json(row);
        }
        return failure(404);
      }
      if (method === 'GET') {
        return json(identity ? row : rows);
      }
      if (method === 'DELETE' && row) {
        return success();
      }
      if ((method === 'POST' && !identity) || (method === 'PUT' && row)) {
        if (
          typeof (body.name ?? body.username) !== 'string' ||
          !(body.name ?? body.username).trim()
        ) {
          return failure(400);
        }
        // Return an existing sample identity; subsequent reads always return the original fixture.
        return json(copy(row ?? rows[0]), method === 'POST' ? 201 : 200);
      }
      return failure(405);
    }
    return failure(404);
  }
  return { execute };
}
