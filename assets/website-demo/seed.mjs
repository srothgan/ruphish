// The SQL consumer of the same complete dataset used by the public browser demo.
import { randomBytes } from 'node:crypto';
import { dataset, DAY, ZERO, iso, features } from './dataset.mjs';

export function seed(db, now = Math.floor(Date.now() / 1000)) {
  const data = dataset(now);
  db.exec('BEGIN IMMEDIATE');
  try {
    const admin = db.prepare("SELECT * FROM users WHERE username='admin'").get();
    if (!admin || db.prepare('SELECT count(*) AS n FROM users').get().n !== 1) throw new Error('Initialize one fresh disposable administrator before seeding.');
    for (const table of ['campaigns', 'results', 'events', 'templates', 'pages', 'groups', 'targets', 'smtp', 'ruphish_domains', 'ruphish_learning_pages', 'ruphish_campaign_drafts', 'webhooks', 'ruphish_mailboxes']) {
      if (db.prepare(`SELECT count(*) AS n FROM ${table}`).get().n !== 0) throw new Error('The recorder requires a fresh database; existing data will not be changed.');
    }
    const owner = admin.id;
    const run = (sql, ...values) => db.prepare(sql).run(...values);
    const insert = (sql, ...values) => Number(run(sql, ...values).lastInsertRowid);
    // Preserve the recorder's random bootstrap password; never use public demo credentials on a server.
    run('UPDATE users SET username=?,password_change_required=0,last_login=? WHERE id=?', data.users[0].username, data.users[0].last_login, owner);
    for (const user of data.users.slice(1)) {
      const role = db.prepare('SELECT id FROM roles WHERE slug=?').get(user.role.slug);
      if (!role) throw new Error('Missing installed role');
      run('INSERT INTO users(username,hash,api_key,role_id,password_change_required,last_login,account_locked) VALUES(?,?,?,?,?,?,?)', user.username, admin.hash, randomBytes(32).toString('hex'), role.id, Number(user.password_change_required), user.last_login, Number(user.account_locked));
    }
    for (const d of data.domains) {
      // Historical state only. Individual domains stay disabled in the recorder.
      run('INSERT INTO ruphish_domains(id,owner_id,name,origin,hostname,enabled,challenge,challenge_expires,checked_at,verified_until,control,dns,routing,tls,code) VALUES(?,?,?,?,?,0,?,?,?,?,?,?,?,?,?)', d.id, owner, d.name, d.origin, d.hostname, randomBytes(32).toString('hex'), 0, d.checked_at, d.verified_until, Number(d.control), Number(d.dns), Number(d.routing), Number(d.tls), d.code);
      run('INSERT INTO ruphish_domain_claims(hostname,owner_id) VALUES(?,?)', d.hostname, owner);
    }
    for (const s of data.smtp) run('INSERT INTO smtp(id,user_id,name,interface_type,host,username,password,from_address,modified_date,ignore_cert_errors) VALUES(?,?,?,?,?,?,?,?,?,?)', s.id, owner, s.name, s.interface_type, s.host, '', '', s.from_address, s.modified_date, 0);
    for (const p of data.pages) run('INSERT INTO pages(id,user_id,name,html,modified_date,redirect_url) VALUES(?,?,?,?,?,?)', p.id, owner, p.name, p.html, p.modified_date, p.redirect_url);
    for (const p of data['learning-pages']) run('INSERT INTO ruphish_learning_pages(id,user_id,name,html,modified_date,public_id) VALUES(?,?,?,?,?,?)', p.id, owner, p.name, p.html, p.modified_date, p.public_id);
    for (const t of data.templates) run('INSERT INTO templates(id,user_id,name,subject,text,html,modified_date) VALUES(?,?,?,?,?,?,?)', t.id, owner, t.name, t.subject, t.text, t.html, t.modified_date);
    const targets = new Map();
    for (const group of data.groups) {
      run('INSERT INTO groups(id,user_id,name,modified_date) VALUES(?,?,?,?)', group.id, owner, group.name, group.modified_date);
      for (const p of group.targets) {
        if (!targets.has(p.email)) targets.set(p.email, insert('INSERT INTO targets(first_name,last_name,email,position) VALUES(?,?,?,?)', p.first_name, p.last_name, p.email, p.position));
        run('INSERT INTO group_targets(group_id,target_id) VALUES(?,?)', group.id, targets.get(p.email));
      }
    }
    for (const { header: c, results } of data.campaigns) {
      const launch = Date.parse(c.launch_date) / 1000;
      run('INSERT INTO campaigns(id,user_id,name,created_date,completed_date,template_id,page_id,status,url,smtp_id,launch_date,send_by_date) VALUES(?,?,?,?,?,?,?,?,?,?,?,?)', c.id, owner, c.name, iso(launch - DAY), c.status === 'Completed' ? iso(launch + DAY) : ZERO, c.template.id, c.page.id, c.status, c.url, c.smtp.id, c.launch_date, c.send_by_date);
      run('INSERT INTO ruphish_campaign_policy(campaign_id,redirect_url) VALUES(?,?)', c.id, c.policy.redirect_url);
      run('INSERT INTO ruphish_campaign_domains(campaign_id,domain_id) VALUES(?,?)', c.id, c.domain_id);
      for (const [ordinal, group] of data.groups.slice(0, 4).entries()) run('INSERT INTO ruphish_campaign_groups(campaign_id,ordinal,group_id) VALUES(?,?,?)', c.id, ordinal, group.id);
      for (const r of results) {
        const status = r.state === 'failed' ? 'Error' : r.state === 'ready' ? 'Sending' : r.events.filter(e => e.kind !== 'Email Reported').at(-1)?.kind ?? 'Email Sent';
        run('INSERT INTO results(id,campaign_id,user_id,r_id,email,first_name,last_name,status,ip,latitude,longitude,position,send_date,reported,modified_date) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)', r.id, c.id, owner, r.rid, r.email, r.first_name, r.last_name, status, '', 0, 0, r.position, r.send_date, Number(r.events.some(e => e.kind === 'Email Reported')), r.modified_date);
        run('INSERT INTO ruphish_delivery(result_id,campaign_id,state,due,generation,code,smtp_code,stage) VALUES(?,?,?,?,?,?,?,?)', r.id, c.id, r.state, r.due, r.attempts.length ? 1 : 0, r.state === 'failed' ? 'smtp_permanent_rejection' : '', r.state === 'failed' ? 550 : null, r.state === 'failed' ? 'Envelope' : '');
        for (const a of r.attempts) run('INSERT INTO ruphish_delivery_attempt(result_id,generation,started,finished,state,stage,smtp_code,message_id) VALUES(?,?,?,?,?,?,?,?)', r.id, a.generation, a.started, a.finished, a.state, a.stage, a.smtp_code, a.message_id);
        const kind = { 'Email Sent': 'email_sent', 'Email Opened': 'email_opened', 'Clicked Link': 'link_clicked', 'Submitted Data': 'form_submitted', 'Email Reported': 'email_reported' };
        for (const e of r.events) run('INSERT INTO events(id,campaign_id,email,time,kind,metadata,recipient_id,source) VALUES(?,?,?,?,?,?,?,?)', e.id, c.id, r.email, e.time, kind[e.kind], JSON.stringify(e.password_present === null ? {} : { password_present: e.password_present }), r.id, e.source);
      }
    }
    for (const draft of data.drafts) run('INSERT INTO ruphish_campaign_drafts(id,owner_id,revision,name,configuration,schema_version,step,created,modified) VALUES(?,?,?,?,?,1,?,?,?)', draft.id, owner, draft.revision, draft.configuration.name, JSON.stringify(draft.configuration), draft.step, Date.parse(draft.created) / 1000, Date.parse(draft.modified) / 1000);
    // Insert integrations after events, disabled, so seeding never queues outbound deliveries.
    const sampleEvent = data.campaigns[0].results[0].events[0].id;
    for (const w of data.webhooks) {
      run('INSERT INTO webhooks(id,name,url,secret,is_active) VALUES(?,?,?,?,0)', w.id, w.name, w.url, randomBytes(32).toString('hex'));
      run('INSERT INTO ruphish_webhook_state(webhook_id,revision) VALUES(?,?)', w.id, w.revision);
      const job = insert("INSERT INTO ruphish_webhook_jobs(webhook_id,revision,event_id,state,attempt,code) VALUES(?,?,?,'done',1,?)", w.id, w.revision, sampleEvent, w.history[0].code);
      for (const h of w.history) run('INSERT INTO ruphish_webhook_attempts(job_id,attempt,started,finished,status,code) VALUES(?,?,?,?,?,?)', job, h.attempt, h.started, h.finished, h.status, h.code);
    }
    for (const m of data['reporting-mailboxes']) {
      run('INSERT INTO ruphish_mailboxes(id,owner_id,name,enabled,host,port,username,password,tls,folder,restrict_domain,delete_reported,frequency,last_check,last_success,code) VALUES(?,?,?,0,?,?,?,?,?,?,?,?,?,?,?,?)', m.id, owner, m.name, m.host, m.port, m.username, randomBytes(32).toString('hex'), Number(m.tls), m.folder, m.restrict_domain, Number(m.delete_reported), m.frequency, m.last_check, m.last_success, m.code);
      for (const h of m.history) run('INSERT INTO ruphish_mailbox_runs(mailbox_id,started,finished,code,matched,rejected) VALUES(?,?,?,?,?,?)', m.id, h.started, h.finished, h.code, h.matched, h.rejected);
    }
    // Migrations ran before these resources existed. Apply the same installation
    // features as the browser demo; individual outbound integrations stay inactive.
    run('UPDATE ruphish_settings SET verification_enabled=?,reporting_mailboxes_enabled=?,webhooks_enabled=?,revision=revision+1,updated_at=?,updated_by=? WHERE id=1', Number(features.verification_enabled), Number(features.reporting_mailboxes_enabled), Number(features.webhooks_enabled), now, owner);
    db.exec('COMMIT');
  } catch (error) {
    db.exec('ROLLBACK');
    throw error;
  }
}
