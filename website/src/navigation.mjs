/**
 * Keep the links available without JavaScript and collapse them only on mobile.
 * @param {HTMLElement} header
 * @param {MediaQueryList} mobile
 */
export function setupNavigation(header, mobile) {
  const button = header.querySelector('button[aria-controls]');
  const nav = header.querySelector('nav');
  if (!button || !nav) return;

  const document = header.ownerDocument;
  function setOpen(open) {
    button.setAttribute('aria-expanded', String(mobile.matches && open));
    nav.hidden = mobile.matches && !open;
  }

  function reset() {
    const active = document.activeElement;
    button.hidden = !mobile.matches;
    setOpen(false);
    if (mobile.matches && nav.contains(active)) button.focus();
    else if (!mobile.matches && active === button) nav.querySelector('a')?.focus();
  }

  button.addEventListener('click', () => {
    setOpen(button.getAttribute('aria-expanded') !== 'true');
  });
  header.addEventListener('keydown', (event) => {
    if (event.key === 'Escape' && button.getAttribute('aria-expanded') === 'true') {
      setOpen(false);
      button.focus();
      event.preventDefault();
    }
  });
  nav.addEventListener('click', (event) => {
    if (event.target.closest('a')) setOpen(false);
  });
  document.addEventListener('click', (event) => {
    if (!header.contains(event.target)) setOpen(false);
  });
  header.addEventListener('focusout', (event) => {
    if (!header.contains(event.relatedTarget)) setOpen(false);
  });
  mobile.addEventListener('change', reset);
  header.dataset.navigation = '';
  reset();
}
