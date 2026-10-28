---
title: Correct recipient events
description: Add, edit and delete individual recipient events.
sidebar:
  order: 6
---

Expand a recipient in campaign **Sending** to review the chronological result history alongside delivery attempts. Both columns are visible immediately and stack on narrow screens. Select **Edit events** to open the editing dialog. Editing works in ongoing and completed campaigns; your account needs permission to modify campaign objects.

- Select **Edit** to change one event's occurrence time, type or submission-presence indicators, then **Save changes**. Time input uses the displayed zone.
- Select **Delete**, review the explanation, and confirm to permanently remove that event.
- Select **Add event**, supply its time and type, and save to create a **Manually recorded** entry.

Five clicks appear as five individual events. Deleting one leaves four clicks and one recipient with a click. Deleting the last direct click removes that direct outcome; a valid submission can still imply the clicked stage in progression statistics. Automatic recording continues normally, so later observations contribute independently.

**Origin** identifies how the event was created. **Manually recorded** means someone added it manually. **Edited** means someone subsequently changed its values. A newly added operator event is not marked edited; saving unchanged values does not mark an event edited either. An edited event keeps its origin and remains marked edited after further changes.

Summaries, charts, timelines and exports use the current event values. Edits replace those values and deletions remove the row; previous values and deleted events cannot be restored. SMTP attempts, delivery acceptance and campaign lifecycle records keep their dedicated operational controls. Editing events never resends messages, reopens completed campaigns or enqueues participant-event webhooks.

Only presence indicators are editable for submissions: **Entered**, **No entry detected**, or **Not recorded**. Ruphish does not accept entered usernames, passwords, OTPs, other participant field values or their derivatives through these controls.

Refreshes do not replace an active edit. Validation and conflict failures keep your input. If another operator changed the event, cancel and reopen it to review the current values. If the save outcome is unknown, keep the editor open and use **Retry**. An addition retry returns the existing event while it remains unchanged; a delete retry can report that the event no longer exists.

New events default to the current time; choose a historical occurrence when correcting a completed campaign. Load earlier history inside the dialog to reach older events. Closing with an active form asks whether to keep editing or discard the draft and close without saving. Refreshes preserve the open recipient row and dialog input.
