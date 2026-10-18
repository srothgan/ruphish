//! Existing authentication guarantees and independently captured Go bcrypt vectors.

use ruphish_adapters::password::{self, ChangeError};
use ruphish_domain::password::{PasswordError, validate};
use serde_json::Value;

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
            assert!(password::verify(candidate, case["hash"].as_str().unwrap()).unwrap());
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
    assert!(password::verify(case["plaintext"].as_str().unwrap(), hash).unwrap());
    assert!(!password::verify("wrong password", hash).unwrap());
    assert!(!password::verify("", hash).unwrap());
    assert!(password::verify("password", "not a hash").is_err());
}

#[test]
fn password_change_preserves_original_rejections_and_verifies_the_new_hash() {
    let current_hash = bcrypt::hash("current password", 4).unwrap();
    assert!(matches!(
        password::change(&current_hash, "valid password", "invalid"),
        Err(ChangeError::Policy(PasswordError::Mismatch))
    ));
    assert!(matches!(
        password::change(&current_hash, "current password", "current password"),
        Err(ChangeError::Policy(PasswordError::Reused))
    ));
    assert!(matches!(
        password::change(&current_hash, "short", "different"),
        Err(ChangeError::Policy(PasswordError::TooShort))
    ));
    let replacement = password::change(&current_hash, "valid password", "valid password").unwrap();
    assert!(password::verify("valid password", &replacement).unwrap());
    assert!(!password::verify("current password", &replacement).unwrap());
    let second = password::change(&current_hash, "valid password", "valid password").unwrap();
    assert_ne!(replacement, second, "new hashes need independent salts");
    let boundary = "x".repeat(72);
    assert!(password::change(&current_hash, &boundary, &boundary).is_ok());
    let too_long = "x".repeat(73);
    assert!(matches!(
        password::change(&current_hash, &too_long, &too_long),
        Err(ChangeError::TooLong)
    ));
}
