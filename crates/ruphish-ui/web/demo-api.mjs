// Browser-only simulation of the existing wire contracts. No HTTP fallback or persistent resources.
import { dataset, DAY, iso, policy } from '../../../assets/website-demo/dataset.mjs';
import { workbook } from './demo-export.mjs';

const kinds = ['Email Sent', 'Email Opened', 'Clicked Link', 'Submitted Data', 'Email Reported'];
const families = ['groups', 'templates', 'pages', 'learning-pages', 'smtp', 'domains', 'users', 'webhooks', 'reporting-mailboxes'];
export const preferences = { language: null, format_locale: null, time_zone: null, show_utc_offset: true };
const features = { verification_enabled: true, reporting_mailboxes_enabled: true, webhooks_enabled: true };
const copy = value => structuredClone(value);
const json = (value, status = 200) => new Response(JSON.stringify(value), { status, headers: { 'content-type': 'application/json', etag: '"1"' } });
const success = (data = null) => json({ success: true, message: 'Demo action simulated', data });
const failure = (status, code = 'REQUEST_FAILED') => json({ success: false, code, message: 'Demo request rejected', data: null }, status);
const counts = events => kinds.map(kind => events.filter(e => e.kind === kind).length);
const sum = rows => rows.reduce((a, b) => a.map((v, i) => v + b[i]), [0, 0, 0, 0, 0]);
const byTime = (a, b) => a.time.localeCompare(b.time) || a.id - b.id;

function posture(results, since = 0, until = Infinity) {
    const rows = results.filter(r => r.state === 'accepted' && r.due >= since && r.due < until);
    const flags = sum(rows.map(r => counts(r.events.filter(e => Date.parse(e.time) / 1000 < until)).map(n => Number(n > 0))));
    return { delivered: rows.length, opened: flags[1], clicked: flags[2], credentials_entered: flags[3], reported: flags[4] };
}

export function overview(data) {
    const headers = data.campaigns.map(c => c.header);
    const scheduled = headers.filter(c => c.status === 'Queued').sort((a, b) => a.launch_date.localeCompare(b.launch_date));
    return {
        as_of: data.now,
        campaigns: { total: headers.length, active: headers.filter(c => c.status === 'In progress').length, scheduled: scheduled.length, completed: headers.filter(c => c.status === 'Completed').length },
        recent: [...data.campaigns].reverse().map(c => ({ id: c.header.id, name: c.header.name, status: c.header.status, launch: c.header.launch_date, recipients: c.results.length, outcomes: posture(c.results) })),
        inventory: { templates: data.templates.length, pages: data.pages.length, groups: data.groups.length, profiles: data.smtp.length, domains: data.domains.length, domains_verified: data.domains.filter(d => d.eligible).length, reporting_mailboxes: data['reporting-mailboxes'].length },
        attention: { unverified_enabled_domains: data.domains.filter(d => d.enabled && !d.eligible).length, delivery_problems: data.campaigns.filter(c => c.header.status !== 'Completed').flatMap(c => c.results).filter(r => r.state === 'failed').length, next_launch: scheduled.length ? { id: scheduled[0].id, name: scheduled[0].name, launch: scheduled[0].launch_date } : null },
    };
}

