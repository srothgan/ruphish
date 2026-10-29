//! Remove only the new performance migration when reconstructing historical fixtures.
pub fn before_indexes(provider: &str) -> &'static str {
    match provider {
        "sqlite" => "DROP INDEX ruphish_delivery_fairness; DROP INDEX ruphish_mailbox_runs_unfinished; DROP INDEX ruphish_webhook_attempts_unfinished; DELETE FROM ruphish_schema WHERE version>=14;",
        "mysql" => "DROP INDEX ruphish_delivery_fairness ON ruphish_delivery; DROP INDEX ruphish_mailbox_runs_unfinished ON ruphish_mailbox_runs; DROP INDEX ruphish_webhook_attempts_unfinished ON ruphish_webhook_attempts; DELETE FROM _sqlx_migrations WHERE version>=8;",
        "postgres" => "DROP INDEX ruphish_delivery_fairness; DROP INDEX ruphish_mailbox_runs_unfinished; DROP INDEX ruphish_webhook_attempts_unfinished; DELETE FROM _sqlx_migrations WHERE version>=8;",
        _ => panic!("unknown fixture provider"),
    }
}
