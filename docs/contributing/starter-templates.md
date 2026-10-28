---
title: Maintain starter templates
description: Update the bundled catalog and its supported customization options.
---

The bundled catalog lives in `crates/ruphish-http/starter-templates/`. Each email includes HTML, a plain-text alternative and a subject. Keep asset attribution and license notices with the content.

When adding or updating an entry:

1. Review each email, landing page or learning page individually, including its preview and applicable form behavior.
2. Update its catalog metadata in `crates/ruphish-http/src/starter_templates.rs`.
3. Declare prefill and company-logo support only where the template supports them. Keep the [company-logo availability list](../user-guide/starter-templates.md#company-logo-availability) current.
4. Keep all required assets offline and preserve supported Go-template expressions.
5. Extend the relevant catalog and transformation tests, following [validation](testing/validation.md).

Emails, landing pages and learning pages are independent entries. Imported copies are ordinary resources with no catalog relationship, installed status or automatic updates.

The six interactive landing starters share `crates/ruphish-http/src/starter_templates/interaction.js`. Their catalog metadata declares this script, and `Source::entry` embeds it before the document's closing body tag so each copied resource contains its own controller. Keep exactly one closing `</body>` in these source documents. Raw `landing.html` files omit the controller; use the prepared catalog preview to exercise the complete page. Preserve the controller's data attributes and local intermediate steps when editing these layouts. The [interactive reference notes](../../crates/ruphish-http/starter-templates/interactive-references.md) document visual sources and simulation adaptations. `web/tests/interactive-starters.test.mjs` covers local navigation, required fields, final presence-only submission and retry; the executable catalog workflow checks script assembly, previews and independent resource copies.

For a learning page, add its static HTML under `crates/ruphish-http/starter-templates/learning-pages/<slug>/learning.html` and include it in the `CATALOG` array using `Source` with `kind: Kind::LearningPage`. Choose a catalog-wide unique `id`, a resource `name` and a searchable `brand` label; load `html` with `include_str!`, set `script: None`, leave `subject` and `text` empty, and declare `logo: Some((selector, height, context))` for a supported company-logo slot, or `None` when absent. The selector must identify exactly one image or container, and the height is the maximum displayed image height in CSS pixels. The source mapper in `src/starter_templates/source.rs` supplies the learning-page payload, exposes declared logo metadata and disables recipient-email prefill. Both page families use the shared logo upload and replacement logic; learning-page transformations preserve static text without parsing Go expressions. Resource and public IDs are assigned when a copy is created.

Learning pages use static rendering: recipient expressions remain literal, and forms, input controls and authored scripts are removed from previews and public responses. Use permitted embedded images and ordinary links. Verify authoring against the [learning-page content rules](../user-guide/learning-pages.md), including the actual restricted preview.

Landing and learning previews have separate resource endpoints but share `ruphish-adapters::landing::render` with public serving and test-send previews. Its explicit mode owns recipient expansion, document metadata, sanitization, diagnostics and collector insertion. Keep title text in the document head and preserve supported semantic containers. Email previews retain the email pipeline; page previews must not use it as a shortcut. The admin frame adds sandbox restrictions to the prepared HTML.

The three learning starters are independently authored documents with distinct layouts. Preserve their method-specific reading order, examples and recovery guidance when editing them. Each uses one `.learning-logo` container, with a maximum image height of 48 CSS pixels and a white background for uploaded logos. The embedded PNG is a 788-pixel-wide rasterization of the canonical `crates/ruphish-ui/web/branding/ruphish-wordmark.svg`; preserve the artwork’s attribution comment when updating the image. Keep images embedded because public learning pages cannot load admin assets or remote images. Content sources and artwork provenance are recorded in `crates/ruphish-http/starter-templates/learning-pages/README.md`.

The executable catalog workflow in `tests/starter_templates_process.rs` exercises bundled entries and an unpublished learning fixture through creation, preview, independent-copy edits, authorization and restart. The dedicated bundled-content case in `tests/learning_pages_process.rs` imports all three methods with their standard and company logos, serves them through `/learn/`, and checks that policy preserves their content and layout. `web/tests/learning-starters.test.mjs` exercises branding and rejected options on the actual documents. Keep synthetic test content in tests; catalog entries belong in the bundled content directory and the catalog array.
