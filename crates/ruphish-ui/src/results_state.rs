//! Reporting URL and chart transformations shared by the UI and functional tests.
use ruphish_contracts::reporting::Filter;

/// Encode only active analysis filters; export options belong only to download requests.
pub fn query(filter: &Filter, export: bool) -> String {
    let mut pairs = Vec::<(&str, String)>::new();
    for (key, value) in [
        ("recipient", filter.recipient),
        ("from", filter.from),
        ("until", filter.until),
    ] {
        if let Some(value) = value {
            pairs.push((key, value.to_string()));
        }
    }
    for (key, value) in [
        ("recipients", &filter.recipients),
        ("event", &filter.event),
        ("search", &filter.search),
        ("source", &filter.source),
    ] {
        if !value.is_empty() {
            pairs.push((key, value.clone()));
        }
    }
    let sort = serde_json::to_value(filter.sort).unwrap_or_default();
    let order = serde_json::to_value(filter.order).unwrap_or_default();
    if filter.sort != Default::default() || filter.order != Default::default() {
        pairs.push(("sort", sort.as_str().unwrap_or("email").to_owned()));
        pairs.push(("order", order.as_str().unwrap_or("asc").to_owned()));
    }
    if filter.event_sort != Default::default()
        || filter.event_order != ruphish_domain::reporting::SortDirection::Desc
    {
        let sort = serde_json::to_value(filter.event_sort).unwrap_or_default();
        let order = serde_json::to_value(filter.event_order).unwrap_or_default();
        pairs.push(("event_sort", sort.as_str().unwrap_or("time").to_owned()));
        pairs.push(("event_order", order.as_str().unwrap_or("desc").to_owned()));
    }
    if !export {
        if filter.offset != 0 {
            pairs.push(("offset", filter.offset.to_string()));
        }
        if filter.limit != 50 {
            pairs.push(("limit", filter.limit.to_string()));
        }
    } else {
        pairs.push(("format", filter.format.clone()));
        pairs.push(("scope", filter.scope.clone()));
    }
    url::form_urlencoded::Serializer::new(String::new())
        .extend_pairs(pairs)
        .finish()
}

/// Append a query without creating an empty question mark.
pub fn address(path: &str, filter: &Filter, export: bool) -> String {
    let query = query(filter, export);
    if query.is_empty() {
        path.into()
    } else {
        format!("{path}?{query}")
    }
}

/// Link to complete recipient history while retaining the originating Results URL.
pub fn recipient_address(campaign: i64, recipient: i64, origin: &str) -> String {
    let query = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("return_to", &results_return(campaign, Some(origin)))
        .finish();
    format!("/campaigns/{campaign}/results/recipients/{recipient}?{query}")
}

/// Accept only a local Results destination for this campaign, including its query.
pub fn results_return(campaign: i64, origin: Option<&str>) -> String {
    let fallback = format!("/campaigns/{campaign}/results");
    origin
        .filter(|value| {
            value.split('?').next() == Some(fallback.as_str())
                && !value
                    .chars()
                    .any(|c| c.is_control() || matches!(c, '\\' | '#'))
        })
        .map(str::to_owned)
        .unwrap_or(fallback)
}

/// Whole-number tick spacing, with at most six intervals and a visible zero baseline.
pub fn count_axis(maximum: i64) -> (i64, i64) {
    let maximum = maximum.max(1);
    let rough = maximum / 6 + i64::from(maximum % 6 != 0);
    let mut scale = 1_i64;
    while scale <= rough / 10 {
        scale *= 10;
    }
    let step = [1, 2, 5, 10]
        .into_iter()
        .map(|n| n * scale)
        .find(|n| *n >= rough)
        .unwrap_or(scale);
    let top = maximum.saturating_add(step - 1) / step * step;
    (top, step)
}

/// Readable minute/hour/day tick intervals chosen from the actual available width.
pub fn time_axis(first: i64, last: i64, width: usize) -> (i64, Vec<i64>) {
    let slots = (width / 150).clamp(1, 8) as i64;
    let span = last.saturating_sub(first).max(60);
    let desired = span / slots + i64::from(span % slots != 0);
    let step = [
        60, 120, 300, 600, 900, 1800, 3600, 7200, 10800, 21600, 43200, 86400, 172800, 259200,
        432000, 604800, 864000, 1296000, 2592000,
    ]
    .into_iter()
    .find(|step| *step >= desired)
    .unwrap_or_else(|| ((desired / 86400) + 1) * 86400);
    let start = first.div_euclid(step) * step + if first.rem_euclid(step) == 0 { 0 } else { step };
    let ticks = (0..=slots)
        .map(|n| start + n * step)
        .take_while(|time| *time <= last)
        .collect();
    (step, ticks)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "Independent transformation assertions")]
mod tests {
    use super::*;
    #[test]
    fn recipient_return_preserves_results_query_and_rejects_other_destinations() {
        let origin = "/campaigns/7/results?recipients=2%2C9&event=link_clicked&source=public&from=100&until=200&sort=name&order=desc&offset=50&limit=25";
        let address = recipient_address(7, 9, origin);
        let parsed = url::Url::parse(&format!("https://admin.example.test{address}")).unwrap();
        let target = parsed
            .query_pairs()
            .find(|(key, _)| key == "return_to")
            .unwrap()
            .1;
        assert_eq!(results_return(7, Some(&target)), origin);
        for invalid in [
            None,
            Some("https://evil.test/campaigns/7/results"),
            Some("//evil.test"),
            Some("/campaigns/8/results?offset=50"),
            Some("/campaigns/7/results/recipients/9"),
            Some("/campaigns/7/results#bad"),
            Some("/campaigns/7/results?x=\n"),
        ] {
            assert_eq!(results_return(7, invalid), "/campaigns/7/results");
        }
    }

