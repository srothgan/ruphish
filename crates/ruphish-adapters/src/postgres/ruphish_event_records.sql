SELECT h.id,
    h.campaign_id,
    h.email,
    h."time",
    h.message,
    h.details,
    h.recipient_id,
    h.source,
    COALESCE(a.revision, (0)::bigint) AS revision,
    COALESCE(a.excluded, (0)::bigint) AS excluded,
    COALESCE(a."time", (h."time")::text) AS effective_time,
    COALESCE(a.message, (h.message)::character varying) AS effective_message,
    COALESCE(a.details, h.details) AS effective_details
   FROM (event_history h
     LEFT JOIN ruphish_event_changes a ON (((a.event_id = h.id) AND (a.revision = ( SELECT max(latest.revision) AS max
           FROM ruphish_event_changes latest
          WHERE (latest.event_id = h.id))))));
