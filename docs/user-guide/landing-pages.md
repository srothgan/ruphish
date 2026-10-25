---
title: Create landing pages
description: Author, import, preview, and configure pages used by campaign links.
sidebar:
  order: 7
---

A landing page is what a recipient sees after following the campaign link. Start with a page that works without authored JavaScript or external assets; the public representation deliberately restricts both.

Administrators can choose a ready-made, independent page from the [Starter Template Repository](starter-templates.md), with optional email prefill and company branding on supported layouts.

## Create or import

Open **Resources → Landing pages → Create**, enter a name, and author the HTML. You can use [template expressions](template-reference.md) for recipient personalization.

To reuse existing content, choose **Import** from the landing-page list. Provide HTML, resource JSON, or a website URL. Website imports validate certificates and apply the installation's outbound restrictions at every redirect. They preserve base-URL semantics; the accepted legacy `include_resources` option does not inline external resources.

Review the imported draft and save it explicitly. Saved authored HTML and the served page are distinct: the public representation applies content restrictions after template rendering. An import can therefore succeed while some of its styling, scripts, or remote resources are absent from the preview/public page.

## Forms and submission tracking

Every supported submission is recorded, together with password presence. A separate submission creates a separate event; retrying an unconfirmed delivery does not.

Ruphish never captures participant-entered values. The supported collector submits an event identity and a presence boolean; it does not submit named form values. Keep authored labels and controls usable, but do not rely on a custom `action`, external form endpoint, event handler, or script to process them. See [participant data](participant-data.md).

A redirect can be an HTTP(S) or relative destination, including supported template expressions. It runs after an acknowledged submission. Leave it empty to stay on the page. Do not place entered values into a redirect URL.

## Page checks, preview, and testing

Use **Check landing page** to inspect the current source. Saving, loading, previewing, and campaign review also return advisory warnings from the processed sample page. Checks flag disabled or unnamed password controls, missing form association, duplicate IDs, repaired HTML, removed authored execution, and forms with no usable submission control. Warnings do not prevent saving. Verify template-dependent forms with representative recipients.

Preview uses the restricted representation with submission tracking disabled. Scripts, remote resources, and submissions are blocked; preview does not create campaign results.

Before launch, inspect the page's layout, labels, keyboard access, and expected redirect using a synthetic local campaign. Embedded PNG/JPEG/GIF data images are supported. Publishing a file under `/static/` does not exempt it from the page's resource restrictions.

Editing a page changes its current consumers, while the launched campaign's saved redirect remains its own. Duplicate a page when you need an independent content version. A page referenced by an unfinished campaign cannot be deleted.

For a hosted explanation after submission, create a [learning page](learning-pages.md) and select it as the campaign destination. Learning pages are separate static resources and do not contain submission collectors.
