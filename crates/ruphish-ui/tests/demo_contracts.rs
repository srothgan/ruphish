//! The demo dispatcher must satisfy real consumers, not a second set of invented schemas.
#![cfg(all(feature = "demo", not(target_arch = "wasm32")))]
#![allow(clippy::expect_used, reason = "Test setup and contract assertions")]

use ruphish_contracts::{
    auth, campaign_reads, campaign_workflow, domains, integrations, overview, resource_details,
    resources, settings, starter_templates,
};
use serde::de::DeserializeOwned;
use serde_json::Value;

fn decode<T: DeserializeOwned>(values: &Value, path: &str) {
    let result = serde_json::from_value::<T>(values[path].clone());
    assert!(
        result.is_ok(),
        "{path}: {}",
        result.err().expect("decode failure")
    );
}

#[test]
fn demo_responses_decode_through_the_existing_contracts() {
    let script =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../demo/contract-samples.mjs");
    let output = std::process::Command::new("node")
        .arg(script)
        .output()
        .expect("Node is required for demo contract tests");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let values: Value = serde_json::from_slice(&output.stdout).expect("synthetic responses");
    // Follow the initial format through demo CSV review into the real creation draft.
    use ruphish_ui::import_state::{ImportError, initial_format, prepare_creation};
    let reviews = values["csv_reviews"]
        .as_array()
        .expect("CSV workflow responses");
    let imported = prepare_creation("groups", initial_format("groups"), reviews[0].clone())
        .expect("downloaded template populates the default group import");
    assert_eq!(imported.fields["targets"][0]["email"], "alex@example.test");
    assert_eq!(
        imported.fields["targets"]
            .as_array()
            .expect("targets")
            .len(),
        2
    );
    assert!(imported.review.ready());
    let mut rejected = prepare_creation("groups", initial_format("groups"), reviews[2].clone())
        .expect("partial import remains reviewable");
    assert_eq!(
        rejected.fields["targets"]
            .as_array()
            .expect("unique targets")
            .len(),
        1
    );
    assert_eq!(rejected.review.rejected_rows.len(), 2);
    assert!(!rejected.review.ready());
    rejected.review.acknowledged = true;
    assert!(rejected.review.ready());
    assert!(matches!(
        prepare_creation("groups", initial_format("groups"), reviews[4].clone()),
        Err(ImportError::NoValidRecipients)
    ));
    for family in ["templates", "pages", "learning-pages", "smtp"] {
        prepare_creation(
            family,
            initial_format(family),
            values[format!("/api/{family}/1")].clone(),
        )
        .expect("other resource imports still start with resource JSON");
        assert!(prepare_creation(family, initial_format(family), reviews[0].clone()).is_err());
    }
    decode::<auth::BrowserSession>(&values, "/auth/session");
    decode::<overview::Snapshot>(&values, "/api/overview");
    decode::<overview::PostureResponse>(&values, "/api/overview/posture?window=30d");
    decode::<campaign_reads::Directory>(&values, "/api/campaigns/directory");
    decode::<campaign_reads::Delivery>(&values, "/api/campaigns/3/delivery");
    decode::<campaign_reads::Results>(&values, "/api/campaigns/3/results");
    decode::<campaign_reads::History>(&values, "/api/campaigns/3/recipients/30001/history");
    decode::<campaign_reads::Draft>(
        &values,
        "/api/campaign-drafts/00000000-0000-4000-8000-000000000001",
    );
    decode::<campaign_workflow::Review>(&values, "review");
    decode::<settings::Settings>(&values, "/api/settings");
    decode::<settings::ClosureResult>(&values, "closure");
    decode::<Vec<auth::UserSummary>>(&values, "/api/users/");
    decode::<auth::User>(&values, "/api/users/1");
    decode::<Vec<integrations::Webhook>>(&values, "/api/webhooks/");
    decode::<Vec<integrations::Mailbox>>(&values, "/api/reporting-mailboxes/");
    decode::<Vec<domains::Domain>>(&values, "/api/domains/");
    decode::<Vec<starter_templates::Summary>>(&values, "/api/starter-templates/");
    decode::<resources::GroupSummaries>(&values, "/api/groups/summary");
    for (family, kind) in [
        ("groups", resource_details::Kind::Group),
        ("templates", resource_details::Kind::Template),
        ("pages", resource_details::Kind::Page),
        ("learning-pages", resource_details::Kind::LearningPage),
        ("smtp", resource_details::Kind::Profile),
    ] {
        if family != "groups" {
            decode::<resources::ResourceSummaries>(&values, &format!("/api/{family}/summary"));
        }
        resource_details::decode(kind, values[format!("/api/{family}/1")].clone())
            .expect("complete resource detail");
    }
}
