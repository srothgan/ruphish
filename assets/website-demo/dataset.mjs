// Shared synthetic scenario. No SQL, network, credentials, or submitted user content.
import source from './data.json' with { type: 'json' };
import { starterContent } from './starter-content.mjs';

export const DAY = 86400;
export const features = Object.freeze(source.features);
export const iso = seconds => new Date(seconds * 1000).toISOString();
export const draftId = n => `00000000-0000-4000-8000-${String(n).padStart(12, '0')}`;
export const policy = { redirect_url: '' };

export function dataset(now) {
    if (!Number.isSafeInteger(now) || now <= 0) throw new Error('A fixed positive snapshot time is required');
    const modified_date = iso(now - DAY);
    const resource = (name, i, fields) => ({ id: i + 1, name, modified_date, ...fields });
    const recipients = Array.from({ length: 617 }, (_, n) => ({
        first_name: source.first_names[n % source.first_names.length],
        last_name: source.last_names[Math.floor(n / source.first_names.length) % source.last_names.length],
        email: `person${String(n + 1).padStart(3, '0')}@example.test`,
        position: source.departments[n % source.departments.length],
    }));
    const groups = source.departments.map((name, i) => resource(name, i, {
        targets: recipients.filter((_, n) => n % source.departments.length === i),
    }));
    groups.push(resource('New starters', 4, { targets: recipients.slice(0, 12) }), resource('Next exercise audience', 5, { targets: [] }));
    const templates = source.templates.map((entry, i) => resource(entry.name, i, {
        ...starterContent.templates[i], envelope_sender: '', attachments: [],
    }));
    const pages = source.pages.map((entry, i) => resource(entry.name, i, {
        ...starterContent.pages[i],
        ...policy,
    }));
    const learning = source.learning_pages.map((entry, i) => resource(entry.name, i, {
        public_id: String(i + 1).padStart(32, '0'), ...starterContent.learning_pages[i],
    }));
    const smtp = ['Awareness team', 'Training team'].map((name, i) => resource(name, i, {
        interface_type: 'SMTP', host: 'smtp.example.test:587', from_address: `${name} <awareness@example.test>`,
        username: '', has_password: false, headers: [], ignore_cert_errors: false,
    }));
    const users = source.users.map((u, i) => ({
        id: i + 1, username: u.username,
        role: { slug: u.role, name: u.role === 'admin' ? 'Admin' : 'User', description: 'Synthetic demonstration account' },
        password_change_required: u.reset ?? false, account_locked: u.locked ?? false,
        api_key: 'demo-key-not-a-real-credential', last_login: iso(now - (i + 1) * 3600),
    }));
    const domains = ['Awareness exercises', 'Pending verification', 'Disabled training domain'].map((name, i) => ({
        id: i + 1, name, origin: `https://${['awareness', 'pending', 'disabled'][i]}.example.test`,
        hostname: `${['awareness', 'pending', 'disabled'][i]}.example.test`, enabled: i !== 2, revision: 1,
        txt_name: `_ruphish.${['awareness', 'pending', 'disabled'][i]}.example.test`, txt_value: 'demo-verification-only',
        challenge_expires: now + DAY, checked_at: now - 3600, verified_until: i === 0 ? now + DAY : 0,
        control: i === 0, dns: i === 0, routing: i === 0, tls: i === 0,
        code: i === 0 ? 'verified' : 'dns_pending', status: i === 0 ? 'verified' : 'unverified',
        eligible: i === 0, checking: false, active_check: null, history: [], next_check_at: now + 3600,
        campaigns: i === 0 ? 4 : 0,
        verification: { required: true, status: i === 0 ? 'verified' : 'unverified', expires_at: i === 0 ? now + DAY : null, next_check_at: now + 3600 },
    }));
    const run = (id, failed, webhook) => ({
        id, event_id: webhook ? id : null, attempt: webhook ? 1 : null, started: now - id * 300,
        finished: now - id * 300 + 2, status: webhook ? (failed ? 503 : 200) : null,
        code: failed ? 'connection_failed' : 'ok', matched: webhook ? 0 : 12, rejected: 0,
    });
    const webhooks = source.webhooks.map((w, i) => ({
        id: i + 1, name: w.name, url: `https://hooks.example.test/${w.path}`, is_active: w.active,
        revision: 1, has_secret: true, history: [run(i + 1, w.failed, true)],
    }));
    const mailboxes = source.mailboxes.map((name, i) => ({
        id: i + 1, name, enabled: i === 0, host: 'imap.example.test', port: 993,
        username: `reports${i + 1}@example.test`, has_password: true, tls: true, ignore_cert_errors: false, folder: 'INBOX',
        restrict_domain: 'example.test', delete_reported: false, frequency: 300, revision: 1,
        last_check: now - 300, last_success: now - 300, next_check: now + 300, running: false,
        code: i === 0 ? 'ok' : 'connection_failed', history: [run(i + 1, i !== 0, false)],
    }));
    const configs = [...source.campaigns, { name: source.scheduled, ago: -3, size: 12, opened: 0, clicked: 0, submitted: 0, reported: 0, complete: false, failed: 0 }];
    const campaigns = configs.map((c, index) => {
        const id = index + 1, launch = now - c.ago * DAY;
        const status = c.ago < 0 ? 'Queued' : c.complete ? 'Completed' : 'In progress';
        const template = templates[index % templates.length];
        const header = {
            id, name: c.name, status, launch_date: iso(launch), send_by_date: null, url: domains[0].origin,
            template: { id: template.id, name: template.name }, page: { id: 1, name: pages[0].name }, smtp: { id: 1, name: smtp[0].name },
            subject: template.subject, profile_sender: smtp[0].from_address, template_sender: '',
            total: c.size + c.failed, policy, domain_id: 1, domain_block: null,
        };
        let eventId = id * 100000;
        const results = Array.from({ length: c.size + c.failed }, (_, n) => {
            const failed = n >= c.size;
            const person = failed ? { first_name: 'Demo', last_name: `Recipient ${n - c.size + 1}`, email: `undeliverable${String(n - c.size + 1).padStart(2, '0')}@example.test`, position: 'Operations' } : recipients[n];
            const resultId = id * 10000 + n + 1, sent = launch + n * 7;
            const state = c.ago < 0 ? 'ready' : failed ? 'failed' : 'accepted';
            const events = [];
            const add = (kind, delay, password_present = null) => events.push({ id: ++eventId, recipient_id: resultId, email: person.email, time: iso(sent + delay), kind, source: 'application', password_present });
            if (state === 'accepted') {
                add('Email Sent', 0);
                if (n < c.opened) add('Email Opened', 900);
                if (n < c.clicked) add('Clicked Link', 1800);
                if (n < c.submitted) add('Submitted Data', 2100, true);
                if (n >= c.size - c.reported) add('Email Reported', 3600);
            }
            const attempts = state === 'ready' ? [] : [{ generation: 1, started: sent, finished: sent + 1, state, stage: failed ? 'Envelope' : 'Data', smtp_code: failed ? 550 : 250, message_id: `demo-${resultId}@example.test` }];
            return { ...person, id: resultId, rid: `demo-recipient-${resultId}`, state, due: sent, send_date: iso(sent), modified_date: iso(sent), attempts, events };
        });
        return { header, results };
    });
    const selection = { groups: [{ id: 1 }], template: { id: 1 }, page: { id: 1 }, smtp: { id: 1 } };
    const drafts = source.drafts.map((name, i) => ({
        id: draftId(i + 1), revision: 1, configuration: { name, url: domains[0].origin, selection, policy, launch_local: null, send_by_local: null },
        step: i, created: iso(now - DAY), modified: iso(now - 3600), campaign_id: null, status: 'draft',
    }));
    const starters = [['templates', 'email', templates], ['pages', 'landing_page', pages], ['learning_pages', 'learning_page', learning]].flatMap(([family, kind, resources]) => source[family].map((entry, i) => ({
        summary: { id: `${{ email: 'email', landing_page: 'page', learning_page: 'learning' }[kind]}-${entry.starter}`, name: entry.name, brand: entry.brand, kind, prefill: kind === 'landing_page', company_logo: entry.company_logo ?? (kind === 'learning_page' ? { selector: '.learning-logo', height: 48, context: 'Learning page header' } : null) },
        resource: resources[i],
    })));
    return { now, groups, templates, pages, 'learning-pages': learning, smtp, users, domains, webhooks, 'reporting-mailboxes': mailboxes, campaigns, drafts, starters };
}
