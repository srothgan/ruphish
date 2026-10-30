// Website-side interpretation of the published release asset contract.
export const repository = 'srothgan/ruphish';
export const targets = [
  'x86_64-pc-windows-msvc',
  'aarch64-pc-windows-msvc',
  'x86_64-unknown-linux-gnu',
  'aarch64-unknown-linux-gnu',
  'aarch64-apple-darwin',
] as const;

export type ReleaseTarget = (typeof targets)[number];
export interface ReleaseVersion {
  version: string;
  base: string;
  channel: 'dev' | 'final';
  number: string | null;
}

export function parseVersion(version: string): ReleaseVersion {
  const match = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-(dev)\.([1-9]\d*))?$/.exec(version);
  if (!match || match[0] !== version) {
    throw new Error(`Unsupported release version: ${version}`);
  }
  return {
    version,
    base: match.slice(1, 4).join('.'),
    channel: match[4] === 'dev' ? 'dev' : 'final',
    number: match[5] ?? null,
  };
}

export function compareVersions(a: string, b: string): number {
  const fields = (version: string) => {
    const v = parseVersion(version);
    return [
      ...v.base.split('.').map(BigInt),
      BigInt(v.channel === 'final' ? 1 : 0),
      BigInt(v.number ?? 0),
    ];
  };
  const x = fields(a);
  const y = fields(b);
  for (let i = 0; i < x.length; i++) {
    if (x[i] !== y[i]) {
      return x[i] > y[i] ? 1 : -1;
    }
  }
  return 0;
}

export function archiveName(version: string, target: ReleaseTarget): string {
  parseVersion(version);
  if (!targets.includes(target)) {
    throw new Error('Unsupported native target');
  }
  return `ruphish-${version}-${target}.${target.includes('windows') ? 'zip' : 'tar.gz'}`;
}
