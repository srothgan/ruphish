import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { select, smokeFilters, ordinaryAdapterTargets } from './select.mjs';

test('documentation checks the website without starting product builds or databases', () => {
  const result = select(['docs/startup.md', 'AGENTS.md']);
  assert.equal(result.native, false);
  assert.equal(result.assets, false);
  assert.equal(result.database, false);
  assert.equal(result.website, true);
});

test('website dependencies and assets remain independent of Rust and databases', () => {
  for (const path of ['website/package-lock.json', 'assets/dashboard.png', 'website/astro.config.mjs', 'docs/api/index.md']) {
    const result = select([path]);
    assert.equal(result.website, true, path);
    assert.equal(result.native || result.ui || result.assets || result.database, false, path);
  }
  assert.equal(select(['crates/ruphish-ui/src/overview.rs']).website, false);
});
test('UI work checks browser consumers without starting databases', () => {
  const result = select(['crates/ruphish-ui/src/account.rs']);
  assert.equal(result.ui, true);
  assert.equal(result.smoke, true);
  assert.equal(result.database, false);
});
test('shared contracts cover native and browser consumers', () => {
  const result = select(['crates/ruphish-contracts/src/legacy.rs']);
  assert.equal(result.native, true);
  assert.equal(result.ui, true);
  assert.equal(result.process, true);
  assert.equal(select(['crates/ruphish-adapters/src/landing-collector.js']).ui, true);
});
test('provider changes select one engine; schema changes select its supported versions', () => {
  const query = select(['crates/ruphish-adapters/src/mysql/resources.rs']);
  assert.equal(query.mysql, true);
  assert.equal(query.postgres, false);
  assert.equal(query.database_matrix.length, 1);
  assert.deepEqual(query.process_filters, ['resource_process::']);
  const migration = select(['migrations/mysql/0002.sql']);
  assert.equal(migration.postgres, false);
  assert.equal(migration.database_matrix.length, 2);
  const fixture = select(['tests/fixtures/mysql-legacy/0001.sql']);
  assert.equal(fixture.postgres, false);
  assert.equal(fixture.database_matrix.length, 2);
});
test('shared storage and domain rules exercise both server providers', () => {
  for (const path of ['crates/ruphish-adapters/src/sqlite/campaigns.rs', 'crates/ruphish-application/src/auth.rs', 'crates/ruphish-domain/src/account.rs']) {
    const result = select([path]);
    assert.equal(result.mysql && result.postgres, true, path);
  }
});
test('deleted, renamed, unknown and build inputs cannot bypass acceptance', () => {
  for (const path of ['Cargo.lock', '.github/workflows/validation.yml', 'new-build-input.txt', '.gitattributes']) {
    const result = select([path]);
    assert.equal(result.full, true, path);
    assert.equal(result.database_matrix.length, 4);
  }
  assert.equal(select(['tests/support/new-network.rs']).process_filters[0], '');
});
test('nightly, release branches and manual auto use complete acceptance', () => {
  for (const [event, branch] of [['schedule', 'main'], ['pull_request', 'release'], ['workflow_dispatch', '']]) {
    assert.equal(select([], 'auto', event, branch).full, true);
  }
});
test('manual profiles are bounded and invalid choices fail', () => {
  assert.equal(select([], 'ui', 'workflow_dispatch').database, false);
  assert.equal(select([], 'databases', 'workflow_dispatch').database_matrix.length, 4);
  assert.throws(() => select([], 'typo'));
});
test('fixture changes include every prerequisite of selected database jobs', () => {
  const result = select(['tests/acceptance/prepare.sh']);
  assert.equal(result.native && result.assets && result.database && result.public, true);
  assert.equal(result.database_matrix.length, 4);
});
test('crate names do not accidentally narrow runtime coverage', () => {
  assert.deepEqual(select(['crates/ruphish-domain/src/datetime.rs']).process_filters, ['']);
  assert.equal(select(['crates/ruphish-adapters/src/database.rs']).database_matrix.length, 2);
  assert.equal(select(['crates/ruphish-adapters/src/mysql.rs']).postgres, false);
  assert.equal(select(['security-audit/reproduce.mjs']).full, true);
});
test('manual provider contracts do not require a browser build', () => {
  assert.equal(select([], 'databases', 'workflow_dispatch').assets, false);
});
test('smoke selection names existing independent executable scenarios', () => {
  const root = new URL('../../tests/', import.meta.url);
  for (const filter of smokeFilters) {
    const parts = filter.split('::');
    const name = parts.pop();
    const file = parts.length ? parts[0] + '.rs' : 'startup_process.rs';
    assert.ok(readFileSync(new URL(file, root), 'utf8').includes('fn ' + name + '('), filter);
  }
});

test('upgrade implementations trigger historical acceptance on both provider versions', () => {
  const result = select(['crates/ruphish-adapters/src/mysql/upgrade.rs']);
  assert.equal(result.migration, true);
  assert.deepEqual(result.database_matrix.map(item => item.engine + item.version), ['mysql8.4', 'mysql9.7']);
});

test('changed provider executable scenarios and fault proxies actually execute', () => {
  for (const [path, engines] of [
    ['tests/mysql_process.rs', ['mysql']],
    ['tests/postgres_process.rs', ['postgres']],
    ['tests/server_database_process.rs', ['mysql', 'postgres']],
    ['crates/ruphish-adapters/tests/support/postgres_proxy.rs', ['postgres']],
    ['crates/ruphish-adapters/tests/support/mysql_proxy.rs', ['mysql']],
  ]) {
    const result = select([path]);
    assert.equal(result.provider_process, true, path);
    assert.equal(result.assets && result.native, true, path);
    assert.deepEqual(result.database_matrix.map(item => item.engine), engines, path);
  }
});

test('unclassified integration targets execute through full acceptance', () => {
  for (const crate of ['ruphish-adapters', 'ruphish-http', 'ruphish-application', 'ruphish-contracts', 'ruphish-ui']) {
    assert.equal(select([`crates/${crate}/tests/new_contract.rs`]).full, true, crate);
  }
  const workflow = readFileSync(new URL('../workflows/validation.yml', import.meta.url), 'utf8');
  for (const name of ordinaryAdapterTargets) assert.ok(workflow.includes('--test ' + name + ' '), name);
  assert.ok(workflow.includes("cargo test -p ruphish-ui --test '*' --target wasm32-unknown-unknown"));
});

test('provider regression edits include their migration cases', () => {
  for (const file of ['database_providers.rs', 'support/provider_regressions.rs']) {
    const result = select(['crates/ruphish-adapters/tests/' + file]);
    assert.equal(result.migration, true);
    assert.equal(result.database_matrix.length, 4);
  }
});

test('native selection includes platform checks and full acceptance covers provider executables', () => {
  assert.equal(select(['crates/ruphish-http/src/auth.rs']).native, true);
  const full = select([], 'full');
  assert.equal(full.platform_full && full.provider_process, true);
  const workflow = readFileSync(new URL('../workflows/validation.yml', import.meta.url), 'utf8');
  for (const runner of ['windows-2025', 'macos-15', 'macos-15-intel']) assert.ok(workflow.includes(runner), runner);
  const release = readFileSync(new URL('../workflows/release.yml', import.meta.url), 'utf8');
  for (const runner of ['ubuntu-24.04', 'windows-2025', 'macos-15', 'macos-15-intel']) assert.ok(release.includes(runner), runner);
});
