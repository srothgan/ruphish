// One deadline covers connection, headers and body. Dropping the Rust future cancels Fetch.
export function startRequest(request, milliseconds, onHeaders = () => {}, download = false) {
    return startRequestWith(fetch, request, milliseconds, onHeaders, download);
}

// Explicit executor injection keeps demo requests out of the network without replacing global Fetch.
export function startRequestWith(execute, request, milliseconds, onHeaders = () => {}, download = false) {
    const controller = new AbortController();
    let timedOut = false;
    const timer = setTimeout(() => {
        timedOut = true;
        controller.abort();
    }, milliseconds);
    const promise = (async () => {
        try {
            const response = await execute(request, { signal: controller.signal });
            if (controller.signal.aborted) throw new Error('aborted');
            onHeaders(response.status);
            const file = download && response.ok ? await response.blob() : null;
            const body = file ? "" : await response.text();
            if (controller.signal.aborted) throw new Error('aborted');
            return { status: response.status, etag: response.headers.get('etag'), body, ...(download ? { file } : {}) };
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

export function saveDownload(result, filename) {
    if (!(result.file instanceof Blob)) throw new Error('invalid_download');
    const anchor = document.createElement('a');
    const url = URL.createObjectURL(result.file);
    try {
        anchor.href = url;
        anchor.download = filename;
        document.body.append(anchor);
        anchor.click();
        setTimeout(() => URL.revokeObjectURL(url), 1000);
    } catch (error) {
        URL.revokeObjectURL(url);
        throw error;
    } finally {
        anchor.remove();
    }
}
