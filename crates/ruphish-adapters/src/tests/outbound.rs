#![allow(
    clippy::unwrap_used,
    reason = "Independent destination-policy fixtures"
)]
use super::*;
use crate::domain_verification::{Config, DnsHttps};

#[tokio::test]
async fn public_ipv4_and_ipv6_are_allowed_without_weakening_special_address_restrictions() {
    let smtp = Policy::default();
    let verifier = DnsHttps::new(Config::default()).unwrap();
    for address in [
        "1.1.1.1",
        "8.8.8.8",
        "2a00:1450:4001:c21::6c",
        "2606:4700:4700::1111",
        "::ffff:8.8.8.8",
    ] {
        let ip = address.parse().unwrap();
        assert!(smtp.permits(ip), "SMTP must allow {address}");
        assert!(verifier.permits(ip), "verification must allow {address}");
    }
    for address in [
        "0.0.0.0",
        "10.0.0.1",
        "100.64.0.1",
        "127.0.0.1",
        "169.254.169.254",
        "172.16.0.1",
        "192.168.0.1",
        "192.0.0.1",
        "192.0.2.1",
        "198.18.0.1",
        "198.51.100.1",
        "203.0.113.1",
        "224.0.0.1",
        "255.255.255.255",
        "::",
        "::1",
        "fc00::1",
        "fd00::1",
        "fe80::1",
        "ff02::1",
        "2001:db8::1",
        "2001::1",
        "2002:7f00:1::",
        "3fff::1",
        "64:ff9b::7f00:1",
        "100::1",
        "::ffff:127.0.0.1",
        "::ffff:10.0.0.1",
        "::ffff:169.254.169.254",
        "::ffff:192.0.2.1",
    ] {
        let ip = address.parse().unwrap();
        assert!(!smtp.permits(ip), "SMTP must deny {address}");
        assert!(!verifier.permits(ip), "verification must deny {address}");
    }
}

#[tokio::test]
async fn internal_exceptions_remain_scoped_to_the_configured_consumer() {
    let smtp =
        Policy::new(&["10.20.0.0/16".into(), "::1".into(), "169.254.0.0/16".into()]).unwrap();
    let verifier = DnsHttps::new(Config {
        allowed_networks: vec!["169.254.0.0/16".parse().unwrap()],
        ..Default::default()
    })
    .unwrap();
    assert!(smtp.permits("::ffff:10.20.1.2".parse().unwrap()));
    assert!(!smtp.permits("10.21.1.2".parse().unwrap()));
    assert!(smtp.permits("::1".parse().unwrap()));
    assert!(!verifier.permits("10.20.1.2".parse().unwrap()));
    assert!(smtp.permits("169.254.1.2".parse().unwrap()));
    assert!(!verifier.permits("169.254.1.2".parse().unwrap()));
}

#[test]
fn resolved_answers_are_validated_as_a_whole_and_bounded_before_connecting() {
    let policy = Policy::default();
    let gmail: SocketAddr = "[2a00:1450:4001:c21::6c]:587".parse().unwrap();
    let public: SocketAddr = "8.8.8.8:587".parse().unwrap();
    for addresses in [
        vec![gmail, public],
        vec![public, gmail],
        vec![gmail],
        vec![public],
    ] {
        assert_eq!(
            policy.check_addresses(addresses.clone()).unwrap(),
            addresses
        );
    }
    assert_eq!(
        policy.check_addresses([gmail, gmail, public]).unwrap(),
        vec![gmail, public]
    );
    for denied in ["127.0.0.1:587", "[::ffff:127.0.0.1]:587", "[fd00::1]:587"] {
        let denied = denied.parse().unwrap();
        for mixed in [
            vec![gmail, denied, public],
            vec![denied, public, gmail],
            vec![public, gmail, denied],
        ] {
            assert!(matches!(
                policy.check_addresses(mixed),
                Err(ResolveFailure::Denied)
            ));
        }
    }
    assert!(matches!(
        policy.check_addresses([]),
        Err(ResolveFailure::Unavailable)
    ));
    let many = (1..=17).map(|last| SocketAddr::from(([8, 8, 8, last], 587)));
    assert!(matches!(
        policy.check_addresses(many),
        Err(ResolveFailure::TooMany)
    ));
}
