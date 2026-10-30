import assert from 'node:assert/strict';
import { spawn, spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { once } from 'node:events';
import {
  chmodSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  renameSync,
  rmSync,
  statSync,
  writeFileSync,
} from 'node:fs';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { gunzipSync, gzipSync } from 'node:zlib';
import test from 'node:test';
import type { TestContext } from 'node:test';
import { releaseResponse } from '../lib/release-selection.ts';

const windows = process.platform === 'win32';
const publicDir = fileURLToPath(new URL('../public/', import.meta.url));
const shell = '/bin/sh';
const powershell = process.env.RUPHISH_TEST_POWERSHELL || 'powershell.exe';
const posix = (path: string) =>
  windows
    ? path
        .replaceAll('\\', '/')
        .replace(/^([A-Za-z]):/, (_, drive: string) => `/${drive.toLowerCase()}`)
    : path;
const hash = (bytes: Buffer) => createHash('sha256').update(bytes).digest('hex');

function run(
  command: string,
  args: string[],
  options: { env?: NodeJS.ProcessEnv; input?: string; cwd?: string } = {},
) {
  return new Promise<{ code: number | null; output: string }>((resolveResult, reject) => {
    const child = spawn(command, args, {
      env: options.env,
      cwd: options.cwd,
      windowsHide: true,
      stdio: ['pipe', 'pipe', 'pipe'],
    });
    let output = '';
    child.stdout.on('data', (data) => {
      output += data;
    });
    child.stderr.on('data', (data) => {
      output += data;
    });
    const timeout = setTimeout(() => {
      child.kill();
      reject(new Error(`Installer timed out: ${output}`));
    }, 45_000);
    child.on('error', (error) => {
      clearTimeout(timeout);
      reject(error);
    });
    child.on('close', (code) => {
      clearTimeout(timeout);
      resolveResult({ code, output });
    });
    child.stdin.end(options.input);
  });
}

async function fixture(t: TestContext, kind: 'sh' | 'ps1') {
  const directory = mkdtempSync(join(tmpdir(), 'ruphish install ü-'));
  const home = join(directory, 'home');
  mkdirSync(home);
  const root = join(home, 'program files');
  const data = join(home, 'application data');
  mkdirSync(data);
  writeFileSync(join(data, 'keep.db'), 'existing application data');
  const requests: string[] = [];
  const packages = new Map<string, { archive: Buffer; sums: string }>();
  let selected = '0.1.0';
  let error = '';
  let versionStatus = 200;
  const server = createServer(async (req, res) => {
    const url = new URL(req.url!, 'http://fixture');
    requests.push(url.pathname + url.search);
    if (url.pathname === '/api/releases') {
      if (versionStatus !== 200) {
        res.writeHead(versionStatus);
        res.end('Release discovery unavailable');
        return;
      }
      const response = await releaseResponse(new Request(`${origin}${req.url}`), (input, init) =>
        fetch(String(input).replace('https://api.github.com', `${origin}/github`), init),
      );
      res.writeHead(response.status, Object.fromEntries(response.headers));
      res.end(await response.text());
      return;
    }
    if (url.pathname === '/github/repos/srothgan/ruphish/releases') {
      const releases = ['0.1.0', selected].map((version) => {
        const names = [
          'x86_64-pc-windows-msvc',
          'aarch64-pc-windows-msvc',
          'x86_64-unknown-linux-gnu',
          'aarch64-unknown-linux-gnu',
          'aarch64-apple-darwin',
        ].map(
          (target) =>
            `ruphish-${version}-${target}.${target.includes('windows') ? 'zip' : 'tar.gz'}`,
        );
        names.push('SHA256SUMS', 'release-manifest.json', 'native-provenance.jsonl');
        return {
          tag_name: `v${version}`,
          draft: false,
          prerelease: version.includes('-'),
          published_at: '2026-10-29T10:00:00Z',
          assets: names.map((name) => ({
            name,
            size: 123,
            state: 'uploaded',
            browser_download_url: `https://github.com/srothgan/ruphish/releases/download/v${version}/${name}`,
          })),
        };
      });
      res.end(JSON.stringify(releases));
      return;
    }
    const match = /^\/repo\/releases\/download\/v([^/]+)\/(.+)$/.exec(url.pathname);
    if (!match || !packages.has(match[1])) {
      res.writeHead(404);
      res.end('not found');
      return;
    }
    const value = packages.get(match[1])!;
    if (error === 'http') {
      res.writeHead(404);
      res.end('not found');
      return;
    }
    if (match[2] === 'SHA256SUMS') {
      res.end(
        error === 'duplicate'
          ? value.sums + value.sums
          : error === 'checksum'
            ? value.sums.replace(/^[0-9a-f]+/, '0'.repeat(64))
            : value.sums,
      );
      return;
    }
    if (error === 'truncated') {
      res.writeHead(200, { 'Content-Length': value.archive.length + 100 });
      res.write(value.archive.subarray(0, 100));
      setImmediate(() => res.destroy());
      return;
    }
    res.writeHead(200, { 'Content-Length': value.archive.length });
    res.end(value.archive);
  });
  server.listen(0, '127.0.0.1');
  await once(server, 'listening');
  const address = server.address();
  assert(address && typeof address !== 'string');
  const origin = `http://127.0.0.1:${address.port}`;
  // Private copies change ONLY production origins and the transport allowlist to loopback HTTP.
  // The shipped scripts have no origin overrides or insecure transport switches.
  const source = readFileSync(join(publicDir, `install.${kind}`), 'utf8');
  assert(source.includes('https://ruphish.com/api/releases'));
  assert(source.includes('https://github.com/srothgan/ruphish'));
  const script = join(directory, `install.${kind}`);
  writeFileSync(
    script,
    source
      .replaceAll('https://ruphish.com', origin)
      .replaceAll('https://github.com/srothgan/ruphish', `${origin}/repo`)
      .replaceAll("'=https'", "'=http'")
      .replace("$next.Scheme -ne 'https'", "$next.Scheme -ne 'http'"),
  );
  const env: NodeJS.ProcessEnv = {
    ...process.env,
    HOME: posix(home),
    CI: 'true',
    NO_COLOR: 'true',
    SHELL: '/bin/sh',
  };
  const target =
    kind === 'ps1'
      ? (process.env.PROCESSOR_ARCHITECTURE === 'ARM64' ? 'aarch64' : 'x86_64') + '-pc-windows-msvc'
      : process.platform === 'darwin'
        ? 'aarch64-apple-darwin'
        : (process.arch === 'arm64' && !windows ? 'aarch64' : 'x86_64') + '-unknown-linux-gnu';
  async function packageVersion(
    version: string,
    contents: { extra?: string; executableVersion?: string; traversal?: boolean } = {},
  ) {
    const name = `ruphish-${version}-${target}`;
    const packageDir = join(directory, name);
    mkdirSync(packageDir);
    const binaryVersion = contents.executableVersion ?? version;
    if (kind === 'sh') {
      writeFileSync(
        join(packageDir, 'ruphish'),
        `#!/bin/sh\nprintf 'Ruphish ${binaryVersion}\\n' >&2\n`,
      );
      chmodSync(join(packageDir, 'ruphish'), 0o755);
    } else {
      const compiler = join(directory, `compile-${version}.ps1`);
      const cs = `public class Fixture { public static void Main(string[] args) { System.Console.Error.WriteLine("Ruphish ${binaryVersion}"); } }`;
      writeFileSync(
        compiler,
        `\ufeff$ErrorActionPreference = 'Stop'\nAdd-Type -TypeDefinition '${cs}' -OutputAssembly '${join(packageDir, 'ruphish.exe').replaceAll("'", "''")}' -OutputType ConsoleApplication\n`,
      );
      const compiled = await run('powershell.exe', ['-NoProfile', '-File', compiler]);
      assert.equal(compiled.code, 0, compiled.output);
      assert(existsSync(join(packageDir, 'ruphish.exe')), compiled.output);
    }
    for (const file of ['LICENSE', 'THIRD-PARTY-NOTICES.md', 'THIRD-PARTY-NOTICES.txt']) {
      writeFileSync(join(packageDir, file), 'fixture notice\n');
    }
    writeFileSync(join(packageDir, 'build.json'), JSON.stringify({ version, target }));
    if (contents.extra) {
      writeFileSync(join(packageDir, contents.extra), 'must not be installed');
    }
    const archiveName = `${name}.${kind === 'ps1' ? 'zip' : 'tar.gz'}`;
    const archivePath = join(directory, archiveName);
    const packed = spawnSync(
      'tar',
      kind === 'ps1'
        ? ['-a', '-cf', archivePath, '-C', directory, name]
        : ['-czf', archivePath, '-C', directory, name],
      { encoding: 'utf8', windowsHide: true },
    );
    assert.equal(packed.status, 0, packed.stderr);
    let archive = readFileSync(archivePath);
    if (contents.traversal) {
      if (kind === 'sh') {
        const tar = gunzipSync(archive);
        let changed = false;
        for (let offset = 0; offset + 512 <= tar.length && tar[offset] !== 0;) {
          const header = tar.subarray(offset, offset + 512);
          const filename = header.subarray(0, 100).toString().replace(/\0.*$/s, '');
          const size = parseInt(
            header.subarray(124, 136).toString().replace(/\0.*$/s, '').trim() || '0',
            8,
          );
          if (filename === `${name}/ruphish`) {
            header.fill(0, 0, 100);
            header.write('../escaped', 0, 'ascii');
            header.fill(32, 148, 156);
            const sum = header.reduce((total, byte) => total + byte, 0);
            header.write(sum.toString(8).padStart(6, '0') + '\0 ', 148, 'ascii');
            changed = true;
          }
          offset += 512 + Math.ceil(size / 512) * 512;
        }
        assert(changed, 'fixture tar must contain the executable header');
        archive = gzipSync(tar);
      } else {
        // Rewrite equal-length local and central ZIP names; file data/CRC remain valid.
        const filename = Buffer.from(`${name}/ruphish.exe`);
        const replacement = Buffer.from('../' + 'x'.repeat(filename.length - 3));
        let count = 0;
        for (
          let offset = archive.indexOf(filename);
          offset !== -1;
          offset = archive.indexOf(filename, offset + filename.length)
        ) {
          replacement.copy(archive, offset);
          count++;
        }
        assert.equal(count, 2);
      }
    }
    packages.set(version, { archive, sums: `${hash(archive)}  ${archiveName}\n` });
  }
  async function install(args: string[] = [], piped = false, modifyPath = false) {
    if (kind === 'sh') {
      const base = [
        '--install-dir',
        posix(root),
        ...(modifyPath ? [] : ['--no-modify-path']),
        ...args,
      ];
      return piped
        ? run(shell, ['-s', '--', ...base], { env, input: readFileSync(script, 'utf8') })
        : run(shell, [posix(script), ...base], { env });
    }
    const base = ['-InstallDir', root, ...(modifyPath ? [] : ['-NoModifyPath']), ...args];
    return piped
      ? run(
          powershell,
          [
            '-NoProfile',
            '-Command',
            `& ([scriptblock]::Create([IO.File]::ReadAllText('${script.replaceAll("'", "''")}'))) ${base.map((value) => (value.startsWith('-') ? value : `'${value.replaceAll("'", "''")}'`)).join(' ')}`,
          ],
          { env },
        )
      : run(powershell, ['-NoProfile', '-File', script, ...base], { env });
  }
  async function interactive(args: string[], answer: string) {
    const quote = (value: string) => `'${value.replaceAll("'", "'\\''")}'`;
    const command = [shell, script, '--install-dir', root, '--no-modify-path', ...args]
      .map(quote)
      .join(' ');
    const interactiveEnv = { ...env, TERM: 'xterm', CI: '' };
    return run('script', ['-q', '-e', '-c', command, '/dev/null'], {
      env: interactiveEnv,
      input: answer,
    });
  }
  async function locked(action: () => Promise<void>) {
    if (kind === 'sh') {
      mkdirSync(`${root}.lock`);
      writeFileSync(join(`${root}.lock`, 'pid'), String(process.pid));
      try {
        await action();
      } finally {
        rmSync(join(`${root}.lock`, 'pid'));
        rmSync(`${root}.lock`, { recursive: true });
      }
    } else {
      const code = `$f = [IO.File]::Open('${root.replaceAll("'", "''")}.lock', 'OpenOrCreate', 'ReadWrite', 'None'); [Console]::WriteLine('ready'); [Console]::ReadLine(); $f.Dispose()`;
      const child = spawn(
        powershell,
        ['-NoProfile', '-EncodedCommand', Buffer.from(code, 'utf16le').toString('base64')],
        { windowsHide: true, stdio: ['pipe', 'pipe', 'pipe'] },
      );
      await once(child.stdout, 'data');
      try {
        await action();
      } finally {
        child.stdin.end('\n');
        await once(child, 'close');
      }
    }
  }
  t.after(() => {
    server.closeAllConnections();
    server.close();
    assert.equal(readFileSync(join(data, 'keep.db'), 'utf8'), 'existing application data');
    // Only the absolute, freshly-created fixture directory is removed.
    assert.equal(dirname(directory), resolve(tmpdir()));
    rmSync(directory, { recursive: true, force: true });
  });
  return {
    directory,
    home,
    root,
    requests,
    install,
    interactive,
    locked,
    packageVersion,
    setSelected: (value: string) => {
      selected = value;
    },
    setError: (value: string) => {
      error = value;
    },
    setVersionStatus: (value: number) => {
      versionStatus = value;
    },
  };
}

for (const kind of ['sh', 'ps1'] as const) {
  test(
    `${kind}: real HTTP downloads and managed installation lifecycle`,
    { skip: kind === 'ps1' ? !windows : windows },
    async (t) => {
      const f = await fixture(t, kind);
      const flag = (shellFlag: string, psFlag: string) => (kind === 'sh' ? shellFlag : psFlag);
      const versionFlag = flag('--version', '-Version');
      await f.packageVersion('0.1.0');
      await f.packageVersion('0.2.0-dev.10');
      await f.packageVersion('0.2.0');
      await t.test(
        'invalid options and unmanaged destinations fail before downloading',
        async () => {
          let result = await f.install([
            versionFlag,
            '0.1.0',
            flag('--include-previews', '-IncludePreviews'),
          ]);
          assert.notEqual(result.code, 0);
          assert.equal(f.requests.length, 0);
          mkdirSync(f.root);
          writeFileSync(join(f.root, 'personal.txt'), 'keep');
          result = await f.install([versionFlag, '0.1.0']);
          assert.notEqual(result.code, 0);
          assert.match(result.output, /Unmanaged installation directory/);
          assert.equal(f.requests.length, 0);
          assert.equal(readFileSync(join(f.root, 'personal.txt'), 'utf8'), 'keep');
          rmSync(join(f.root, 'personal.txt'));
          rmSync(f.root, { recursive: true });
        },
      );
      await t.test(
        'default install, piped invocation, spaces/Unicode, and repeated install',
        async () => {
          let result = await f.install([], true);
          assert.equal(result.code, 0, result.output);
          assert.match(result.output, /Installed Ruphish 0\.1\.0/);
          assert.equal(readFileSync(join(f.root, 'current', '.version'), 'utf8').trim(), '0.1.0');
          assert(f.requests.includes('/api/releases?format=text&includePreviews=false'));
          const timestamp = statSync(join(f.root, 'current', '.version')).mtimeMs;
          f.requests.length = 0;
          result = await f.install();
          assert.equal(result.code, 0, result.output);
          assert.match(result.output, /already installed/);
          assert.equal(statSync(join(f.root, 'current', '.version')).mtimeMs, timestamp);
          assert.equal(f.requests.filter((url) => url.startsWith('/repo/')).length, 0);
          assert.doesNotMatch(result.output, /\x1b\[/);
        },
      );
      await t.test(
        'preview opt-in updates program files and uses the same exact tag for archive and checksum',
        async () => {
          f.setSelected('0.2.0-dev.10');
          f.requests.length = 0;
          const result = await f.install([flag('--include-previews', '-IncludePreviews')]);
          assert.equal(result.code, 0, result.output);
          assert.match(result.output, /Preview release/);
          assert(f.requests.includes('/api/releases?format=text&includePreviews=true'));
          assert(
            f.requests
              .filter((url) => url.startsWith('/repo/'))
              .every((url) => url.startsWith('/repo/releases/download/v0.2.0-dev.10/')),
          );
        },
      );
      await t.test(
        'full releases upgrade matching dev versions; returning to dev requires downgrade acceptance',
        async () => {
          let result = await f.install([versionFlag, '0.2.0']);
          assert.equal(result.code, 0, result.output);
          assert.equal(readFileSync(join(f.root, 'current', '.version'), 'utf8').trim(), '0.2.0');
          result = await f.install([versionFlag, '0.2.0-dev.10']);
          assert.notEqual(result.code, 0);
          assert.match(result.output, /Downgrade requires confirmation/);
          assert.equal(readFileSync(join(f.root, 'current', '.version'), 'utf8').trim(), '0.2.0');
          result = await f.install([
            versionFlag,
            '0.2.0-dev.10',
            flag('--allow-downgrade', '-AllowDowngrade'),
          ]);
          assert.equal(result.code, 0, result.output);
          assert.equal(
            readFileSync(join(f.root, 'current', '.version'), 'utf8').trim(),
            '0.2.0-dev.10',
          );
        },
      );
      await t.test(
        'unattended downgrade stops; explicit acceptance proceeds without touching data',
        async () => {
          let result = await f.install([versionFlag, '0.1.0']);
          assert.notEqual(result.code, 0);
          assert.match(result.output, /Downgrade requires confirmation/);
          assert.equal(
            readFileSync(join(f.root, 'current', '.version'), 'utf8').trim(),
            '0.2.0-dev.10',
          );
          result = await f.install([
            versionFlag,
            '0.1.0',
            flag('--allow-downgrade', '-AllowDowngrade'),
          ]);
          assert.equal(result.code, 0, result.output);
          assert.match(result.output, /NOT reversed/);
        },
      );
      await t.test(
        'explicit versions bypass unavailable selection; failed downloads preserve installed files',
        async () => {
          f.setVersionStatus(503);
          let result = await f.install();
          assert.notEqual(result.code, 0);
          f.requests.length = 0;
          result = await f.install([versionFlag, '0.1.0']);
          assert.equal(result.code, 0, result.output);
          assert(!f.requests.some((url) => url.startsWith('/api/')));
          f.setVersionStatus(200);
          for (const error of ['http', 'checksum', 'duplicate', 'truncated']) {
            f.setError(error);
            f.requests.length = 0;
            result = await f.install([versionFlag, '0.2.0']);
            assert.notEqual(result.code, 0, error);
            assert.match(
              result.output,
              error === 'checksum'
                ? /checksum mismatch/
                : error === 'duplicate'
                  ? /exactly one valid checksum/
                  : /Download failed/,
            );
            assert.equal(readFileSync(join(f.root, 'current', '.version'), 'utf8').trim(), '0.1.0');
            assert(!f.requests.some((url) => url.startsWith('/api/')));
          }
          f.setError('');
        },
      );
      await t.test(
        'a competing installer cannot replace or remove an active installation',
        async () => {
          await f.locked(async () => {
            const result = await f.install([versionFlag, '0.2.0']);
            assert.notEqual(result.code, 0);
            assert.match(result.output, /Another installer/);
            assert.equal(readFileSync(join(f.root, 'current', '.version'), 'utf8').trim(), '0.1.0');
          });
        },
      );
      await t.test(
        'interactive downgrade defaults to Stop and proceeds only after yes',
        { skip: kind !== 'sh' || process.platform !== 'linux' },
        async () => {
          let result = await f.install([versionFlag, '0.2.0-dev.10']);
          assert.equal(result.code, 0, result.output);
          result = await f.interactive([versionFlag, '0.1.0'], '\n');
          assert.notEqual(result.code, 0);
          assert.match(result.output, /Downgrade cancelled/);
          assert.equal(
            readFileSync(join(f.root, 'current', '.version'), 'utf8').trim(),
            '0.2.0-dev.10',
          );
          result = await f.interactive([versionFlag, '0.1.0'], 'yes\n');
          assert.equal(result.code, 0, result.output);
          assert.equal(readFileSync(join(f.root, 'current', '.version'), 'utf8').trim(), '0.1.0');
        },
      );
      await t.test(
        'unexpected archive files and wrong executable versions fail before replacement',
        async () => {
          await f.packageVersion('0.3.0', { extra: 'unexpected.txt' });
          await f.packageVersion('0.4.0', { executableVersion: '9.9.9' });
          await f.packageVersion('0.5.0', { traversal: true });
          for (const version of ['0.3.0', '0.4.0', '0.5.0']) {
            const result = await f.install([versionFlag, version]);
            assert.notEqual(result.code, 0);
            assert.match(
              result.output,
              version === '0.4.0' ? /Unexpected executable version/ : /Unexpected archive/,
            );
            assert.equal(readFileSync(join(f.root, 'current', '.version'), 'utf8').trim(), '0.1.0');
          }
        },
      );
      await t.test('interrupted replacement is recovered on the next invocation', async () => {
        renameSync(join(f.root, 'current'), join(f.root, 'previous'));
        const result = await f.install([versionFlag, '0.1.0']);
        assert.equal(result.code, 0, result.output);
        assert.match(result.output, /Recovered program files/);
        assert(!existsSync(join(f.root, 'previous')));
      });
      await t.test(
        'shell PATH blocks are idempotent and preserve unrelated profile contents',
        { skip: kind !== 'sh' },
        async () => {
          writeFileSync(join(f.home, '.profile'), '# existing user configuration\n');
          for (let i = 0; i < 2; i++) {
            const result = await f.install([versionFlag, '0.1.0'], false, true);
            assert.equal(result.code, 0, result.output);
          }
          const profile = readFileSync(join(f.home, '.profile'), 'utf8');
          assert.equal(profile.split('# >>> ruphish installer >>>').length, 2);
          assert(profile.startsWith('# existing user configuration\n'));
        },
      );
      await t.test(
        'unknown files block removal, and normal removal retains application data',
        async () => {
          writeFileSync(join(f.root, 'unmanaged.txt'), 'preserve me');
          let result = await f.install([flag('--uninstall', '-Uninstall')]);
          assert.notEqual(result.code, 0);
          assert.equal(readFileSync(join(f.root, 'unmanaged.txt'), 'utf8'), 'preserve me');
          rmSync(join(f.root, 'unmanaged.txt'));
          result = await f.install([flag('--uninstall', '-Uninstall')], false, kind === 'sh');
          assert.equal(result.code, 0, result.output);
          assert(!existsSync(f.root));
          if (kind === 'sh') {
            assert(!existsSync(join(f.home, '.local/bin/ruphish')));
            assert.equal(
              readFileSync(join(f.home, '.profile'), 'utf8').trim(),
              '# existing user configuration',
            );
          }
        },
      );
    },
  );
}
