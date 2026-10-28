---
title: Correct recipient events
description: Add, edit, exclude and restore individual recipient observations with an audit history.
sidebar:
  order: 6
---

Expand a recipient in campaign **Delivery** to review the chronological result history alongside delivery attempts. Both columns are visible immediately and stack on narrow screens. Select **Edit events** to open the correction dialog. Corrections work in ongoing and completed campaigns; your account needs permission to modify campaign objects.

- Select **Edit** to change one event's occurrence time, supported type or submission-presence indicators, then **Save changes**. Time input uses the displayed zone.
- Select **Delete**, review the explanation, and save to exclude that one event from results. The row remains visible as **Excluded**.
- Select **Add event**, supply its time and type, and save to create an **Operator** entry.
- Select **Restore original** to restore the original values and include the event again. For an operator entry, this restores its initial entered values.

Five clicks appear as five individual events. Deleting one leaves four clicks and one recipient with a click. Deleting the last direct click removes that direct outcome; a valid submission can still imply the clicked stage in progression statistics. Automatic recording continues normally, so later observations contribute independently.

In the dialog, the **Origin** column distinguishes **System** and **Operator**. Editing a system observation keeps its origin and adds an **Edited** indicator. **View changes** appears for manually changed events and shows the original evidence and audited changes, including who made them and when. Older audit revisions can be loaded from the same table. Corrections never resend messages, reopen completed campaigns or emit participant-event webhooks.

Results summaries, charts and exports use effective events and corrected occurrence times. Excluded events remain inspectable in the timeline and editing dialog. SMTP attempts, delivery acceptance and campaign lifecycle records keep their dedicated operational controls.

Only presence indicators are editable for submissions: **Present**, **Absent**, or **Not recorded**. Ruphish does not accept entered usernames, passwords, OTPs, other participant field values or their derivatives through these controls.

Refreshes do not replace an active edit. Validation and revision-conflict failures keep your input. If another operator changed the event, inspect its current history before cancelling and reopening the edit. If the save outcome is unknown, keep the editor open and use **Retry**; it resends the same change identity to avoid duplicate additions.

Load earlier history inside the dialog to reach older events. Save or cancel an active edit before closing. Refreshes preserve the open recipient row and dialog input.
