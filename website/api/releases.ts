import { releaseResponse } from '../lib/release-selection.ts';

export function GET(request: Request): Promise<Response> {
  return releaseResponse(request);
}
