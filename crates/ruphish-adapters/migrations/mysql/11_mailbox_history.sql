-- Full history needs ordering across finished and unfinished runs.
CREATE INDEX ruphish_mailbox_history ON ruphish_mailbox_runs(mailbox_id,id);
