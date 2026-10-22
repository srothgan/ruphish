// Select checks only; Cargo/Node/Docker commands remain visible in the workflow.
import { execFileSync } from 'node:child_process';
import { appendFileSync, readFileSync } from 'node:fs';
import { pathToFileURL } from 'node:url';

export const smokeFilters = [
  'actual_http_servers_serve_built_assets_isolate_public_routes_and_shutdown_on_real_ctrl_c',
  'auth_process::browser_resource_create_read_edit_and_reload_routes_preserve_saved_data',
  'campaign_process::executable_launch_delivers_personalized_mail_once_and_persists_acceptance_across_restart',
  'public_process::verified_domain_to_campaign_to_private_events_survives_restart_and_rejects_bypasses',
];

const features = [
  [/auth|accounts?|sessions?|password|users/, 'auth_process::'],
  [/campaign|delivery|outbound|smtp/, 'campaign_process::'],
  [/resources?|imports?|templates?|landing|editor/, 'resource_process::'],
  [/domains?|interactions|public|landing/, 'public_process::'],
  [/proxy/, 'campaign_process::proxy_process::'],
  [/integrations?|reporting|mailbox|webhook/, 'reporting_process::'],
  [/overview/, 'overview_process::'],
];

// Targets explicitly executed by the ordinary native baseline. Unknown targets
// get full acceptance, so adding a test cannot silently compile without running.
export const ordinaryAdapterTargets = new Set(['accounts', 'campaign_storage', 'campaign_transport', 'database_facade', 'legacy_auth', 'legacy_templates', 'overview', 'public_domains', 'reporting', 'resource_formats', 'session_store', 'startup_database', 'tls_files']);

