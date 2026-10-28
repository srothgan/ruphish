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

## Form validation and recovery

- Start a campaign with empty fields. Continue must stay on the incomplete step, show localized field feedback and move keyboard focus to the first error. Correct each step and confirm the draft is saved before the next step appears.
- Save an incomplete draft, reload it, and attempt Review. Missing audience/content or scheduled time must lead to the relevant editing step without a broken review page. Choose immediate delivery explicitly, then compare with a scheduled launch requiring a valid date and time zone.
- Delete a selected resource from another session before review, then correct the selection. Repeat with a schedule that has passed and with a launch response interrupted after submission; the latter must retain the existing retry action.
- Submit invalid resource content, including a recipient row, template expression and attachment. Confirm errors point to editable controls or content anchors and that the saved resource remains intact. Correct and save; then test a stale edit from another session.
- Check password confirmation, short and oversized passwords, blank password on a user edit, and blank secret on an integration edit. Confirm diagnostics never echo credentials and that a rejected change preserves the existing credential.
- Check settings duration ranges, invalid domain origins and preference values. Correct an error while a request is delayed; an obsolete field error must not replace the new state. Distinguish operational failures from field errors.
- Repeat in English and German using keyboard navigation and narrow layouts. Inspect labels, associated error descriptions, focus and hidden editor panels; functional Rust tests do not establish these browser behaviors.

## Participant-browser checks

Keep the admin campaign view open alongside DevTools Network on the public page. Use invented canaries such as `LOCAL-USERNAME-CANARY` and `LOCAL-PASSWORD-CANARY`; never use real login information. Create a fresh campaign with `seed` when a case requires a clean recipient/session.

1. Open the browser campaign email in Mailpit. Allow its external image if prompted. Confirm one `Email Opened` event only when the tracking image is fetched; opening Mailpit's message view alone is not evidence of a fetch. Follow the practice link and confirm the greeting is personalized and `Clicked Link` appears for the same recipient.
2. Inspect the public document's CSP and DOM. The header must include `sandbox allow-scripts allow-forms` without `allow-same-origin`, plus `form-action 'none'`. The fixture's authored script remains and sets `document.body.dataset.authoredScriptExecuted`. Names, IDs, autocomplete and validation should remain usable. `allow-forms` enables validation/submit events, while `form-action` blocks native network submissions.
3. On a form whose password is optional, submit with the password empty, then submit again with a synthetic password. Each acknowledged submission must create a separate event. Campaign/dashboard counts stay at one recipient per category; recipient details show both events. The built-in request body must contain only `event_id` and boolean `password_present`. Repeat with both fields required: an empty required field must prevent submission and create no event. Distinguish independent authored requests from collector requests.
4. Test a password manager with a synthetic saved entry and the external password control below the form. Confirm filling only that external control still reports presence. Repeat with a prefilled/read-only control and a revealed password. Disabled or unnamed controls must not report presence. Verify disabled-fieldset controls and the first-legend exception. In the page editor, confirm disabled controls receive advisory warnings and can still be saved. Check the network body again; do not infer privacy from the UI text.
5. Block the public submission request in DevTools, submit, then unblock and use **Retry this submission**. The page must show an unconfirmed/error state, recover, and reuse the original event ID. For a true timeout, use a request-stalling proxy; DevTools Offline usually produces immediate failure, which does not prove the 15-second deadline. Timeout/retry logic also has isolated functional tests.
6. Disable JavaScript and reload the public page. Attempt submission: no form data may leave through native navigation. Re-enable JavaScript afterward. Check console CSP failures as well as Network; an error alone does not prove that no data was sent.
7. Disable the domain in admin, then try another page load/submission. It must fail without new events. Re-enable and verify as needed; complete the campaign and confirm later submissions likewise fail. Use a fresh campaign for subsequent cases.
8. Inspect campaign responses/history and `tests/acceptance/data/ruphish.log` for your synthetic canaries. The automated test additionally scans database/WAL bytes for its own rejected-payload canary. Record browser/version, case, expected/actual result and any failure; do not attach real credentials or unredacted admin tokens.

9. In an authenticated administrator session, open a functional landing preview with synthetic scripts that attempt to read `parent.document`, `document.cookie` and origin storage, navigate the admin window, open a popup and embed another frame. These capabilities must remain unavailable. Verify an attempted credentialed admin API read cannot expose its response and an attempted write cannot mutate application state. Repeat after the preview navigates within its own frame. Keep probes local and never print credentials. Browser enforcement requires this manual check; JSDOM does not implement the sandbox.
10. Exercise custom validation, `type="button"` intermediate steps, dynamic fields and final `window.ruphish.submit(form)`. Cancelled submit events must not be tracked; a final accepted event must use the same presence-only body and retry semantics. In editor/starter/test-email previews, scripts run but built-in submissions show preview feedback and create no events or campaign redirects. Source editing and saved-source overview remain inert.
11. Import a ZIP with root `index.html`, a script, images and nested CSS/font dependencies. Review and save; verify source preservation after editing, duplication, export/import and reload. Check external resources with controlled endpoints and appropriate CORS, then test missing dependencies and unavailable remote assets. No import may silently substitute unrelated content or claim missing assets were packaged.

## Interactive starter templates

- Confirm the catalog offers 25 landing pages and 23 emails, including six new interactive pages and three matching document invitations. Existing entries must remain independently selectable.
- Preview each new page at desktop and narrow widths. Check logo placement, readable file cards, labels, keyboard focus, native required-field feedback and the Microsoft side-panel layout. Compare the intended compositions with the [reference notes](../../../crates/ruphish-http/starter-templates/interactive-references.md); no automated test establishes visual fidelity.
- Open each document card, continue from email to password where applicable, reveal the synthetic password and go back. The password must clear while the email remains. Use Enter in both steps and confirm it activates the current action. Repeat with recipient-email prefill and supported company-logo options, then add a copy and reopen its editor preview.
- Verify intermediate actions create no submission requests. Final submission must show preview feedback in previews and send only the collector's presence-only payload in a synthetic campaign. Repeated clicks must not create duplicate submissions. Interrupt final delivery and recover with **Retry this submission**; close and reopen a completed preview to restart.
- Preview the three new invitation emails in HTML and plain text. Confirm sender and filename match the corresponding landing card and the campaign link opens that page. Edit both independent copies when customizing those details.

## Record the boundary

A passing direct HTTP test cannot prove browser sandbox enforcement or that a browser sent no entered values. Inspect built-in collector requests separately from authored code and configured reverse-proxy behavior. Arbitrary authored code can read or transmit input; its requests are outside the collector guarantee. Retain synthetic evidence without publishing credentials, tokens or fixture secrets.
