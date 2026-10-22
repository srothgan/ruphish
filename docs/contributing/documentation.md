---
title: Write and maintain documentation
description: Write useful reader-focused pages, maintain source-backed contracts, and prepare content for Starlight.
sidebar:
  order: 8
---

Start with the reader and the task, inspect the implementation, and write what the evidence supports. A polished page is not proof that its commands or claims work.

## Choose the canonical home

| Content | Home |
| --- | --- |
| Orientation and a guided local exercise | `getting-started/` |
| Campaign tasks, concepts, and result interpretation | `user-guide/` |
| Configuration, permissions, operation, and recovery | `administration/` |
| Public HTTP contracts and client examples | `api/` |
| Compatibility, upgrades, and rollback | `migration/` |
| Development, testing, translations, and documentation | `contributing/` |

Use [Diátaxis](https://diataxis.fr/) to distinguish tutorials, task guides, reference, and explanations within these sections. It is a writing framework, not a requirement for four more top-level directories. Give each detailed fact one canonical home and link to it elsewhere.

README introduces the product; `CONTRIBUTING.md` provides the conventional contribution entry point. Licenses and third-party notices live at the product root. Internal investigations, plans, proposals, and per-run evidence stay outside this public tree.

## Write for the reader

Use direct second-person instructions, concrete verbs, sentence-case headings, and consistent terminology. Put prerequisites and consequences before the dependent step. Give task guides an expected result and a way to check it. Connect troubleshooting symptoms to diagnosis, fix, and verification.

Keep each paragraph and list item on one source line. Use lists for real steps or parallel items and tables for comparisons. Link with descriptive text and give meaningful images alternative text. Do not fill an empty page just to complete a directory tree.

Distinguish application accounts from recipients, delivery from interaction, and observed events from inferred metrics. Translate UI labels while preserving API paths, wire fields, template variables, and error codes exactly.

## Check facts and examples

Trace behavior through the UI/API entry point, domain/application rules, persistence, external effects, and consumers. Cover failure, retry, restart, ownership, missing values, and boundaries where they affect the reader. Upstream Gophish examples and old docs are leads to verify, not proof of Ruphish behavior.

Use synthetic data and controlled services. State whether examples were executed, checked against source, or left unverified. Keep JSON valid and explain permissions, IDs, origins, and trust prerequisites before a write. Never publish credentials, participant input, or raw fixture state.

Use curl for HTTP request examples and environment variables for values readers replace: `RUPHISH_URL`, `RUPHISH_API_KEY`, optional `RUPHISH_CA_FILE`, and resource IDs such as `GROUP_ID` or `CAMPAIGN_ID`. Show clearly marked dummy values. Use `RUPHISH_CONFIG` when passing a configurable path to `--config`. These are shell-example conventions; do not imply the application reads them automatically or interpolates variables in JSON. Keep fixed protocol paths and wire fields literal, and use language-neutral pseudocode for algorithms such as webhook signature verification.

The API reference is maintained from source; it is not generated OpenAPI. Update affected handlers, shared wire types, tests, and reference pages together. If a generator is added, keep its schema/source authoritative instead of maintaining duplicate handwritten contracts.

## Starlight content contract

Use plain Markdown with `title` and `description` frontmatter:

```yaml
---
title: A task-oriented page title
description: One sentence explaining what the reader will learn or do.
---
```

The title supplies the page heading; begin body headings at `##`. Use lowercase, hyphenated paths and section `index.md` pages. `sidebar.order` can supply reading order. Avoid MDX component imports unless the site explicitly supplies them.

The website integration reads this tree as canonical content and mounts it under `/docs/`, with landing/about pages separate. It creates disposable Starlight input under the website's content directory. Edit these source pages, not the generated copy. Website implementation and deployment are separate from documentation authoring.

Relative `.md` links work in repository browsing. The site importer resolves them to published routes, preserves fragments, and maps links outside this tree to repository files. Images and other documentation assets live under `docs/assets/` and use relative links. Generated web links still need a site build check; frontmatter validation alone does not establish working web routes.

The importer excludes `old/**`. It is a temporary untouched archive, not public content. Sidebar hiding alone does not prevent routing or search indexing. For an unfinished new page, Starlight's `draft: true` excludes production publication; keep sidebar entries and links consistent with that exclusion.

Consult Starlight's [authoring guide](https://starlight.astro.build/guides/authoring-content/) and [frontmatter reference](https://starlight.astro.build/reference/frontmatter/) when adding the site. Verify generated links, assets, navigation, and search in that integration.

## Review and verify

Check local links/fragments, frontmatter, fences, example syntax, and terminology. Walk representative reader journeys: setup, first campaign, a failed send, export, API request, migration rehearsal, and contributor setup. Execute safe examples against controlled fixtures where practical and record what was not run.

Update section indexes, incoming links, and related audiences when a feature changes or a page moves. Preserve attribution and license notices for adapted material. Follow [validation](testing/validation.md); prose-only edits do not require a product build.
