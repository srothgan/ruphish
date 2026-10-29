-- Restrict recovery to unfinished attempts/runs without changing durable state.
CREATE INDEX ruphish_mailbox_runs_unfinished ON ruphish_mailbox_runs(mailbox_id,finished);
CREATE INDEX ruphish_webhook_attempts_unfinished ON ruphish_webhook_attempts(job_id,finished);
