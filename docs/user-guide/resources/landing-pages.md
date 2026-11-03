---
title: Landing pages
description: Create the pages recipients visit and check their forms, assets, and submission behavior.
sidebar:
  order: 14
---

A landing page is the page recipients see after following a campaign link. It can show a message, present a form, or guide someone through several steps.

Ruphish records page visits and supported form submissions. Its built-in tracking records whether a password was entered, without collecting the entered value. Read [Participant data](../results/participant-data.md) before choosing or importing page content.

## Create a page

1. Open **Resources → Landing pages** and choose **Create**.
2. Enter a name.
3. Write the HTML in the source or visual editor, or start with [starter content](../add-starter-templates.md).
4. Use **Check landing page** to inspect the current content and address the relevant warnings.
5. Open **Preview** to check layout and interactions.
6. Choose **Create**.

Apply visual-editor changes before saving. Page checks inspect your current draft; they do not save it. You can use [personalization expressions](../personalize-content.md) to include recipient details.

## Import existing content

Choose **Import** from the landing-page list, then use the format that matches your source:

| Source | What to provide |
| --- | --- |
| HTML document | The page's HTML file or source text. |
| Website URL | A reachable page URL. Ruphish fetches its HTML; external assets remain external. |
| Resource JSON | A previously exported Ruphish landing page. |
| ZIP file | A page with `index.html` at the archive root and the local assets it references. |

Review the imported draft and choose **Create** to save it. A website import can fail if the destination is blocked by the installation's outbound rules or its certificate is invalid.

For a ZIP import, include relative images, stylesheets, fonts, and classic JavaScript files. Referenced local assets are embedded in the saved page, so they stay with it when you duplicate or export it. Unreferenced files are discarded.

The ZIP, expanded contents, and resulting page each have an 8 MiB limit, with at most 256 archive entries. Missing assets, unsafe paths, and cyclic stylesheet imports reject the import. Local JavaScript modules and dynamically constructed asset requests need to be bundled or rewritten before import. External URLs remain external.

## Add a form

Use an ordinary HTML form with labeled controls and a submit button. For example:

```html
<form>
  <label>
    Email
    <input name="email" type="email" required>
  </label>
  <label>
    Password
    <input name="password" type="password" required>
  </label>
  <button type="submit">Continue</button>
</form>
```

Ruphish supplies the submission tracking. Do not add an action that sends the form values elsewhere. Browser validation runs first; typing in a field or clicking an ordinary button does not itself record a submission.

A submission records password presence when a named, enabled password field belonging to that form is nonempty. This does not check whether the password is correct. Usernames and other entered values are not collected.

For a multi-step page, use `type="button"` for intermediate buttons and a submit button for the final action.

## Handle custom interactions

If your JavaScript cancels the normal submit event for extra validation or a multi-step flow, call `await window.ruphish.submit(form)` once when the final submission is ready. It uses the built-in validation, feedback, and redirect behavior.

```html
<script>
  const form = document.querySelector('form');
  form.addEventListener('submit', async (event) => {
    event.preventDefault();
    await window.ruphish.submit(form);
  });
</script>
```

Use this handler as the final submission path, not in addition to an uncancelled normal submission. The call resolves to `true` when the live event is acknowledged; previews and unconfirmed or invalid submissions return `false`. After an uncertain result, use the provided **Retry this submission** control rather than calling the API again and creating a separate observation.

For ordinary programmatic form submission, use `requestSubmit()`; `form.submit()` bypasses submit events.

## Choose what happens afterward

Set an optional redirect URL on the landing page to suggest a destination for new campaigns. During campaign creation, review that choice: select a [learning page](learning-pages.md), enter a custom HTTP(S) or relative URL, or leave the custom destination empty to stay on the landing page.

The redirect follows a confirmed submission. The launched campaign keeps the destination selected in its review.

## Check the page recipients will see

Preview supports checking the page without creating campaign events. Use synthetic details to try validation, intermediate steps, and the final submit action. Then use a controlled campaign to check the complete link and redirect path.

Authored scripts, styles, images, and fonts are supported. Embedded frames and objects, native form destinations, and automatic HTML refresh redirects are restricted. Preview and live content run separately from the Ruphish application.

Custom scripts and external services can still read or transmit input independently of Ruphish's collector. Review imported code and external requests before using the page.

## Change or delete a page

Editing affects future campaigns. Launched campaigns retain their saved page, including those scheduled for later. Choose **Duplicate** for an independent variation.

A page cannot be deleted while an unfinished campaign references it.
