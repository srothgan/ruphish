import assert from 'node:assert/strict';

export function github(repository, token) {
  assert(/^[\w.-]+\/[\w.-]+$/.test(repository), 'Invalid repository');
  assert(token, 'GitHub token is required');
  return async function request(
    path,
    { method = 'GET', body, bytes = false, missing = false } = {},
  ) {
    const response = await fetch(`https://api.github.com/repos/${repository}${path}`, {
      method,
      headers: {
        Authorization: `Bearer ${token}`,
        Accept: bytes ? 'application/octet-stream' : 'application/vnd.github+json',
        'X-GitHub-Api-Version': '2022-11-28',
        ...(body ? { 'Content-Type': 'application/json' } : {}),
      },
      body: body ? JSON.stringify(body) : undefined,
      signal: AbortSignal.timeout(120_000),
    });
    if (missing && response.status === 404) {
      return null;
    }
    assert(response.ok, `GitHub ${method} ${path}: HTTP ${response.status}`);
    if (response.status === 204) {
      return null;
    }
    return bytes ? Buffer.from(await response.arrayBuffer()) : response.json();
  };
}
