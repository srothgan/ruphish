import { execFileSync } from 'node:child_process';
import { fileURLToPath, pathToFileURL } from 'node:url';

export const affectsWebsite = (paths) => paths.some((path) => /^(website|docs|assets)\//.test(path.replaceAll('\\', '/')));

/** Vercel: zero skips, one builds. Unknown history must build, never silently skip. */
export function shouldBuild(env, git) {
  const previous = env.VERCEL_GIT_PREVIOUS_SHA;
  const current = env.VERCEL_GIT_COMMIT_SHA;
  if (!/^[0-9a-f]{40,64}$/i.test(previous ?? '') || !/^[0-9a-f]{40,64}$/i.test(current ?? '')) return true;
  try {
    // Include both sides of renames, and all commits since the previous deployment.
    const changed = git(['diff', '--name-only', '--no-renames', '-z', previous, current, '--', 'website/', 'docs/', 'assets/']);
    return affectsWebsite(changed.split('\0').filter(Boolean));
  } catch { return true; }
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const cwd = fileURLToPath(new URL('../../', import.meta.url));
  const build = shouldBuild(process.env, (args) => execFileSync('git', args, { cwd, encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'] }));
  console.log(build ? 'Build: website/docs/assets changed, or no reliable baseline.' : 'Skip: website, docs and assets unchanged.');
  process.exitCode = build ? 1 : 0;
}
