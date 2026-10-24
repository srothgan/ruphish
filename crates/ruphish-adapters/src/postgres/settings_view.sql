SELECT d.id,
        CASE
            WHEN (NOT s.verification_enabled) THEN '9223372036854775807'::bigint
            WHEN (d.verified_until <= 0) THEN (0)::bigint
            WHEN (NOT s.verification_expires) THEN '9223372036854775807'::bigint
            ELSE (d.checked_at + s.verification_validity_seconds)
        END AS eligible_until,
    ((NOT s.verification_enabled) OR (d.verified_until > 0)) AS serves,
    s.verification_enabled AS verification_required,
        CASE
            WHEN (s.verification_enabled AND s.verification_expires AND (d.verified_until > 0)) THEN (d.checked_at + s.verification_validity_seconds)
            ELSE NULL::bigint
        END AS expires_at
   FROM (ruphish_domains d
     CROSS JOIN ruphish_settings s)
  WHERE (s.id = 1);
