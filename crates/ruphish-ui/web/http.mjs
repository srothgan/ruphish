// One deadline covers connection, headers and body. Dropping the Rust future cancels Fetch.
export function startRequest(request, milliseconds, onHeaders = () => {}) {
    const controller = new AbortController();
    let timedOut = false;
    const timer = setTimeout(() => {
        timedOut = true;
        controller.abort();
    }, milliseconds);
    const promise = (async () => {
        try {
            const response = await fetch(request, { signal: controller.signal });
            if (controller.signal.aborted) throw new Error('aborted');
            onHeaders(response.status);
            const body = await response.text();
            if (controller.signal.aborted) throw new Error('aborted');
            return { status: response.status, etag: response.headers.get('etag'), body };
        } catch {
            // Never propagate URLs, headers, response bodies or platform diagnostics.
            throw new Error(timedOut ? 'request_timeout' : 'request_unavailable');
        } finally {
            clearTimeout(timer);
        }
    })();
    return { promise, cancel: () => { clearTimeout(timer); controller.abort(); } };
}
export function requestPromise(request) { return request.promise; }
export function cancelRequest(request) { request.cancel(); }
