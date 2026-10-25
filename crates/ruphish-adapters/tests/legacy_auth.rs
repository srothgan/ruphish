//! Existing authentication guarantees and independently captured Go bcrypt vectors.

mod support;
use ruphish_adapters::password_worker as password;
use ruphish_domain::password::{PasswordError, validate};
use serde_json::Value;
use support::DatabaseFixture;

#[test]
fn password_policy_preserves_original_cases_and_captured_byte_lengths() {
    assert_eq!(validate("short"), Err(PasswordError::TooShort));
    assert_eq!(validate("valid password"), Ok(()));
    let cases: Vec<Value> = serde_json::from_str(include_str!(
        "../../../tests/fixtures/gophish-v0.12.1/password-policy.json"
    ))
    .unwrap();
    for case in cases {
        let result = validate(case["password"].as_str().unwrap());
        assert_eq!(
            result.err().map(|e| e.to_string()).unwrap_or_default(),
            case["expected_policy_error"].as_str().unwrap()
        );
    }
}

#[test]
fn extended_go_hashes_preserve_unicode_embedded_nul_and_72_byte_boundaries() {
    let cases: Vec<Value> = serde_json::from_str(include_str!(
        "../../../tests/fixtures/compatibility/passwords.json"
    ))
    .unwrap();
    for case in cases {
        let candidate = case["password"].as_str().unwrap();
        assert_eq!(
            validate(candidate).is_err(),
            case["policy_rejected"].as_bool().unwrap()
        );
        if !case["hash_rejected"].as_bool().unwrap() {
            assert!(password::verify(candidate, case["hash"].as_str().unwrap()));
        }
    }
}

#[test]
fn captured_go_hash_accepts_only_the_correct_password() {
    let case: Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/gophish-v0.12.1/password.json"
    ))
    .unwrap();
    let hash = case["bcrypt_hash"].as_str().unwrap();
    assert!(password::verify(case["plaintext"].as_str().unwrap(), hash));
    assert!(!password::verify("wrong password", hash));
    assert!(!password::verify("", hash));
    assert!(!password::verify("password", "not a hash"));
}

#[tokio::test]
async fn password_change_preserves_original_rejections_and_verifies_the_new_hash() {
    use ruphish_adapters::password_worker::PasswordWorker;
    use ruphish_application::auth::{Error, Service};
    use std::sync::Arc;
    let directory = tempfile::tempdir().unwrap();
    let database = DatabaseFixture::open(&directory.path().join("accounts.db"), "")
        .await
        .unwrap();
    database
        .bootstrap(Some("current password".into()), None)
        .await
        .unwrap();
    let pool = database.inspection_pool();
    let captured: Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/gophish-v0.12.1/password.json"
    ))
    .unwrap();
    sqlx::query("UPDATE users SET hash=?,password_change_required=0")
        .bind(captured["bcrypt_hash"].as_str().unwrap())
        .execute(&pool)
        .await
        .unwrap();
    let repo = database.accounts();
    let service = Service::new(repo.clone(), Arc::new(PasswordWorker::default()));
    let user = service
        .login("admin", "FixtureOnlyPassword42!".into())
        .await
        .unwrap();
    for (candidate, confirmation, expected) in [
        ("valid password", "invalid", PasswordError::Mismatch),
        (
            "FixtureOnlyPassword42!",
            "FixtureOnlyPassword42!",
            PasswordError::Reused,
        ),
        ("short", "different", PasswordError::TooShort),
    ] {
        let result = service
            .change_password(
                user.identity,
                Some("FixtureOnlyPassword42!".into()),
                candidate.into(),
                confirmation,
            )
            .await;
        let error = result.err().unwrap();
        assert_eq!(error.to_string(), expected.to_string());
        match expected {
            PasswordError::Reused => {
                assert!(matches!(error, Error::Password(PasswordError::Reused)))
            }
            _ => {
                let Error::Validation(report) = error else {
                    panic!("expected field validation")
                };
                let (field, code) = if expected == PasswordError::Mismatch {
                    (
                        "confirmation",
                        ruphish_domain::validation::Code::PasswordMismatch,
                    )
                } else {
                    ("password", ruphish_domain::validation::Code::PasswordShort)
                };
                assert_eq!(report.issues()[0].field, field);
                assert_eq!(report.issues()[0].code, code);
            }
        }
        assert_eq!(
            repo.by_id(user.identity.id).await.unwrap().hash,
            captured["bcrypt_hash"].as_str().unwrap()
        );
    }
    let long = "é".repeat(40);
    let replacement = service
        .change_password(
            user.identity,
            Some("FixtureOnlyPassword42!".into()),
            long.clone(),
            &long,
        )
        .await
        .unwrap();
    assert!(replacement.hash.starts_with("$argon2id$"));
    assert!(password::verify(&long, &replacement.hash));
    assert!(!password::verify(
        "FixtureOnlyPassword42!",
        &replacement.hash
    ));
    assert!(service.session(user.identity).await.is_err());
    let independently_salted = password::hash(&long).unwrap();
    assert_ne!(replacement.hash, independently_salted);
    assert!(password::verify(&long, &independently_salted));
    assert!(matches!(
        password::hash(&"x".repeat(1025)),
        Err(Error::TooLong)
    ));
    assert!(!password::verify(
        "password",
        "$argon2id$v=19$m=4294967295,t=2,p=1$c29tZXNhbHQ$YWJjZGVmZ2hpamtsbW5vcA"
    ));
    database.close().await;
}
