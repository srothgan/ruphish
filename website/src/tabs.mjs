/** @param {HTMLElement} group */
export function setupTabs(group) {
  const tablist = group.querySelector(':scope > [role="tablist"]');
  const tabs = [...tablist.querySelectorAll('[role="tab"]')];
  const panels = [...group.querySelectorAll(':scope > [role="tabpanel"]')];

  function select(index) {
    tabs.forEach((tab, position) => {
      tab.setAttribute('aria-selected', String(position === index));
      tab.tabIndex = position === index ? 0 : -1;
      panels[position].hidden = position !== index;
    });
  }

  tablist.addEventListener('click', (event) => {
    const index = tabs.indexOf(event.target.closest('[role="tab"]'));
    if (index >= 0) {
      select(index);
    }
  });
  tablist.addEventListener('keydown', (event) => {
    const index = tabs.indexOf(event.target.closest('[role="tab"]'));
    if (index < 0) {
      return;
    }
    let next;
    switch (event.key) {
      case 'ArrowRight':
        next = (index + 1) % tabs.length;
        break;
      case 'ArrowLeft':
        next = (index + tabs.length - 1) % tabs.length;
        break;
      case 'Home':
        next = 0;
        break;
      case 'End':
        next = tabs.length - 1;
        break;
      default:
        return;
    }
    event.preventDefault();
    select(next);
    tabs[next].focus();
  });
}