export function select(paths, profile = 'auto', event = 'pull_request', branch = 'main') {
  if (!['auto', 'native', 'ui', 'databases', 'full'].includes(profile)) throw new Error('Unknown validation profile');
  const selected = { native: false, ui: false, dependencies: false, smoke: false, mysql: false, postgres: false, migration: false, provider_process: false, full: false, public: false, platform_full: false, rust_format: false };
  const filters = new Set();
  const all = () => {
    for (const key of Object.keys(selected)) selected[key] = true;
    filters.clear();
  };
  const feature = path => {
    const relative = path.replace(/^crates\/[^/]+\//, '');
    const matches = features.filter(([pattern]) => pattern.test(relative));
    if (matches.length) for (const [, filter] of matches) filters.add(filter);
    else filters.add(''); // An unclassified runtime change gets the full process suite.
  };
  if (event === 'schedule' || profile === 'full' || branch === 'prod' || branch.startsWith('release/')) all();
  else if (profile === 'native') Object.assign(selected, { native: true, smoke: true, rust_format: true });
  else if (profile === 'ui') Object.assign(selected, { ui: true, smoke: true, rust_format: true });
  else if (profile === 'databases') Object.assign(selected, { native: true, mysql: true, postgres: true, migration: true, rust_format: true });
  else if (event === 'workflow_dispatch') all(); // Manual auto has no PR diff: never guess from HEAD~1.
  else for (const original of paths) {
    const path = original.replaceAll('\\', '/');
    if (path.startsWith('.github/') || /(^|\/)(Cargo\.(toml|lock)|rust-toolchain\.toml)$/.test(path) || path.startsWith('.cargo/') || path === 'deny.toml') {
      all();
      break;
    }
    if (/\.md$/.test(path) || /^(LICENSE|NOTICE|\.gitignore|\.editorconfig)$/.test(path)) continue;
    if (path === 'rustfmt.toml') { selected.rust_format = true; continue; }
    if (path === 'leptosfmt.toml') { selected.ui = true; continue; }
    const integration = /^crates\/([^/]+)\/tests\/([^/]+)\.rs$/.exec(path);
    if (integration && !(
      integration[1] === 'ruphish-domain' ||
      (integration[1] === 'ruphish-http' && integration[2] === 'transport_compatibility') ||
      (integration[1] === 'ruphish-ui' && integration[2] === 'browser_requests') ||
      (integration[1] === 'ruphish-adapters' && (ordinaryAdapterTargets.has(integration[2]) || ['database_providers', 'mysql_preflight', 'sqlite_upgrade'].includes(integration[2])))
    )) { all(); break; }
    if (path === 'Trunk.toml' || path.startsWith('crates/ruphish-ui/')) {
      selected.ui = selected.smoke = true;
      if (/\.rs$/.test(path)) selected.rust_format = true;
      if (/package(-lock)?\.json$/.test(path)) selected.dependencies = true;
      continue;
    }
    if (path.startsWith('tests/acceptance/')) {
      selected.native = selected.public = selected.smoke = true;
      selected.mysql = selected.postgres = selected.migration = true;
      continue;
    }
    if (/tests\/(mysql_process|postgres_process|server_database_process)\.rs$/.test(path) || /\/tests\/support\/(mysql|postgres)_proxy\.rs$/.test(path)) {
      selected.native = selected.smoke = selected.rust_format = selected.provider_process = true;
      if (!path.includes('postgres')) selected.mysql = true;
      if (!path.includes('mysql')) selected.postgres = true;
      continue;
    }
    if (/database_providers|provider_regressions/.test(path)) {
      selected.native = selected.smoke = selected.rust_format = selected.migration = selected.mysql = selected.postgres = true;
      continue;
    }
    if (path.startsWith('migrations/') || path.startsWith('tests/fixtures/') || /schema|migration|upgrade|mysql_preflight/.test(path)) {
      selected.native = selected.rust_format = selected.smoke = selected.migration = true;
      if (!/(?:^|\/)postgres(?:\/|-|_|\.)/.test(path)) selected.mysql = true;
      if (!/(?:^|\/)mysql(?:\/|-|_|\.)/.test(path)) selected.postgres = true;
      feature(path);
      continue;
    }
    if (path.startsWith('crates/') || path.startsWith('tests/')) {
      selected.native = selected.rust_format = selected.smoke = true;
      if (path === 'crates/ruphish-adapters/src/landing-collector.js') selected.ui = true;
      if (/crates\/ruphish-(domain|contracts)\//.test(path)) selected.ui = true;
      if (/\/mysql(?:\/|\.rs$)/.test(path)) selected.mysql = true;
      else if (/\/postgres(?:\/|\.rs$)/.test(path)) selected.postgres = true;
      else if (/crates\/ruphish-(application|domain)\/|crates\/ruphish-adapters\/src\/|database_providers|provider_regressions|server_database_process/.test(path)) selected.mysql = selected.postgres = true;
      if (/tls|transport|socket|startup|config|server\.rs|\.gitattributes/.test(path)) selected.platform_full = true;
      feature(path);
      continue;
    }
    all(); // Unknown non-documentation inputs cannot silently bypass checks.
    break;
  }
  const processFilters = selected.full || filters.has('') ? [''] : [...filters].sort();
  const db = [];
  if (selected.mysql) {
    db.push({ engine: 'mysql', version: '8.4', image: 'mysql@sha256:0f51e63e537a1f176423fa03ef559aabe6fa4372a53160a44bc7e75da7d15364' });
    if (selected.full || selected.migration) db.push({ engine: 'mysql', version: '9.7', image: 'mysql@sha256:e2fa358a74ac81148e7e27d9433e4c0ab62133de8ea4428b4f39a51a4ba7d07f' });
  }
  if (selected.postgres) {
    db.push({ engine: 'postgres', version: '17', image: 'postgres@sha256:3645570cccdfa447589da9f57dd740faa29b30938e861289a5574b6ca6b03826' });
    if (selected.full || selected.migration) db.push({ engine: 'postgres', version: '18', image: 'postgres@sha256:afc7e2d441324c0388fa80c3d24f733b4194a4eb7f47dd8ee2b08eb1a24a647c' });
  }
  return { ...selected, assets: selected.ui || selected.smoke || selected.public || selected.platform_full, process: processFilters.length > 0, process_filters: processFilters, database: db.length > 0, database_matrix: db };
}

function main() {
  const event = JSON.parse(readFileSync(process.env.GITHUB_EVENT_PATH, 'utf8'));
  let paths = [];
  if (process.env.GITHUB_EVENT_NAME === 'pull_request') {
    const base = event.pull_request.base.sha;
    const head = event.pull_request.head.sha;
    if (![base, head].every(value => /^[a-f0-9]{40}$/.test(value))) throw new Error('Invalid diff commit');
    paths = execFileSync('git', ['diff', '--name-only', '-z', '--no-renames', base + '...' + head], { encoding: 'utf8', maxBuffer: 16 * 1024 * 1024 }).split('\0').filter(Boolean);
  }
  const profile = process.env.VALIDATION_PROFILE ?? event.inputs?.profile ?? 'auto';
  const result = select(paths, profile, process.env.GITHUB_EVENT_NAME, event.pull_request?.base.ref ?? '');
  for (const [key, value] of Object.entries(result)) appendFileSync(process.env.GITHUB_OUTPUT, key + '=' + JSON.stringify(value) + '\n');
  appendFileSync(process.env.GITHUB_STEP_SUMMARY, '## Selected validation\n\nProfile: ' + profile + '\n\n```json\n' + JSON.stringify(result, null, 2) + '\n```\n');
}
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) main();
