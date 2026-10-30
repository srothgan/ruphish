import {
  archiveName,
  compareVersions,
  parseVersion,
  repository,
  targets,
} from './release-contract.ts';
import type { ReleaseTarget } from './release-contract.ts';

export interface SelectedRelease {
  version: string;
  preview: boolean;
  notes: string;
  checksums: string;
  provenance: string;
  assets: { target: ReleaseTarget; url: string }[];
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

export function selectRelease(
  releases: unknown[],
  includePreviews: boolean,
): SelectedRelease | null {
  const candidates = releases.flatMap((release) => {
    if (
      !isRecord(release) ||
      typeof release.draft !== 'boolean' ||
      typeof release.tag_name !== 'string'
    ) {
      throw new Error('Invalid GitHub release metadata');
    }
    if (release.draft || release.published_at === null || !release.tag_name.startsWith('v')) {
      return [];
    }
    if (
      typeof release.published_at !== 'string' ||
      !Number.isFinite(Date.parse(release.published_at))
    ) {
      throw new Error('Invalid GitHub publication date');
    }
    let parsed;
    try {
      parsed = parseVersion(release.tag_name.slice(1));
    } catch {
      return [];
    }
    if (release.prerelease !== (parsed.channel !== 'final')) {
      throw new Error(`Release classification disagrees with ${release.tag_name}`);
    }
    return includePreviews || parsed.channel === 'final' ? [{ release, parsed }] : [];
  });
  candidates.sort((a, b) => compareVersions(b.parsed.version, a.parsed.version));
  if (!candidates.length) {
    return null;
  }
  const { release, parsed } = candidates[0];
  const names = [
    ...targets.map((target) => archiveName(parsed.version, target)),
    'SHA256SUMS',
    'release-manifest.json',
    'native-provenance.jsonl',
  ];
  const base = `https://github.com/${repository}/releases/download/${release.tag_name}`;
  for (const name of names) {
    const matches = Array.isArray(release.assets)
      ? release.assets.filter((asset: unknown) => isRecord(asset) && asset.name === name)
      : [];
    const asset: unknown = matches[0];
    if (
      matches.length !== 1 ||
      !isRecord(asset) ||
      asset.state !== 'uploaded' ||
      typeof asset.size !== 'number' ||
      !Number.isSafeInteger(asset.size) ||
      asset.size <= 0 ||
      asset.browser_download_url !== `${base}/${name}`
    ) {
      throw new Error(`Release ${release.tag_name} has an unavailable asset: ${name}`);
    }
  }
  return {
    version: parsed.version,
    preview: parsed.channel !== 'final',
    notes: `https://github.com/${repository}/releases/tag/${release.tag_name}`,
    checksums: `${base}/SHA256SUMS`,
    provenance: `${base}/native-provenance.jsonl`,
    assets: targets.map((target) => ({
      target,
      url: `${base}/${archiveName(parsed.version, target)}`,
    })),
  };
}

export async function publishedReleases(fetcher: typeof fetch = fetch): Promise<unknown[]> {
  const releases: unknown[] = [];
  const signal = AbortSignal.timeout(15_000);
  for (let page = 1; page <= 50; page++) {
    const response = await fetcher(
      `https://api.github.com/repos/${repository}/releases?per_page=100&page=${page}`,
      {
        headers: {
          Accept: 'application/vnd.github+json',
          'User-Agent': 'ruphish-install',
          'X-GitHub-Api-Version': '2022-11-28',
        },
        signal,
      },
    );
    if (!response.ok) {
      throw new Error(`GitHub release discovery returned HTTP ${response.status}`);
    }
    const batch: unknown = await response.json();
    if (!Array.isArray(batch)) {
      throw new Error('Invalid GitHub release list');
    }
    releases.push(...batch);
    if (batch.length < 100) {
      return releases;
    }
  }
  throw new Error('Release discovery exceeded its pagination limit');
}

export async function releaseResponse(
  request: Request,
  fetcher: typeof fetch = fetch,
): Promise<Response> {
  const query = new URL(request.url).searchParams;
  const text = query.get('format') === 'text';
  const reply = (status: number, value: string | SelectedRelease): Response =>
    new Response(
      text
        ? `${typeof value === 'string' ? value : value.version}\n`
        : JSON.stringify(typeof value === 'string' ? { error: value } : value),
      {
        status,
        headers: {
          'Content-Type': text ? 'text/plain; charset=utf-8' : 'application/json; charset=utf-8',
          'Cache-Control': status === 200 ? 'public, max-age=0, s-maxage=300' : 'no-store',
          'X-Content-Type-Options': 'nosniff',
        },
      },
    );
  if (
    request.method !== 'GET' ||
    [...query.keys()].some((key) => !['format', 'includePreviews'].includes(key)) ||
    query.getAll('format').length > 1 ||
    query.getAll('includePreviews').length > 1 ||
    ![null, 'json', 'text'].includes(query.get('format')) ||
    ![null, 'true', 'false'].includes(query.get('includePreviews'))
  ) {
    return reply(400, 'Use GET with format=json|text and includePreviews=true|false.');
  }
  const includePreviews = query.get('includePreviews') === 'true';
  try {
    const selected = selectRelease(await publishedReleases(fetcher), includePreviews);
    return selected
      ? reply(200, selected)
      : reply(
          404,
          includePreviews
            ? 'No published release is available yet.'
            : 'No full release is available yet. Use --include-previews (PowerShell: -IncludePreviews) to include development previews.',
        );
  } catch (error) {
    console.error(
      'Release discovery failed:',
      error instanceof Error ? error.message : 'Unknown upstream error',
    );
    return reply(
      503,
      'Release discovery is temporarily unavailable. Retry later or select an exact published version.',
    );
  }
}
