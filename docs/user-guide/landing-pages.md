---
title: Create landing pages
description: Author, import, preview, and configure pages used by campaign links.
sidebar:
  order: 7
---

A landing page is what a recipient sees after following the campaign link. Pages support authored JavaScript, inline event handlers, external scripts, images, stylesheets and fonts. Ruphish's built-in collector records interactions and password presence without collecting entered values. Custom code can read or transmit input; review your code and dependencies before launch.

Administrators can choose a ready-made, independent page from the [Starter Template Repository](starter-templates.md), with optional email prefill and company branding on supported layouts.

## Create or import

Open **Resources → Landing pages → Create**, enter a name, and author the HTML. You can use [template expressions](template-reference.md) for recipient personalization.

To reuse existing content, choose **Import** from the landing-page list. Provide HTML, resource JSON, or a website URL. Website imports validate certificates and apply the installation's outbound restrictions at every redirect. They preserve base-URL semantics; the accepted legacy `include_resources` option does not inline external resources.

Review the imported draft and save it explicitly. Ruphish preserves authored scripts and resources while preparing a full document with managed submission and browser isolation. It removes native form destinations, embedded frames/objects, authored HTTP-equivalent metadata (including CSP and refresh), and duplicate titles. The first head title is retained. Authored `iframe`, `frame`, and other embedded documents are not supported in the first release. Author the content directly in the page or start from a [starter template](starter-templates.md). The sandboxed iframe used by the administration UI for preview does not grant support for embedded frames inside authored content. Learning pages have a separate [static content policy](learning-pages.md).

### Upload a page with local assets

Choose a ZIP file in the landing-page import dialog. Put `index.html` at its root and include the files referenced by the page. Relative image, classic script and stylesheet references are embedded in the resulting HTML, including nested CSS `@import`, `url()` and `image-set()` dependencies, inline styles, fonts and image `srcset` candidates. Supported asset extensions are CSS, JS, PNG, JPEG, GIF, SVG, WebP, AVIF, ICO, WOFF, WOFF2, TTF and OTF. HTML and CSS must be UTF-8.

The compressed archive, expanded files and resulting embedded document each have an 8 MiB limit; the archive may contain at most 256 entries. Missing local assets, cyclic stylesheet imports, unsafe paths and unsupported resource types reject the import. External HTTP(S) resources remain external; the importer does not download them. Bundle local JavaScript modules into a classic script before uploading. Dependencies calculated by JavaScript, such as relative `fetch()` calls, are not discovered or rewritten. Use explicit external URLs for those requests. Template expressions inside embedded asset bytes are not expanded.

Embedded assets are part of the HTML: they persist with the page, travel with duplication and JSON export/import, and are removed with the page or its replacement HTML. There are no separately orphaned asset files. Unreferenced archive files are not retained. Integrity attributes on embedded resources are removed because packaging can change their bytes; external integrity attributes are preserved. Website URL imports continue to fetch HTML only. External resource availability, browser CORS requirements and mixed-content rules still apply; an HTTPS campaign should use HTTPS resources.

## Forms and submission tracking

Each accepted built-in submission records an event and password presence. A separate submission creates a separate event; retrying an unconfirmed delivery does not. The page reports confirmation only after the backend acknowledges the event. Failed HTML validation, typing and ordinary `type="button"` controls do not create submission events.

The built-in collector submits an event identity and a presence boolean; it does not serialize form values. Native form navigation is blocked, including when JavaScript fails. Ordinary forms use browser validation such as `required` before collection. Authors may use scripts for additional validation, multi-step interfaces and dynamic controls. See [participant data](participant-data.md) for the boundary between built-in collection and authored code.

### Custom submission handlers

The collector observes uncancelled submit events at the window after form and document handlers. Cancel the event with `event.preventDefault()` when custom validation or another step must finish first. For a final custom submission, call `await window.ruphish.submit(form)` exactly once; it checks native validity and uses the same collector, feedback, retries and campaign redirect. It resolves `true` only for an acknowledged live event, or `false` for invalid input, an unconfirmed delivery or a preview. A separate call creates a separate observation; use the provided retry button after uncertain delivery rather than calling again. Do not use `form.submit()`, which bypasses submit events; use `requestSubmit()` for ordinary submissions.

```html
<form id="exercise">
  <label>Email <input name="username" type="email" required></label>
  <label>Password <input name="password" type="password" required></label>
  <label><input id="ready" type="checkbox"> Ready to continue</label>
  <button type="submit">Continue</button>
</form>
<script>
  const form = document.getElementById('exercise');
  form.addEventListener('submit', async (event) => {
    event.preventDefault();
    if (!document.getElementById('ready').checked) return;
    await window.ruphish.submit(form);
  });
</script>
```

Custom handlers attached at window level, handlers that stop propagation, and direct script network requests are not guaranteed to be observed automatically. Use the explicit API when your page owns the final interaction. Keep intermediate buttons as `type="button"`; otherwise a valid ordinary form can submit before the intended final step.

A redirect can be an HTTP(S) or relative destination, including supported template expressions. It runs after an acknowledged submission. Leave it empty to stay on the page. Do not place entered values into a redirect URL.

## Page checks, preview, and testing

Use **Check landing page** at the right of the editor toolbar, beside the field picker and source/visual controls, to inspect the current unsaved source. Results appear below the toolbar and clear when the HTML changes. The check does not save or change your draft, execute scripts, or test redirects. The optional redirect remains a separate field below the editor. Saving, loading, previewing, and campaign review also return advisory warnings from the processed sample page. Checks flag disabled or unnamed password controls, missing form association, duplicate IDs, repaired HTML and forms with no usable submission control. Warnings do not prevent saving and do not certify arbitrary scripts. Verify template-dependent forms with representative recipients.

The editor's **Preview** tab and landing starter previews run scripts and external resources in an isolated frame. Built-in submissions show preview feedback and create no campaign results or campaign redirects. Custom scripts can still contact their own external services. The visual editor and saved-source overview remain inert; use the editor Preview tab to test behavior.

Both live pages and functional previews run without the administrative application's origin, cookies or storage access. Native form submissions, child frames, objects, workers, pop-ups and preview navigation of the admin window are not enabled. This isolation can affect libraries requiring origin storage or authenticated requests; external module scripts and fonts must permit cross-origin loading. The page is not a general authenticated web application host. Go templates retain text-template behavior: escape recipient values for their insertion context and avoid interpolating them into executable code. See [template expressions](template-reference.md).

Before launch, inspect scripts, external requests, layout, labels, keyboard access and the expected redirect using a synthetic local campaign. User-owned [browser checks](../contributing/testing/manual-ui.md) remain required; source and protocol tests alone do not establish browser sandbox enforcement.

Editing a page changes its current consumers, while the launched campaign's saved redirect remains its own. Duplicate a page when you need an independent content version. A page referenced by an unfinished campaign cannot be deleted.

For a hosted explanation after submission, create a [learning page](learning-pages.md) and select it as the campaign destination. Learning pages are separate static resources and do not contain submission collectors.
