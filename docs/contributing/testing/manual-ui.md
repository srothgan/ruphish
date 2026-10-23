---
title: Manual UI checks
description: Review operator workflows and participant-browser behavior that compilation and protocol tests cannot establish.
sidebar:
  order: 5
---

The user performs browser appearance and interaction review. Use synthetic data in the [local fixture](local-fixture.md), and record browser/version, scenario, expected result, actual result, and unresolved failures.

## Admin workflows

- Check sign-in, required password change, account preferences, API-key reveal/copy, and sign-out with unsaved work.
- Create, edit, import, preview, and delete resources. Inspect empty, loading, failed, stale, and permission-limited states.
- Exercise source and visual editors, attachments, keyboard focus, dialogs, and content preservation between modes.
- Save a campaign draft, reload it, review recipients, schedule or launch it, inspect delivery, and resolve a controlled uncertain outcome.
- Filter analysis, inspect recipient timelines, export both formats, and compare displayed definitions with downloaded rows.
- Switch English/German, formatting locale, and time zone independently. Check persistence after another login and reset-to-browser behavior.
- Inspect narrow layouts, wrapped labels/actions, keyboard navigation, visible focus, reduced motion, and long localized strings.

## Participant-browser checks

Keep the admin campaign view open alongside DevTools Network on the public page. Use invented canaries such as `LOCAL-USERNAME-CANARY` and `LOCAL-PASSWORD-CANARY`; never use real login information. Create a fresh campaign with `seed` when a case requires a clean recipient/session.

1. Open the browser campaign email in Mailpit. Allow its external image if prompted. Confirm one `Email Opened` event only when the tracking image is fetched; opening Mailpit's message view alone is not evidence of a fetch. Follow the practice link and confirm the greeting is personalized and `Clicked Link` appears for the same recipient.
2. Inspect the public document's CSP and DOM. Native form submissions must be blocked by `form-action 'none'`; the authored script must be absent, and `document.body.dataset.authoredScriptExecuted` must be undefined. Ordinary names, IDs and autocomplete attributes should remain usable.
3. Submit with the password empty, then submit again on the same page with a synthetic password. Each submission must create a separate event. Campaign/dashboard summary counts stay at one recipient per category; recipient details show both events. The sole submission request body must contain `event_id` and boolean `password_present`, with no usernames, password values, field maps, lengths or hashes. The corresponding history must show only submission/presence metadata.
4. Test a password manager with a synthetic saved entry and the external password control below the form. Confirm filling only that external control still reports presence. Repeat with a prefilled/read-only control and a revealed password. Disabled or unnamed controls must not report presence. Verify disabled-fieldset controls and the first-legend exception. In the page editor, confirm disabled controls receive advisory warnings and can still be saved. Check the network body again; do not infer privacy from the UI text.
5. Block the public submission request in DevTools, submit, then unblock and use **Retry this submission**. The page must show an unconfirmed/error state, recover, and reuse the original event ID. For a true timeout, use a request-stalling proxy; DevTools Offline usually produces immediate failure, which does not prove the 15-second deadline. Timeout/retry logic also has isolated functional tests.
6. Disable JavaScript and reload the public page. Attempt submission: no form data may leave through native navigation. Re-enable JavaScript afterward. Check console CSP failures as well as Network; an error alone does not prove that no data was sent.
7. Disable the domain in admin, then try another page load/submission. It must fail without new events. Re-enable and verify as needed; complete the campaign and confirm later submissions likewise fail. Use a fresh campaign for subsequent cases.
8. Inspect campaign responses/history and `tests/acceptance/data/ruphish.log` for your synthetic canaries. The automated test additionally scans database/WAL bytes for its own rejected-payload canary. Record browser/version, case, expected/actual result and any failure; do not attach real credentials or unredacted admin tokens.

## Record the boundary

A passing direct HTTP test cannot prove that a browser sent no entered values. Inspect actual request bodies, blocked native submissions, and all applicable output channels before claiming end-to-end participant privacy. Retain synthetic evidence without publishing credentials, tokens, or fixture secrets.