export function analysis(campaign, query, now, paginate = true) {
    const from = Number(query.get('from') ?? 0), until = Number(query.get('until') ?? now);
    const selected = new Set((query.get('recipients') ?? '').split(',').filter(Boolean).map(Number));
    const search = (query.get('search') ?? '').toLocaleLowerCase();
    const results = campaign.results.filter(r => (!query.has('recipient') || r.id === Number(query.get('recipient'))) && (!selected.size || selected.has(r.id)) && (!search || `${r.email} ${r.first_name} ${r.last_name}`.toLocaleLowerCase().includes(search)));
    const events = results.flatMap(r => r.events).filter(e => {
        const at = Date.parse(e.time) / 1000;
        return at >= from && at <= until && (!query.get('event') || query.get('event') === e.kind) && (!query.get('source') || query.get('source') === e.source);
    }).sort(byTime);
    const eventMap = new Map();
    for (const e of events) {
        if (!eventMap.has(e.recipient_id)) eventMap.set(e.recipient_id, []);
        eventMap.get(e.recipient_id).push(e);
    }
    const rows = results.filter(r => (!query.get('event') && !query.get('source')) || eventMap.has(r.id)).map(r => ({ id: r.id, email: r.email, name: `${r.first_name} ${r.last_name}`, delivery: r.state, counts: counts(eventMap.get(r.id) ?? []), historical_reported: false }));
    const unique = sum(rows.map(r => r.counts.map(n => Number(n > 0))));
    const progression = sum(rows.map(r => [Number(r.counts[0] > 0), Number(r.counts[1] + r.counts[2] + r.counts[3] > 0), Number(r.counts[2] + r.counts[3] > 0), Number(r.counts[3] > 0), Number(r.counts[4] > 0)]));
    const buckets = new Map();
    for (const e of events) {
        const time = Math.floor(Date.parse(e.time) / 1000 / 3600) * 3600;
        if (!buckets.has(time)) buckets.set(time, [0, 0, 0, 0, 0]);
        buckets.get(time)[kinds.indexOf(e.kind)]++;
    }
    const offset = Math.max(0, Number(query.get('offset') ?? 0)), limit = Math.max(1, Math.min(200, Number(query.get('limit') ?? 50)));
    return { id: campaign.header.id, name: campaign.header.name, status: campaign.header.status, as_of: now, audience: campaign.results.length, unique, progression, totals: counts(events), recipient_total: rows.length, event_total: events.length, recipients: paginate ? rows.slice(offset, offset + limit) : rows, events: paginate ? [...events].reverse().slice(offset, offset + limit) : events, buckets: [...buckets].map(([time, values]) => ({ time, counts: values })), bucket_seconds: 3600 };
}

function summaries(data, family) {
    const rows = data[family].map(r => ({ id: r.id, name: r.name, modified_date: r.modified_date, ...(family === 'groups' ? { num_targets: r.targets.length } : { active_campaigns: data.campaigns.filter(c => c.header.status !== 'Completed' && c.header[{ templates: 'template', pages: 'page', smtp: 'smtp' }[family]]?.id === r.id).length }) }));
    return { total: rows.length, [family === 'groups' ? 'groups' : 'items']: rows };
}

function delivery(campaign, query) {
    const after = Number(query.get('after') ?? 0), matches = campaign.results.filter(r => r.id > after), page = matches.slice(0, 50);
    const counts = {};
    for (const r of campaign.results) counts[r.state] = (counts[r.state] ?? 0) + 1;
    return { campaign: campaign.header, counts, items: page.map(r => ({ id: r.id, rid: r.rid, email: r.email, attempt_count: r.attempts.length, state: r.state, due: r.due, retries: 0, generation: 1, code: r.state === 'failed' ? 'smtp_permanent_rejection' : '', attempts: r.attempts })), next_after: matches.length > page.length ? page.at(-1).id : null, policy: campaign.header.policy, mailer_enabled: false, activity: campaign.results.flatMap(r => r.events).sort(byTime).slice(-25).reverse().map(e => ({ id: e.id, time: e.time, email: e.email, message: e.kind, details: JSON.stringify(e.password_present === null ? {} : { password_present: e.password_present }) })) };
}

function review(data, input) {
    const select = (family, ref) => {
        const r = data[family].find(r => r.id === ref?.id);
        if (!r) throw new Error('missing_selection');
        return { id: r.id, name: r.name, revision: '1' };
    };
    const selection = { groups: (input.selection?.groups ?? []).map(r => select('groups', r)), template: select('templates', input.selection?.template), page: input.selection?.page?.id === 0 ? { id: 0, name: '', revision: '' } : select('pages', input.selection?.page), smtp: select('smtp', input.selection?.smtp) };
    const targets = selection.groups.flatMap(g => data.groups.find(r => r.id === g.id).targets);
    const total = new Set(targets.map(r => r.email)).size;
    if (!total) throw new Error('empty_audience');
    const template = data.templates.find(r => r.id === selection.template.id), smtp = data.smtp.find(r => r.id === selection.smtp.id);
    return { warnings: [], total, duplicate_count: targets.length - total, launch_date: data.now, send_by_date: null, selection, policy: { ...policy, ...(input.policy ?? {}) }, mailer_enabled: false, conflict_count: 0, conflicts: [], preview: { subject: template.subject, html: '<h1>Demo campaign preview</h1><p>This is a sample preview. No messages will be sent.</p>', text: template.text, visible_sender: smtp.from_address, transport_sender: smtp.from_address } };
}

