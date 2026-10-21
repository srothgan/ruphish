// Shared positioning, dismissal and focus behavior for anchored panels.
export function bindAnchoredPanel(trigger, panel, win = window) {
  let disposed = false;
  const position = () => {
    if (panel.hidden) return;
    const viewport = win.visualViewport;
    const width = viewport?.width ?? win.innerWidth;
    const height = viewport?.height ?? win.innerHeight;
    const left = viewport?.offsetLeft ?? 0;
    const top = viewport?.offsetTop ?? 0;
    const margin = 12;
    const rect = trigger.getBoundingClientRect();
    const preferredWidth = Number(panel.dataset.panelWidth) || 304;
    const panelWidth = Math.max(0, Math.min(preferredWidth, width - 2 * margin));
    panel.style.width = `${panelWidth}px`;
    const above = Math.max(0, Math.min(height - 2 * margin, rect.top - top - margin - 8));
    const below = Math.max(0, Math.min(height - 2 * margin, top + height - margin - rect.bottom - 8));
    const useAbove = above >= panel.scrollHeight || above >= below;
    const room = Math.max(1, useAbove ? above : below);
    panel.style.maxHeight = `${room}px`;
    const actualHeight = Math.min(panel.scrollHeight, room);
    const y = useAbove ? rect.top - 8 - actualHeight : rect.bottom + 8;
    panel.style.left = `${Math.max(left + margin, Math.min(rect.left, left + width - panelWidth - margin))}px`;
    panel.style.top = `${Math.max(top + margin, Math.min(y, top + height - actualHeight - margin))}px`;
  };
  const close = (restoreFocus = true) => {
    if (panel.hidden) return;
    panel.hidden = true;
    trigger.setAttribute('aria-expanded', 'false');
    if (restoreFocus && trigger.isConnected) trigger.focus();
  };
  const toggle = () => {
    if (trigger.disabled) return;
    if (!panel.hidden) { close(); return; }
    panel.hidden = false;
    trigger.setAttribute('aria-expanded', 'true');
    position();
    panel.querySelector('input:not(:disabled),a[href],button:not(:disabled)')?.focus();
  };
  const outside = event => {
    if (event.target.closest?.('dialog[open]')) return;
    if (!panel.contains(event.target) && !trigger.contains(event.target)) close(false);
  };
  const keyboard = event => {
    if (event.defaultPrevented || win.document.querySelector('dialog[open]')) return;
    if (event.key === 'Enter' && !panel.hidden && panel.contains(event.target) && event.target.matches?.('input[type="search"]')) {
      event.preventDefault(); // Searching a picker must never implicitly submit its enclosing form.
    }
    if (event.key === 'Escape' && !panel.hidden) { event.preventDefault(); close(); }
  };
  const focus = event => {
    if (event.target.closest?.('dialog[open]')) return;
    if (!panel.contains(event.target) && !trigger.contains(event.target)) close(false);
  };
  const link = event => {
    const action = event.target.closest?.('a[href],button,[data-panel-close]');
    if (!action || action.disabled || event.defaultPrevented) return;
    if (action.matches('input[type="radio"]') && event.detail === 0) return; // Native arrow/space navigation stays open until Done/Escape.
    if (action.matches('a[href], [data-panel-close]') || panel.hasAttribute('data-close-after-action')) close();
  };
  const disabledObserver = new win.MutationObserver(() => { if (trigger.disabled) close(false); });
  disabledObserver.observe(trigger, {attributes: true, attributeFilter: ['disabled']});
  trigger.addEventListener('click', toggle);
  panel.addEventListener('click', link);
  win.addEventListener('pointerdown', outside);
  win.addEventListener('keydown', keyboard);
  win.addEventListener('focusin', focus);
  win.addEventListener('resize', position);
  win.addEventListener('scroll', position, true);
  win.visualViewport?.addEventListener('resize', position);
  win.visualViewport?.addEventListener('scroll', position);
  const observer = typeof win.ResizeObserver === 'function' ? new win.ResizeObserver(position) : null;
  observer?.observe(panel);
  observer?.observe(trigger);
  return () => {
    if (disposed) return;
    disposed = true;
    close(false);
    trigger.removeEventListener('click', toggle);
    panel.removeEventListener('click', link);
    win.removeEventListener('pointerdown', outside);
    win.removeEventListener('keydown', keyboard);
    win.removeEventListener('focusin', focus);
    win.removeEventListener('resize', position);
    win.removeEventListener('scroll', position, true);
    win.visualViewport?.removeEventListener('resize', position);
    win.visualViewport?.removeEventListener('scroll', position);
    observer?.disconnect();
    disabledObserver.disconnect();
  };
}
