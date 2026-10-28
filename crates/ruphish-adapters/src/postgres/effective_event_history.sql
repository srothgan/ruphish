SELECT id,
    campaign_id,
    email,
    effective_time AS "time",
    effective_message AS message,
    effective_details AS details,
    recipient_id,
    source,
    revision
   FROM ruphish_event_records
  WHERE (excluded = 0);