    #[test]
    fn bookmark_and_download_filters_round_trip_without_empty_or_irrelevant_parameters() {
        let mut filter = Filter::default();
        assert_eq!(
            address("/campaigns/7/results", &filter, false),
            "/campaigns/7/results"
        );
        filter.recipients = "2,9".into();
        filter.search = "Renée & team+one".into();
        filter.event = "email_reported".into();
        filter.offset = 50;
        filter.format = "xlsx".into();
        filter.scope = "all".into();
        let bookmark = query(&filter, false);
        let parsed: Filter = serde_urlencoded::from_str(&bookmark).unwrap();
        assert_eq!(parsed.recipients, "2,9");
        assert_eq!(parsed.event, "email_reported");
        assert_eq!(parsed.search, filter.search);
        assert_eq!(parsed.offset, 50);
        assert!(!bookmark.contains("format="));
        assert!(!bookmark.contains("source="));
        let download: Filter = serde_urlencoded::from_str(&query(&filter, true)).unwrap();
        assert_eq!(download.offset, 0);
        assert_eq!(download.recipients, parsed.recipients);
        assert_eq!(download.format, "xlsx");
    }
    #[test]
    fn event_counts_have_whole_ticks_without_crowding_or_clipping() {
        for maximum in [0, 1, 2, 3, 7, 31, 999, 100_000] {
            let (top, step) = count_axis(maximum);
            assert!(step >= 1 && top >= maximum && top > 0);
            assert!(top / step <= 6);
        }
        assert_eq!(count_axis(2), (2, 1));
    }

    #[test]
    fn time_labels_adapt_from_minutes_to_thirty_days_and_narrow_screens() {
        for (span, expected) in [
            (300, 60),
            (1800, 300),
            (21600, 3600),
            (604800, 86400),
            (2592000, 432000),
        ] {
            let width = if span == 604800 { 1200 } else { 900 };
            assert_eq!(time_axis(0, span, width).0, expected);
        }
        for width in [240, 450, 900, 1440] {
            for span in [
                0, 60, 299, 300, 301, 1799, 1800, 1801, 86399, 86400, 604800, 2592000,
            ] {
                let first = 1792443570; // Unaligned time; axis ticks must still be whole minutes.
                let (step, ticks) = time_axis(first, first + span, width);
                assert!(step >= 60 && step % 60 == 0);
                assert!(
                    ticks
                        .iter()
                        .all(|time| *time >= first && *time <= first + span && time % 60 == 0)
                );
                assert!(ticks.windows(2).all(|pair| pair[1] - pair[0] == step));
                assert!(ticks.len() <= (width / 150).clamp(1, 8) + 1);
            }
        }
        assert!(time_axis(0, 2592000, 240).0 > time_axis(0, 2592000, 1440).0);
    }
}

/// Timing of the first recorded interaction, independent of visible history pages.
#[derive(Debug, PartialEq, Eq)]
pub enum InteractionTiming {
    /// No opened, clicked, submitted or reported event was recorded.
    NotObserved,
    /// Evidence exists but cannot be related to a valid SMTP acceptance instant.
    Unavailable,
    /// Nonnegative elapsed seconds from acceptance to the first interaction.
    AfterAcceptance(u64),
}

/// Use the earliest observed interaction; never infer a missing open from a click,
/// or skip an earlier inconsistent timestamp in favor of a convenient later one.
pub fn first_interaction(first: [Option<i64>; 5], totals: [i64; 5]) -> InteractionTiming {
    if totals[1..].iter().all(|count| *count == 0) {
        return InteractionTiming::NotObserved;
    }
    if (1..5).any(|index| totals[index] > 0 && first[index].is_none()) {
        return InteractionTiming::Unavailable;
    }
    let earliest = first[1..].iter().flatten().min().copied();
    match first[0]
        .zip(earliest)
        .and_then(|(accepted, event)| event.checked_sub(accepted))
    {
        Some(seconds) if seconds >= 0 => InteractionTiming::AfterAcceptance(seconds as u64),
        _ => InteractionTiming::Unavailable,
    }
}

#[cfg(test)]
mod interaction_timing_tests {
    use super::{InteractionTiming::*, first_interaction};
    #[test]
    fn timing_uses_recorded_evidence_even_when_the_first_event_is_not_an_open() {
        assert_eq!(
            first_interaction(
                [Some(100), None, Some(160), Some(200), Some(150)],
                [1, 0, 1, 1, 1]
            ),
            AfterAcceptance(50)
        );
        assert_eq!(
            first_interaction([Some(100), Some(100), None, None, None], [1, 3, 0, 0, 0]),
            AfterAcceptance(0)
        );
        assert_eq!(
            first_interaction([Some(100), None, None, None, None], [1, 0, 0, 0, 0]),
            NotObserved
        );
    }
    #[test]
    fn incomplete_or_inconsistent_history_never_becomes_an_invented_response_time() {
        assert_eq!(
            first_interaction([Some(100), None, Some(160), None, None], [1, 1, 1, 0, 0]),
            Unavailable
        );
        assert_eq!(
            first_interaction([None, Some(150), None, None, None], [0, 1, 0, 0, 0]),
            Unavailable
        );
        assert_eq!(
            first_interaction([Some(100), None, None, None, None], [1, 1, 0, 0, 0]),
            Unavailable
        );
        assert_eq!(
            first_interaction(
                [Some(100), Some(90), Some(150), None, None],
                [1, 1, 1, 0, 0]
            ),
            Unavailable
        );
        assert_eq!(
            first_interaction(
                [Some(i64::MIN), Some(i64::MAX), None, None, None],
                [1, 1, 0, 0, 0]
            ),
            Unavailable
        );
    }
}
