//! Calendar and persistence guarantees, with independent expected instants.
#![allow(clippy::unwrap_used)]
use ruphish_domain::datetime::*;

#[test]
fn fractional_utc_values_convert_without_losing_precision() {
    let timestamp = parse("2026-10-18T19:53:21.8939891Z").unwrap();
    assert_eq!(utc(timestamp), "2026-10-18T19:53:21.8939891Z");
    assert_eq!(timestamp.subsec_nanosecond(), 893_989_100);
    for (zone, local) in [
        ("UTC", "2026-10-18T19:53"),
        ("Europe/Berlin", "2026-10-18T21:53"),
        ("America/New_York", "2026-10-18T15:53"),
        ("Asia/Tokyo", "2026-10-19T04:53"),
        ("Asia/Kathmandu", "2026-10-19T01:38"),
        ("Australia/Sydney", "2026-10-19T06:53"),
    ] {
        assert_eq!(local_input(timestamp, &named_zone(zone).unwrap()), local);
    }
    assert_eq!(
        parse_stored("2026-10-18 21:53:21.8939891+02:00").unwrap(),
        timestamp
    );
    assert_eq!(
        parse("2026-10-18T21:53:21.8939891+02:00").unwrap(),
        timestamp
    );
}

#[test]
fn invalid_and_offsetless_instants_never_use_the_machine_zone() {
    for value in [
        "",
        "bad",
        "2026-02-30T12:00:00Z",
        "2026-10-18",
        "2026-10-18T19:53:21",
        "2026-10-18T19:53Z",
        "2026-10-18T19:53:21Z[Europe/Berlin]",
        "2026-10-18T19:53:21.1234567891Z",
    ] {
        assert!(parse(value).is_err(), "{value}");
    }
    assert!(from_seconds(i64::MAX).is_err());
    assert!(named_zone("invalid/zone").is_err());
    assert_eq!(zone_name(&display_zone(None, None)), "UTC");
    assert_eq!(
        zone_name(&display_zone(Some("invalid"), Some("Asia/Tokyo"))),
        "Asia/Tokyo"
    );
    assert_eq!(
        zone_name(&display_zone(
            Some("America/New_York"),
            Some("Europe/Berlin")
        )),
        "America/New_York"
    );
    assert_eq!(zone_name(&display_zone(None, Some("invalid"))), "UTC");
}

#[test]
fn dst_conversion_and_civil_resolution_share_the_same_rules() {
    for (local, zone, utc_expected) in [
        ("2026-03-29T02:30", "Europe/Berlin", "2026-03-29T01:30:00Z"),
        ("2026-10-25T02:30", "Europe/Berlin", "2026-10-25T00:30:00Z"),
        (
            "2026-03-08T02:30",
            "America/New_York",
            "2026-03-08T07:30:00Z",
        ),
        (
            "2026-11-01T01:30",
            "America/New_York",
            "2026-11-01T05:30:00Z",
        ),
        (
            "2026-10-04T02:15",
            "Australia/Lord_Howe",
            "2026-10-03T15:45:00Z",
        ),
    ] {
        assert_eq!(utc(resolve_local(local, zone).unwrap()), utc_expected);
    }
    for input in [
        "2026-10-25T02:30Z",
        "2026-10-25T02:30+02:00",
        "2026-10-25T02:30[Europe/Berlin]",
        "2026-02-30T10:00",
    ] {
        assert!(resolve_local(input, "Europe/Berlin").is_err());
    }
    let zone = named_zone("Europe/Berlin").unwrap();
    for (utc_value, local) in [
        ("2026-03-29T00:59:59Z", "2026-03-29T01:59"),
        ("2026-03-29T01:00:00Z", "2026-03-29T03:00"),
        ("2026-10-25T00:30:00Z", "2026-10-25T02:30"),
        ("2026-10-25T01:30:00Z", "2026-10-25T02:30"),
    ] {
        assert_eq!(local_input(parse(utc_value).unwrap(), &zone), local);
    }
}

#[test]
fn utc_revisions_are_monotonic_even_when_the_clock_moves_back() {
    let now = parse("2026-10-18T19:53:21.8939891Z").unwrap();
    assert_eq!(
        next_revision(None, now).unwrap(),
        "2026-10-18T19:53:21.8939891Z"
    );
    assert_eq!(
        next_revision(Some("2026-10-18T19:53:21.8939891Z"), now).unwrap(),
        "2026-10-18T19:53:21.893989101Z"
    );
    assert_eq!(
        next_revision(Some("2026-10-19T00:00:00Z"), now).unwrap(),
        "2026-10-19T00:00:00.000000001Z"
    );
    assert!(next_revision(Some("invalid"), now).is_err());
    assert!(next_revision(Some("9999-12-31T23:59:59.999999999Z"), now).is_err());
    assert_eq!(utc(ZERO_TIMESTAMP), ZERO);
}
