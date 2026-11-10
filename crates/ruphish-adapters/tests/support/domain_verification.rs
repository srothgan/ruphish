//! Real loopback DNS/TLS peers: no public DNS, external recipients or mocked verifier.
use super::{domain_network::Network, fixture};
use ruphish_adapters::domain_verification::{Config, DnsHttps};
use ruphish_application::domains::{Check, Observation, Verifier};
use std::{
    net::{IpAddr, TcpListener},
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

const HOST: &str = "training.example.test";

fn verifier(directory: &Path, dns: &Network, trusted: &Network) -> DnsHttps {
    let ca = directory.join("probe-ca.pem");
    std::fs::write(&ca, &trusted.certificate).unwrap();
    DnsHttps::new(Config {
        resolvers: vec![dns.resolver],
        allowed_networks: vec!["127.0.0.1/32".parse().unwrap(), "::1/128".parse().unwrap()],
        trusted_ca_path: Some(ca),
        ..Default::default()
    })
    .unwrap()
}

fn pair(
    check: &mut Check,
    replies: [(u16, String); 2],
    untrusted_ipv6: bool,
) -> (Network, Network, Arc<AtomicUsize>) {
    let certificate = super::domain_network::tls_fixture::server_certificate(&[HOST]);
    let other_certificate = super::domain_network::tls_fixture::server_certificate(&[HOST]);
    let ipv4 = TcpListener::bind("127.0.0.1:0").unwrap();
    let ipv6 = TcpListener::bind(("::1", ipv4.local_addr().unwrap().port())).unwrap();
    let hits = Arc::new(AtomicUsize::new(0));
    let [(status4, body4), (status6, body6)] = replies;
    let observed = hits.clone();
    let path = format!("/.well-known/ruphish/{}", check.domain.challenge);
    let path4 = path.clone();
    let v4 = Network::with_listener(ipv4, &certificate, status4, move |host, request| {
        assert_eq!(host.split_once(':').unwrap().0, HOST);
        assert_eq!(request, path4, "redirects must never be followed");
        observed.fetch_add(1, Ordering::SeqCst);
        Some(body4.clone())
    });
    let observed = hits.clone();
    let v6 = Network::with_listener(
        ipv6,
        if untrusted_ipv6 {
            &other_certificate
        } else {
            &certificate
        },
        status6,
        move |host, request| {
            assert_eq!(host.split_once(':').unwrap().0, HOST);
            assert_eq!(request, path, "redirects must never be followed");
            observed.fetch_add(1, Ordering::SeqCst);
            Some(body6.clone())
        },
    );
    check.domain.origin = v4.origin(HOST);
    v4.publish(HOST, &check.domain.challenge);
    v4.resolve_to(vec![v4.address.ip(), v6.address.ip()]);
    (v4, v6, hits)
}

fn assert_passed(observation: &Observation, code: &str) {
    assert_eq!(observation.code, code);
    assert!(observation.control && observation.dns && observation.tls && observation.routing);
}

#[tokio::test(flavor = "multi_thread")]
async fn ipv4_only_ipv6_only_and_dual_stack_verify_without_user_selection() {
    let (dir, db) = fixture().await;
    let mut check = db
        .domains()
        .claim(time::OffsetDateTime::now_utc().unix_timestamp())
        .await
        .unwrap()
        .unwrap();
    let body = check.routing_proof.clone();
    let (v4, v6, hits) = pair(&mut check, [(200, body.clone()), (200, body)], false);
    for (addresses, expected_hits) in [
        (vec![v4.address.ip()], 1),
        (vec![v6.address.ip()], 1),
        (vec![v4.address.ip(), v6.address.ip()], 2),
    ] {
        v4.resolve_to(addresses);
        hits.store(0, Ordering::SeqCst);
        assert_passed(
            &verifier(dir.path(), &v4, &v4).verify(&check).await,
            "verified",
        );
        assert_eq!(hits.load(Ordering::SeqCst), expected_hits);
    }
    db.close().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn either_unavailable_family_falls_back_to_the_other() {
    let (dir, db) = fixture().await;
    let mut check = db
        .domains()
        .claim(time::OffsetDateTime::now_utc().unix_timestamp())
        .await
        .unwrap()
        .unwrap();
    for ipv6_works in [false, true] {
        let certificate = super::domain_network::tls_fixture::server_certificate(&[HOST]);
        let listener =
            TcpListener::bind(if ipv6_works { "[::1]:0" } else { "127.0.0.1:0" }).unwrap();
        let other_host = if ipv6_works { "127.0.0.1" } else { "::1" };
        let unavailable =
            TcpListener::bind((other_host, listener.local_addr().unwrap().port())).unwrap();
        let other = unavailable.local_addr().unwrap().ip();
        // Reserve then close the other family listener to produce connection refusal.
        drop(unavailable);
        let body = check.routing_proof.clone();
        let network =
            Network::with_listener(listener, &certificate, 200, move |_, _| Some(body.clone()));
        network.publish(HOST, &check.domain.challenge);
        network.resolve_to(vec![other, network.address.ip()]);
        check.domain.origin = network.origin(HOST);
        assert_passed(
            &verifier(dir.path(), &network, &network)
                .verify(&check)
                .await,
            "verified",
        );
    }
    db.close().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn stalled_family_is_bounded_and_does_not_discard_a_valid_proof() {
    let (dir, db) = fixture().await;
    let mut check = db
        .domains()
        .claim(time::OffsetDateTime::now_utc().unix_timestamp())
        .await
        .unwrap()
        .unwrap();
    let body = check.routing_proof.clone();
    let network = Network::new(move |_, _| Some(body.clone()));
    // TCP can connect but no TLS handshake is served on IPv6.
    let stalled = TcpListener::bind(("::1", network.address.port())).unwrap();
    network.publish(HOST, &check.domain.challenge);
    network.resolve_to(vec![
        stalled.local_addr().unwrap().ip(),
        network.address.ip(),
    ]);
    check.domain.origin = network.origin(HOST);
    let verifier = verifier(dir.path(), &network, &network);
    let result = tokio::time::timeout(Duration::from_secs(10), verifier.verify(&check))
        .await
        .unwrap();
    assert_passed(&result, "verified");
    // With only the stalled family, no successful path exists.
    network.resolve_to(vec![stalled.local_addr().unwrap().ip()]);
    let result = tokio::time::timeout(Duration::from_secs(10), verifier.verify(&check))
        .await
        .unwrap();
    assert_eq!(result.code, "domain_https_unavailable");
    assert!(!result.tls && !result.routing);
    db.close().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn reachable_bad_proofs_redirects_and_oversized_responses_are_retained_as_warnings() {
    let (dir, db) = fixture().await;
    let mut check = db
        .domains()
        .claim(time::OffsetDateTime::now_utc().unix_timestamp())
        .await
        .unwrap()
        .unwrap();
    for bad in [
        (200, check.domain.challenge.clone()), // Public TXT token is not the private routing proof.
        (302, check.routing_proof.clone()),
        (0, check.routing_proof.clone()), // Malformed HTTP status line.
        (200, "x".repeat(129)),
        (404, check.routing_proof.clone()),
    ] {
        for bad_ipv6 in [false, true] {
            let good = (200, check.routing_proof.clone());
            let replies = if bad_ipv6 {
                [good, bad.clone()]
            } else {
                [bad.clone(), good]
            };
            let (v4, _v6, hits) = pair(&mut check, replies, false);
            assert_passed(
                &verifier(dir.path(), &v4, &v4).verify(&check).await,
                "verified_with_endpoint_errors",
            );
            assert_eq!(hits.load(Ordering::SeqCst), 2);
        }
    }
    db.close().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn invalid_tls_is_reported_and_proofs_cannot_be_combined_across_endpoints() {
    let (dir, db) = fixture().await;
    let mut check = db
        .domains()
        .claim(time::OffsetDateTime::now_utc().unix_timestamp())
        .await
        .unwrap()
        .unwrap();
    let good = (200, check.routing_proof.clone());
    let (v4, v6, hits) = pair(&mut check, [good.clone(), good.clone()], true);
    // Trust only one endpoint at a time; the other's independently signed TLS fails.
    for trusted in [&v4, &v6] {
        assert_passed(
            &verifier(dir.path(), &v4, trusted).verify(&check).await,
            "verified_with_endpoint_errors",
        );
    }
    assert_eq!(
        hits.load(Ordering::SeqCst),
        2,
        "untrusted peers never receive the HTTP request"
    );
    let (v4, _v6, hits) = pair(&mut check, [(200, "wrong-instance".into()), good], true);
    let result = verifier(dir.path(), &v4, &v4).verify(&check).await;
    assert_eq!(result.code, "domain_routing_mismatch");
    assert!(
        !result.tls && !result.routing,
        "TLS and proof must pass on the same endpoint"
    );
    assert_eq!(hits.load(Ordering::SeqCst), 1);
    db.close().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn all_destinations_are_checked_before_any_probe_and_address_count_remains_bounded() {
    let (dir, db) = fixture().await;
    let mut check = db
        .domains()
        .claim(time::OffsetDateTime::now_utc().unix_timestamp())
        .await
        .unwrap()
        .unwrap();
    let good = (200, check.routing_proof.clone());
    let (v4, _v6, hits) = pair(&mut check, [good.clone(), good], false);
    let verifier = verifier(dir.path(), &v4, &v4);
    for forbidden in [
        "169.254.169.254",
        "fe80::1",
        "::ffff:169.254.169.254",
        "10.0.0.1",
    ] {
        v4.resolve_to(vec![v4.address.ip(), forbidden.parse::<IpAddr>().unwrap()]);
        let result = verifier.verify(&check).await;
        assert_eq!(result.code, "domain_destination_denied");
        assert!(result.control && !result.dns && !result.tls && !result.routing);
    }
    v4.resolve_to(
        (1..=17)
            .map(|n| format!("127.0.0.{n}").parse().unwrap())
            .collect(),
    );
    assert_eq!(
        verifier.verify(&check).await.code,
        "domain_dns_address_limit"
    );
    assert_eq!(
        hits.load(Ordering::SeqCst),
        0,
        "preflight must finish before any HTTP side effect"
    );
    db.close().await;
}
