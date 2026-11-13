// Shared by both page layouts; inlined before paint by ThemeProvider.astro.
(() => {
  const storageKey = 'starlight-theme';
  const system = window.matchMedia('(prefers-color-scheme: dark)');
  const parse = (value) => (value === 'light' || value === 'dark' ? value : 'auto');
  let preference = 'auto';

  try {
    preference = parse(window.localStorage.getItem(storageKey));
  } catch {
    // A blocked preference store still allows theme selection for this page.
  }

  function apply() {
    document.documentElement.dataset.theme =
      preference === 'auto' ? (system.matches ? 'dark' : 'light') : preference;
    for (const picker of document.querySelectorAll('[data-theme-select]')) {
      picker.value = preference;
      const control = picker.closest('[data-theme-control]');
      control.dataset.preference = preference;
      control.hidden = false;
    }
  }

  apply();
  document.addEventListener('DOMContentLoaded', apply, { once: true });
  document.addEventListener('change', (event) => {
    if (
      !(event.target instanceof HTMLSelectElement) ||
      !event.target.hasAttribute('data-theme-select')
    ) {
      return;
    }
    preference = parse(event.target.value);
    apply();
    try {
      window.localStorage.setItem(storageKey, preference === 'auto' ? '' : preference);
    } catch {
      // Applying the selected theme does not depend on persistence.
    }
  });
  system.addEventListener('change', () => {
    if (preference === 'auto') {
      apply();
    }
  });
  window.addEventListener('storage', (event) => {
    if (event.key === storageKey || event.key === null) {
      preference = parse(event.newValue);
      apply();
    }
  });
})();
