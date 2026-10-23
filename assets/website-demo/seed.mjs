// One initial dataset for a newly initialized, disposable screenshot database.
import { randomBytes } from 'node:crypto';

const day = 86400;
const iso = (seconds) => new Date(seconds * 1000).toISOString();
const zero = '0001-01-01T00:00:00Z';
const names = ['Alex', 'Sam', 'Robin', 'Morgan', 'Jamie', 'Taylor', 'Casey', 'Avery'];
const surnames = ['Rivera', 'Lee', 'Meyer', 'Brooks', 'Patel', 'Winter', 'Chen', 'Berg'];
const campaigns = [
  { name: 'Quarterly awareness baseline', ago: 54, size: 400, opened: 260, clicked: 72, submitted: 28, reported: 44, complete: true, failed: 0 },
  { name: 'Benefits enrollment', ago: 21, size: 617, opened: 320, clicked: 48, submitted: 12, reported: 140, complete: true, failed: 0 },
  { name: 'Delivery notification', ago: 2, size: 320, opened: 170, clicked: 22, submitted: 6, reported: 98, complete: false, failed: 12 },
];

export function seed(db, now = Math.floor(Date.now() / 1000)) {
  db.exec('BEGIN IMMEDIATE');
  try {
    const owner = db.prepare("SELECT id FROM users WHERE username='admin'").get()?.id;
    if (!owner) throw new Error('Initialize the application administrator before seeding.');
    for (const table of ['campaigns', 'results', 'events', 'templates', 'pages', 'groups', 'targets', 'smtp', 'ruphish_domains']) {
      if (db.prepare(`SELECT count(*) AS total FROM ${table}`).get().total !== 0) {
        throw new Error('The recorder requires a fresh database; existing data will not be changed.');
      }
    }
    const insert = (sql, ...values) => Number(db.prepare(sql).run(...values).lastInsertRowid);
    const date = iso(now);
    const origin = 'https://awareness.example.test';
    // Historical demonstration only: no claimed verification and no domain worker requests.
    const domain = insert('INSERT INTO ruphish_domains(owner_id,name,origin,hostname,enabled,challenge,challenge_expires) VALUES(?,?,?,?,?,?,?)', owner, 'Awareness exercises', origin, 'awareness.example.test', 0, randomBytes(32).toString('hex'), 0);
    db.prepare('INSERT INTO ruphish_domain_claims(hostname,owner_id) VALUES(?,?)').run('awareness.example.test', owner);
    const profile = insert('INSERT INTO smtp(user_id,name,interface_type,host,username,password,from_address,modified_date,ignore_cert_errors) VALUES(?,?,?,?,?,?,?,?,?)', owner, 'Awareness team', 'SMTP', 'smtp.example.test:1025', '', '', 'Awareness team <awareness@example.test>', date, 0);
    const pages = ['Awareness exercise', 'Document sharing exercise'].map((name) => insert('INSERT INTO pages(user_id,name,html,modified_date,capture_credentials,capture_passwords,redirect_url) VALUES(?,?,?,?,?,?,?)', owner, name, '<!doctype html><html lang="en"><title>Awareness exercise</title><h1>Synthetic awareness exercise</h1><p>No sign-in is needed.</p></html>', date, 0, 0, ''));
    const templates = ['Benefits enrollment', 'Shared document', 'Delivery notification', 'Team onboarding'].map((name) => insert('INSERT INTO templates(user_id,name,subject,text,html,modified_date) VALUES(?,?,?,?,?,?)', owner, name, `${name} - local exercise`, 'Hello {{.FirstName}}, this is a local demonstration. {{.URL}}', '<p>Hello {{.FirstName}},</p><p>This is a local demonstration. <a href="{{.URL}}">View the exercise</a>.</p>{{.Tracker}}', date));
    const departments = ['Operations', 'Engineering', 'Finance', 'People & culture'];
    const groups = departments.map((name) => insert('INSERT INTO groups(user_id,name,modified_date) VALUES(?,?,?)', owner, name, date));
    const people = Array.from({ length: Math.max(...campaigns.map((campaign) => campaign.size)) }, (_, n) => {
      const first = names[n % names.length];
      const last = surnames[Math.floor(n / names.length) % surnames.length];
      const email = `person${String(n + 1).padStart(3, '0')}@example.test`;
      const position = departments[n % departments.length];
      const id = insert('INSERT INTO targets(first_name,last_name,email,position) VALUES(?,?,?,?)', first, last, email, position);
      db.prepare('INSERT INTO group_targets(group_id,target_id) VALUES(?,?)').run(groups[n % groups.length], id);
      return { first, last, email, position };
    });
    for (const [index, campaign] of campaigns.entries()) {
      const launch = now - campaign.ago * day;
      const id = insert('INSERT INTO campaigns(user_id,name,created_date,completed_date,template_id,page_id,status,url,smtp_id,launch_date,send_by_date) VALUES(?,?,?,?,?,?,?,?,?,?,?)', owner, campaign.name, iso(launch - day), campaign.complete ? iso(launch + day) : zero, templates[index % templates.length], pages[index % pages.length], campaign.complete ? 'Completed' : 'In progress', origin, profile, iso(launch), zero);
      db.prepare('INSERT INTO ruphish_campaign_policy(campaign_id,capture_credentials,capture_passwords,redirect_url) VALUES(?,?,?,?)').run(id, 0, 0, '');
      db.prepare('INSERT INTO ruphish_campaign_domains(campaign_id,domain_id) VALUES(?,?)').run(id, domain);
      for (const [ordinal, group] of groups.entries()) db.prepare('INSERT INTO ruphish_campaign_groups(campaign_id,ordinal,group_id) VALUES(?,?,?)').run(id, ordinal, group);
      const event = (email, time, kind, details = '{}') => db.prepare('INSERT INTO events(campaign_id,email,time,message,details) VALUES(?,?,?,?,?)').run(id, email, iso(time), kind, details);
      event('', launch - day, 'Campaign Created');
      for (let n = 0; n < campaign.size; n++) {
        const person = people[n];
        const accepted = launch + n * 7;
        const reported = n >= campaign.size - campaign.reported;
        const status = n < campaign.submitted ? 'Submitted Data' : n < campaign.clicked ? 'Clicked Link' : n < campaign.opened ? 'Email Opened' : 'Email Sent';
        const result = insert('INSERT INTO results(campaign_id,user_id,r_id,email,first_name,last_name,status,ip,latitude,longitude,position,send_date,reported,modified_date) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?)', id, owner, randomBytes(16).toString('hex'), person.email, person.first, person.last, status, '', 0, 0, person.position, iso(accepted), Number(reported), iso(accepted + 7200));
        db.prepare('INSERT INTO ruphish_delivery(result_id,campaign_id,state,due,code) VALUES(?,?,?,?,?)').run(result, id, 'accepted', accepted, '');
        event(person.email, accepted, 'Email Sent');
        if (n < campaign.opened) event(person.email, accepted + 900, 'Email Opened');
        if (n < campaign.clicked) event(person.email, accepted + 1800, 'Clicked Link');
        if (n < campaign.submitted) event(person.email, accepted + 2100, 'Submitted Data', '{"password_present":true}');
        if (reported) event(person.email, accepted + 3600, 'Email Reported');
      }
      for (let n = 1; n <= campaign.failed; n++) {
        const email = `undeliverable${String(n).padStart(2, '0')}@example.test`;
        const due = launch + 7200;
        const result = insert('INSERT INTO results(campaign_id,user_id,r_id,email,first_name,last_name,status,ip,latitude,longitude,position,send_date,reported,modified_date) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?)', id, owner, randomBytes(16).toString('hex'), email, 'Demo', `Recipient ${n}`, 'Error', '', 0, 0, 'Operations', iso(launch), 0, iso(due));
        db.prepare('INSERT INTO ruphish_delivery(result_id,campaign_id,state,due,code,smtp_code,stage) VALUES(?,?,?,?,?,?,?)').run(result, id, 'failed', due, 'smtp_permanent_rejection', 550, 'Envelope');
        event(email, due, 'Error Sending Email', JSON.stringify({ code: 'smtp_permanent_rejection', error: 'Synthetic SMTP 550: recipient mailbox unavailable' }));
      }
      if (campaign.complete) event('', launch + day, 'Campaign Completed');
    }
    db.exec('COMMIT');
  } catch (error) {
    db.exec('ROLLBACK');
    throw error;
  }
}
