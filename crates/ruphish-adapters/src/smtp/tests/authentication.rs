use super::*;

#[test]
fn rfc_2195_challenge_and_invalid_inputs() {
    // RFC 2195 section 2: independent published challenge/response.
    assert_eq!(
        cram_response(
            "tim",
            "tanstaaftanstaaf",
            "PDE4OTYuNjk3MTcwOTUyQHBvc3RvZmZpY2UucmVzdG9uLm1jaS5uZXQ+"
        )
        .unwrap(),
        "dGltIGI5MTNhNjAyYzdlZGE3YTQ5NWI0ZTZlNzMzNGQzODkw"
    );
    for challenge in ["", "***", "YQ=", "YQ==\r\nMAIL FROM:bad"] {
        assert!(cram_response("tim", "secret", challenge).is_err());
    }
    assert!(cram_response("two words", "secret", "YQ==").is_err());
    assert!(cram_response("tim", "secret", &"A".repeat(4097)).is_err());
}
