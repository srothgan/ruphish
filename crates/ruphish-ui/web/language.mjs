// Browser defaults only. The authenticated account owns saved overrides.
export function browserLanguages(browser = globalThis) {
  const navigator = browser.navigator;
  return navigator?.languages?.length
    ? [...navigator.languages]
    : navigator?.language
      ? [navigator.language]
      : [];
}
