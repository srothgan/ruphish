---
title: Localization
description: Update translations and preserve independent language, formatting, and time-zone preferences.
sidebar:
  order: 260
---

Ruphish currently provides English and German interface text. Translation catalogs live in `crates/ruphish-ui/locales/en.json` and `de.json`; the UI build uses `leptos_i18n` to generate checked keys. English is the default and fallback.

## Add or change text

1. Find the feature's existing keys and shared terminology. Add equivalent entries to both catalogs instead of embedding a literal in a view.
2. Preserve interpolation names and the intent of action, field, help, and error text. Translate the task meaning rather than copying English word order.
3. Use the existing localization context and helpers: `tr!` for reactive view text, `trs!` for shared control labels, and `t_string!` with a captured context in callbacks or reactive computations.
4. Run the locale/functional checks and affected WASM checks from [validation](../testing/validation-commands.md). Review the changed screens in both languages, including longer labels and wrapped controls.

Backend error codes remain stable machine identifiers. Translate them in the response handler instead of localizing server contracts. Already displayed error strings do not automatically change when the interface language changes.

Editor text uses `crates/ruphish-ui/web/editor/localization.mjs`; update it when changing editor-specific labels. Run the focused translation check from the repository root:

```sh
node --test crates/ruphish-ui/web/tests/locales.test.mjs
```

Then build the interface and select the relevant WASM checks. A JSON syntax check alone cannot detect every missing generated key or presentation issue.

## Add a supported language

Adding a catalog alone does not enable a language. Update the `leptos_i18n` configuration in `crates/ruphish-ui/build.rs`, the domain language inventory and preference validation, the language picker, and editor translations together. Match every key and interpolation used by existing catalogs. Review fallback behavior and unsupported browser locale handling before adding the language to published documentation.

Run the UI build and the locale, preference, and browser checks affected by these changes. Use [validation](../testing/validation-commands.md#frontend-and-browser-code) for commands. Verify the picker, saved preference, reset-to-browser behavior, errors, and long labels in the actual interface.

## Keep three preferences independent

| Preference | Controls | Example |
| --- | --- | --- |
| Interface language | Navigation, labels, help, and translated errors | German |
| Formatting locale | Numeric date order, separators, and hour cycle | `zh-CN` |
| IANA time zone | Conversion from stored instants to civil time | `Europe/Berlin` |

Account preferences also include `show_utc_offset`. A null override means follow the current browser default, not a saved snapshot of browser detection. Preferences are persisted by account and revalidated through browser session lifecycle. Container/server locale does not choose browser presentation.

`/auth/session` supplies browser state; `GET`/`PUT /internal/preferences` read or replace self-service preferences. These require the browser session/CSRF boundary and reject API-key access and impersonated preference changes. They are not public integration endpoints. Saving preferences does not change authentication generations.

## Dates and schedules

Use domain date/time conversion and the shared browser formatter. Preserve exact UTC instants when the user leaves a filter unchanged. A display-preference change must not reschedule an existing campaign. New local-time input follows the shared daylight-saving resolution rules; do not add a different parser in one feature.

The formatting picker uses the vendored CLDR locale inventory filtered by browser `Intl` support and domain validation. Picker examples, preview, and application timestamps should use the same formatting path. Test language changes, browser-default resets, invalid/absent browser zones, repeated local times, and long localized labels where affected.