export function createDemoApi({ now = Math.floor(Date.now() / 1000), storage } = {}) {
    const data = dataset(now);
    const key = 'ruphish-demo-session-v1';
    const readIdentity = () => {
        try { return Number(storage?.getItem(key)) || 0; } catch { return 0; }
    };
    const saveIdentity = id => {
        try {
            if (!storage) return false;
            if (id) storage.setItem(key, String(id)); else storage.removeItem(key);
            return readIdentity() === id;
        } catch { return false; }
    };
    const settings = { revision: 1, policy: { ...features, verification_expires: true, verification_validity_seconds: DAY, verification_recheck_seconds: 21600 }, updated_at: now - DAY, updated_by: 1 };

    async function execute(request, { signal } = {}) {
        if (signal?.aborted) throw new Error('aborted');
        const url = new URL(request.url), path = url.pathname.replace(/\/$/, ''), method = request.method;
        const current = data.users.find(u => u.id === readIdentity() && !u.account_locked && !u.password_change_required);
        let body = {};
        if (!['GET', 'HEAD'].includes(method)) {
            try {
                const raw = await request.text();
                if (raw.length > 8 * 1024 * 1024) return failure(413);
                body = raw ? request.headers.get('content-type')?.includes('application/x-www-form-urlencoded') ? Object.fromEntries(new URLSearchParams(raw)) : JSON.parse(raw) : {};
                if (!body || Array.isArray(body) || typeof body !== 'object') return failure(400);
            } catch { return failure(400); }
        }
        if (path === '/auth/session' && method === 'GET') return json({ features: current ? features : null, preferences, csrf_token: 'demo-csrf', user: current ? { id: current.id, username: current.username, role: current.role, password_change_required: false, permissions: current.role.slug === 'admin' ? ['modify_objects', 'modify_system'] : ['modify_objects'] } : null, actor: current && current.id !== 1 ? 'demo' : null });
        if (path === '/login' && method === 'POST') {
            if (body.username !== 'demo' || body.password !== 'demodemo') return failure(401, 'AUTH_INVALID_CREDENTIALS');
            if (!saveIdentity(1)) return failure(503, 'DEMO_STORAGE_REQUIRED');
            const next = typeof body.next === 'string' && /^\/(?:$|(?:groups|templates|pages|learning-pages|smtp|campaigns|analysis|domains|webhooks|reporting-mailboxes|users|user|settings)(?:[/?]|$))/.test(body.next) && !/[\\\r\n]/.test(body.next) ? body.next : '/';
            return success(next);
        }
        if (!current) return failure(401);
        if (path === '/logout' && method === 'POST') return saveIdentity(0) ? success('/login') : failure(503, 'DEMO_STORAGE_REQUIRED');
        if (path === '/impersonate' && method === 'POST') {
            const target = data.users.find(u => u.username === body.username && !u.account_locked && !u.password_change_required);
            return current.id === 1 && target && saveIdentity(target.id) ? success('/') : failure(403);
        }
        if (path === '/impersonate/exit' && method === 'POST') return saveIdentity(1) ? success('/') : failure(503);
        if (path === '/internal/preferences' && method === 'PUT') return json(preferences);
        if (['/internal/password', '/reset_password'].includes(path) && method === 'POST') return success('/user');
        if (path === '/api/reset' && method === 'POST') return success('demo-key-not-a-real-credential');
        if (path === '/api/settings' && ['GET', 'PUT'].includes(method)) return json(settings);
        if (path === '/api/overview' && method === 'GET') return json(overview(data));
        if (path === '/api/overview/posture' && method === 'GET') {
            const window = url.searchParams.get('window') ?? '30d', days = { '7d': 7, '30d': 30, '365d': 365 }[window];
            if (!days && window !== 'all') return failure(400);
            const since = days ? now - days * DAY : null, results = data.campaigns.flatMap(c => c.results);
            const previous = since === null ? null : { since: since - days * DAY, until: since, counts: posture(results, since - days * DAY, since) };
            return json({ window, since, as_of: now, ...posture(results, since ?? 0, now + 1), previous });
        }
        if (path === '/api/campaigns/directory' && method === 'GET') {
            const all = [...data.drafts.map(d => ({ id: d.id, kind: 'draft', revision: d.revision, name: d.configuration.name, status: 'Draft', total: null, launch_date: null })), ...data.campaigns.map(c => ({ id: String(c.header.id), kind: 'campaign', revision: null, name: c.header.name, status: c.header.status, total: c.header.total, launch_date: c.header.launch_date }))];
            const filter = url.searchParams.get('filter') ?? '';
            const filtered = all.filter(r => !filter || filter === 'all' || (filter === 'drafts' ? r.kind === 'draft' : filter === 'active' ? ['In progress', 'Queued'].includes(r.status) : filter === 'completed' ? r.status === 'Completed' : false));
            return json({ items: url.searchParams.get('after') ? [] : filtered, next_after: null });
        }
        if (path === '/api/campaigns/complete-all/preview' && method === 'POST') return json({ token: 'demo-confirmation', expires_at: now + 600, campaigns: 2, pending_deliveries: 12 });
        if (path === '/api/campaigns/complete-all' && method === 'POST') return body.token === 'demo-confirmation' && body.confirmation === 'CLOSE ALL' ? json({ campaigns: 2, cancelled_deliveries: 12, completed_at: now }) : failure(400);
        if (path === '/api/campaign-drafts/prepare' && method === 'POST') {
            try { return json(review(data, body)); } catch { return failure(400); }
        }
        const draftMatch = path.match(/^\/api\/campaign-drafts\/([^/]+)(\/launch)?$/);
        if (draftMatch) {
            const draft = data.drafts.find(d => d.id === draftMatch[1])
                ?? (method === 'PUT' && !draftMatch[2] && /^[\da-f]{8}-[\da-f]{4}-4[\da-f]{3}-[89ab][\da-f]{3}-[\da-f]{12}$/i.test(draftMatch[1]) ? data.drafts[0] : null);
            if (!draft) return failure(404);
            if (draftMatch[2]) return method === 'POST' ? json({ id: 3 }) : failure(405);
            if (method === 'GET') return json(draft);
            if (method === 'DELETE') return success();
            if (method === 'PUT' && body.configuration && Number.isInteger(body.step)) return json({ ...draft, revision: Number(body.revision ?? 0) + 1, step: body.step, modified: iso(now) });
            return failure(400);
        }
        if (path === '/api/campaigns/test' && method === 'POST') return success();
        const campaignMatch = path.match(/^\/api\/campaigns\/(\d+)(?:\/(.*))?$/);
        if (campaignMatch) {
            const c = data.campaigns.find(c => c.header.id === Number(campaignMatch[1])), operation = campaignMatch[2] ?? '';
            if (!c) return failure(404);
            if (method === 'GET' && operation === 'delivery') return json(delivery(c, url.searchParams));
            if (method === 'GET' && operation === 'analysis') return json(analysis(c, url.searchParams, now));
            if (method === 'GET' && operation === 'draft') return json({ name: c.header.name, url: c.header.url, selection: { groups: [{ id: 1 }], template: { id: c.header.template.id }, page: { id: 1 }, smtp: { id: 1 } }, policy });
            const history = operation.match(/^delivery\/(\d+)\/history$/);
            if (method === 'GET' && history) {
                const r = c.results.find(r => r.id === Number(history[1]));
                if (!r) return failure(404);
                return json({ result_id: r.id, email: r.email, attempts: r.attempts, events: r.events.map(e => ({ id: e.id, time: e.time, at: Date.parse(e.time) / 1000, message: e.kind, details: JSON.stringify(e.password_present === null ? {} : { password_present: e.password_present }) })), next_attempt: null, next_event: null });
            }
            if ((method === 'DELETE' && !operation) || (method === 'POST' && (operation === 'complete' || /^delivery\/\d+$/.test(operation)))) return success();
            if (method === 'GET' && operation === 'export') return exportResponse(analysis(c, url.searchParams, now, false), url.searchParams);
            return failure(405);
        }
        if (['/api/util/preview', '/api/pages/preview', '/api/learning-pages/preview'].includes(path) && method === 'POST') return json({ html: '<h1>Sample preview</h1><p>The public demo displays sample content. Submitted content is not sent to a server.</p>', warnings: [] });
        if (['/api/pages/validate', '/api/learning-pages/validate'].includes(path) && method === 'POST') return json({ warnings: [] });
        if (['/api/util/test_connection', '/api/util/send_test_email'].includes(path) && method === 'POST') return success();
        if (path === '/api/import/site' && method === 'POST') return json({ html: data.pages[0].html });
        if (path === '/api/import/email' && method === 'POST') return json({ ...data.templates[0], id: 0 });
        if (path === '/api/import/group/review' && method === 'POST') return json({ targets: data.groups[4].targets, errors: [], duplicates: 0 });
        if (path.startsWith('/api/starter-templates') && method === 'GET') {
            const entries = ['templates', 'pages', 'learning-pages'].flatMap((family, i) => data[family].map(r => ({ summary: { id: `demo-${family}-${r.id}`, name: r.name, brand: 'Ruphish', kind: ['email', 'landing_page', 'learning_page'][i], prefill: false, company_logo: null }, resource: r })));
            if (path === '/api/starter-templates') return json(entries.map(e => e.summary));
            const entry = entries.find(e => e.summary.id === path.split('/').at(-1));
            return entry ? json(entry) : failure(404);
        }
        const match = path.match(/^\/api\/([^/]+)(?:\/([^/]+))?(?:\/([^/]+))?$/);
        if (match && families.includes(match[1])) {
            const [, family, identity, operation] = match;
            const rows = family === 'learning-pages' ? data[family].map(row => ({ ...row, preview_html: row.html })) : data[family];
            if (method === 'GET' && identity === 'summary' && !operation && ['groups', 'templates', 'pages', 'learning-pages', 'smtp'].includes(family)) return json(summaries(data, family));
            if (method === 'GET' && family === 'domains' && identity === 'validated') return json(rows.filter(r => r.eligible));
            const row = identity ? rows.find(r => r.id === Number(identity)) : null;
            if (identity && !row) return failure(404);
            if (operation) {
                if (method === 'POST' && ((family === 'domains' && operation === 'verify') || (family === 'webhooks' && operation === 'test') || (family === 'reporting-mailboxes' && ['validate', 'check', 'resume'].includes(operation)))) return json(row);
                return failure(404);
            }
            if (method === 'GET') return json(identity ? row : rows);
            if (method === 'DELETE' && row) return success();
            if ((method === 'POST' && !identity) || (method === 'PUT' && row)) {
                if (typeof (body.name ?? body.username) !== 'string' || !(body.name ?? body.username).trim()) return failure(400);
                // Return an existing sample identity; subsequent reads always return the original fixture.
                return json(copy(row ?? rows[0]), method === 'POST' ? 201 : 200);
            }
            return failure(405);
        }
        return failure(404);
    }
    return { execute };
}

