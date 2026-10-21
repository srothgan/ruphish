// Locale-only presentation. Rust/Jiff supplies valid, already-zoned civil fields.
// CLDR 48.0.0 locale inventory; source and Unicode license are in THIRD-PARTY-NOTICES.txt.
import localeInventory from './format-locales.json' with {type: 'json'};
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
    const options = {
        ...(value.style !== 'clock' ? {month: '2-digit', day: '2-digit', ...(value.include_year ? {year: 'numeric'} : {})} : {}),
        ...(value.style !== 'date' ? {hour: '2-digit', minute: '2-digit'} : {}),
    };
    const text = formatter(locale, options).format(date);
    return value.offset ? `${text} (${value.offset})` : text;
}
export function browserFormatLocale() { return Intl.DateTimeFormat().resolvedOptions().locale; }
export function supportsFormatLocale(locale) { return Intl.DateTimeFormat.supportedLocalesOf([locale]).length > 0; }

// Intl cannot enumerate locales. Use CLDR candidates, keeping only browser-supported ones.
export function formatLocaleOptions(displayLocale, selected = '', sampleJson) {
    const candidates = localeInventory.availableLocales.full.map(value => {
        const locale = new Intl.Locale(value);
        return new Intl.Locale(locale, {region: locale.region ?? locale.maximize().region}).toString();
    });
    candidates.push(browserFormatLocale(), ...globalThis.navigator?.languages ?? []);
    if (selected) candidates.push(selected);
    const valid = [];
    for (const candidate of candidates) {
        try { valid.push(...Intl.DateTimeFormat.supportedLocalesOf([candidate])); } catch { /* Ignore an obsolete saved identifier. */ }
    }
    if (!sampleJson) throw new TypeError('A date label is required for format examples');
    const options = [...new Set(valid)].map(value => {
        const example = formatDateLabel(sampleJson, value);
        return [value, `${formatLocaleName(displayLocale, value)} — ${example} (${value})`];
    });
    options.sort((a, b) => a[1].localeCompare(b[1], displayLocale));
    return JSON.stringify(options);
}

export function browserTimeZone() { return Intl.DateTimeFormat().resolvedOptions().timeZone || 'UTC'; }

export function formatLocaleName(displayLocale, locale) {
    return new Intl.DisplayNames([displayLocale], {type: 'language'}).of(new Intl.Locale(locale).baseName);
}
