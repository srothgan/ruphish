// Run after starting the canonical fixture's mysql and postgres services.
import test from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { randomBytes } from 'node:crypto';
import { fileURLToPath } from 'node:url';
import { resolve } from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { establishContainerAccount } from './container-account.mjs';

const compose = ['compose', '-f', fileURLToPath(new URL('./compose.yaml', import.meta.url))];
const data = resolve(
  process.env.RUPHISH_ACCEPTANCE_DATA_DIRECTORY ??
    fileURLToPath(new URL('./data/', import.meta.url)),
);
const image = process.env.RUPHISH_APP_IMAGE ?? 'ruphish:local';

function docker(args, allowFailure = false) {
  const result = spawnSync('docker', args, { encoding: 'utf8', timeout: 120_000 });
  assert.ifError(result.error);
  if (!allowFailure) {
    assert.equal(result.status, 0, result.stderr);
  }
  return result;
}

for (const provider of ['mysql', 'postgres']) {
  test(
    `${provider} image configuration through environment uses verified TLS and survives replacement`,
    { timeout: 90_000 },
    async (t) => {
      const id = randomBytes(8).toString('hex');
      const database = `ruphish_env_${id}`;
      const name = `ruphish-db-image-${id}`;
      const password = randomBytes(20).toString('hex');
      const sql = (statement) =>
        provider === 'mysql'
          ? docker([
              ...compose,
              'exec',
              '-T',
              '-e',
              'MYSQL_PWD=ruphish-mysql-fixture-only',
              'mysql',
              'mysql',
              '-u',
              'root',
              '--batch',
              '--skip-column-names',
              '-e',
              statement,
            ]).stdout.trim()
          : docker([
              ...compose,
              'exec',
              '-T',
              'postgres',
              'psql',
              '-U',
              'postgres',
              '-d',
              'postgres',
              '-v',
              'ON_ERROR_STOP=1',
              '-Atc',
              statement,
            ]).stdout.trim();
      if (provider === 'mysql') {
        sql(
          `CREATE DATABASE ${database} CHARACTER SET utf8mb4 COLLATE utf8mb4_0900_bin; CREATE USER '${database}'@'%' IDENTIFIED BY '${password}'; GRANT ALL ON ${database}.* TO '${database}'@'%';`,
        );
      } else {
        sql(`CREATE ROLE ${database} LOGIN PASSWORD '${password}';`);
        sql(`CREATE DATABASE ${database} OWNER ${database};`);
      }
      t.after(() => {
        docker(['rm', '-f', name], true);
        sql(`DROP DATABASE ${database};`);
        sql(provider === 'mysql' ? `DROP USER '${database}'@'%';` : `DROP ROLE ${database};`);
      });
      const prefix = provider.toUpperCase();
      const start = () =>
        docker([
          'run',
          '-d',
          '--name',
          name,
          '--network',
          'ruphish-acceptance_lab',
          '--read-only',
          '--tmpfs',
          '/tmp:rw,noexec,nosuid,size=32m',
          '--mount',
          `type=bind,source=${data}/${provider},target=/certificates,readonly`,
          ...[
            `DB_NAME=${provider}`,
            `${prefix}_HOST=${provider}`,
            `${prefix}_DATABASE=${database}`,
            `${prefix}_USERNAME=${database}`,
            `${prefix}_PASSWORD=${password}`,
            `${prefix}_TLS=verify_identity`,
            `${prefix}_CA_PATH=/certificates/ca.pem`,
            'GOPHISH_INITIAL_ADMIN_PASSWORD=synthetic-provider-password',
            'LOGGING_LEVEL=warn',
          ].flatMap((value) => ['-e', value]),
          image,
          '--mode',
          'admin',
          '--disable-mailer',
        ]).stdout.trim();
      const waitReady = async () => {
        const deadline = Date.now() + 30_000;
        while (Date.now() < deadline) {
          if (docker(['exec', name, 'ruphish-healthcheck'], true).status === 0) {
            return;
          }
          if (docker(['inspect', '--format={{.State.Running}}', name]).stdout.trim() !== 'true') {
            assert.fail(docker(['logs', '--tail', '20', name]).stderr);
          }
          await delay(250);
        }
        assert.fail(`${provider} image did not become ready`);
      };
      const first = start();
      await waitReady();
      establishContainerAccount(name, 'synthetic-provider-password');
      const users = () =>
        provider === 'mysql'
          ? sql(`SELECT id, username, hash FROM ${database}.users ORDER BY id;`)
          : docker([
              ...compose,
              'exec',
              '-T',
              'postgres',
              'psql',
              '-U',
              'postgres',
              '-d',
              database,
              '-Atc',
              'SELECT id, username, hash FROM ruphish.users ORDER BY id;',
            ]).stdout.trim();
      const before = users();
      assert.match(before, /admin/);
      docker(['stop', '--time', '30', name]);
      assert.equal(docker(['inspect', '--format={{.State.ExitCode}}', name]).stdout.trim(), '0');
      docker(['rm', name]);
      assert.notEqual(start(), first);
      await waitReady();
      assert.equal(
        users(),
        before,
        'Existing accounts and password hashes must survive replacement',
      );
      const config = JSON.parse(docker(['exec', name, 'cat', '/etc/ruphish/config.json']).stdout);
      assert.equal(config.db_name, 'sqlite3', 'Environment selection must not rewrite JSON');
    },
  );
}