function exportResponse(value, query) {
    const format = query.get('format') ?? 'csv', scope = query.get('scope') ?? 'recipients';
    const rows = scope === 'events' ? value.events.map(e => ({ email: e.email, time: e.time, event: e.kind, source: e.source, password_present: e.password_present })) : value.recipients.map(r => ({ email: r.email, name: r.name, delivery: r.delivery, sent: r.counts[0], opened: r.counts[1], clicked: r.counts[2], submitted: r.counts[3], reported: r.counts[4] }));
    if (format === 'json') return json(scope === 'all' ? value : rows);
    if (format === 'xlsx') return new Response(workbook([
        ['Recipients', value.recipients.map(r => ({ email: r.email, name: r.name, delivery: r.delivery, sent: r.counts[0], opened: r.counts[1], clicked: r.counts[2], submitted: r.counts[3], reported: r.counts[4] }))],
        ['Events', value.events.map(e => ({ email: e.email, time: e.time, event: e.kind, source: e.source, password_present: e.password_present }))],
    ]), { headers: { 'content-type': 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet' } });
    if (format !== 'csv') return failure(400, 'DEMO_EXPORT_FORMAT');
    const columns = Object.keys(rows[0] ?? { email: '', name: '' });
    const cell = value => `"${String(value ?? '').replaceAll('"', '""')}"`;
    return new Response([columns, ...rows.map(r => columns.map(k => r[k]))].map(r => r.map(cell).join(',')).join('\r\n'), { headers: { 'content-type': 'text/csv;charset=utf-8' } });
}
