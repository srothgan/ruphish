// Locale-only presentation. Rust/Jiff supplies valid, already-zoned civil fields.
const formatters = new Map();
function formatter(locale, options) {
    const key = JSON.stringify([locale, options]);
    if (!formatters.has(key)) formatters.set(key, new Intl.DateTimeFormat(locale, { ...options, timeZone: 'UTC' }));
    return formatters.get(key);
}
export function formatDateLabel(json, locale) {
    const value = JSON.parse(json);
    const date = new Date(0);
    date.setUTCFullYear(value.year, value.month - 1, value.day);
    date.setUTCHours(value.hour, value.minute, 0, 0);
    const options = { month: value.style === 'full' ? 'long' : 'short', day: 'numeric', ...(value.include_year ? { year: 'numeric' } : {}) };
    const day = () => formatter(locale, options).format(date);
    const clock = () => formatter(locale, { hour: '2-digit', minute: '2-digit' }).format(date);
    const text = value.style === 'clock' ? clock() : value.style === 'date' ? day() : `${day()}, ${clock()}`;
    return value.offset ? `${text} (${value.offset})` : text;
}
export function loadTimeZone(key) { return localStorage.getItem(key) || ''; }
export function saveTimeZone(key, value) {
    if (value) localStorage.setItem(key, value);
    else localStorage.removeItem(key);
}

export function browserTimeZone() { return Intl.DateTimeFormat().resolvedOptions().timeZone || 'UTC'; }
