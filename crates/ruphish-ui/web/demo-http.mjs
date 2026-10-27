import { startRequestWith } from './http.mjs';
import { createDemoApi } from './demo-api.mjs';
export { requestPromise, cancelRequest, saveDownload } from './http.mjs';

let api;
export function startRequest(request, milliseconds, onHeaders, download) {
    if (!api) {
        let storage;
        try { storage = window.sessionStorage; } catch { /* Login reports unavailable storage. */ }
        let now = Math.floor(Date.now() / 1000);
        try {
            const key = 'ruphish-demo-time-v1', previous = Number(storage?.getItem(key));
            if (Number.isSafeInteger(previous) && previous > 0) now = previous;
            else storage?.setItem(key, String(now));
        } catch { /* Login reports unavailable storage. */ }
        api = createDemoApi({ storage, now });
    }
    return startRequestWith(api.execute, request, milliseconds, onHeaders, download);
}
