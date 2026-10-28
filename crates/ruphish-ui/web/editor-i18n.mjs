import en from '../locales/en.json' with { type: 'json' };
import de from '../locales/de.json' with { type: 'json' };

// Only trusted application chrome uses this catalog; authored HTML is never scanned.
const catalogs = { en: en.editor, de: de.editor };
export function editorText(locale, key) {
  return catalogs[locale]?.[key] ?? catalogs.en[key] ?? key;
}

export class EditorError extends Error {
  constructor(key) {
    super(editorText('en', key));
    this.key = key;
  }
}

export function editorError(locale, error) {
  return error instanceof EditorError ? editorText(locale, error.key) : String(error);
}

export function localizeVisualShell(document, locale) {
  document.documentElement.lang = locale;
  for (const element of document.querySelectorAll('[data-editor-text]')) {
    element.textContent = editorText(locale, element.dataset.editorText);
  }
  for (const element of document.querySelectorAll('[data-editor-label]')) {
    element.setAttribute('aria-label', editorText(locale, element.dataset.editorLabel));
  }
}

export function codeMirrorPhrases(locale) {
  const catalog = locale === 'de' ? de.codemirror : en.codemirror;
  return Object.fromEntries(
    Object.entries(en.codemirror).map(([key, phrase]) => [phrase, catalog[key] ?? phrase]),
  );
}

export function contentLocale(document, fallback) {
  const language = document.documentElement.lang.split('-')[0].toLowerCase();
  return ['en', 'de'].includes(language) ? language : fallback;
}
