const formatters = new Map();
function formatter(locale, timeZone, options) {
    const key = JSON.stringify([locale, timeZone, options]);
    if (!formatters.has(key)) formatters.set(key, new Intl.DateTimeFormat(locale, { ...options, ...(timeZone ? { timeZone } : {}) }));
    return formatters.get(key);
}
export function analysisMinute(value, locale, timeZone) {
    const date = new Date(value);
    if (!Number.isFinite(date.getTime())) return 'Not recorded';
    return formatter(locale, timeZone, { year: 'numeric', month: '2-digit', day: '2-digit', hour: '2-digit', minute: '2-digit' })
        .format(new Date(Math.floor(date.getTime() / 60000) * 60000));
}
export function analysisZone() { return Intl.DateTimeFormat().resolvedOptions().timeZone; }
export function analysisTick(seconds, step, first, last, locale, timeZone) {
    const a = new Date(first * 1000), b = new Date(last * 1000);
    const calendar = formatter(locale, timeZone, { year: 'numeric', month: 'numeric', day: 'numeric' });
    const sameDay = calendar.format(a) === calendar.format(b);
    const options = step >= 86400 ? { month: 'short', day: 'numeric' }
        : sameDay ? { hour: '2-digit', minute: '2-digit' }
            : { month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit' };
    const year = formatter(locale, timeZone, { year: 'numeric' });
    if (year.format(a) !== year.format(b)) options.year = 'numeric';
    if (step < 86400) {
        const offset = formatter(locale, timeZone, { timeZoneName: 'shortOffset' });
        const zoneAt = date => offset.formatToParts(date).find(part => part.type === 'timeZoneName')?.value;
        if (zoneAt(a) !== zoneAt(b)) options.timeZoneName = 'shortOffset';
    }
    return formatter(locale, timeZone, options).format(new Date(seconds * 1000));
}
export function observeAnalysisWidth(element, callback) {
    const observer = new ResizeObserver(entries => callback(entries[0].contentRect.width));
    observer.observe(element);
    return observer;
}
export function unobserveAnalysisWidth(observer) { observer.disconnect(); }
