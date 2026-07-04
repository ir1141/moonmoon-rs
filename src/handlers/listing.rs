//! The VOD listing pipeline: given VOD refs already in final display order,
//! produce one page of cards — paginated, with period (month) or series (game)
//! headers, plus the "load more" nav. Callers own selection (browse filters the
//! catalog; history resolves client ids) and hand ordered refs to
//! [`Listing::build`]; the module owns everything from there.

use super::{ListQuery, VodDisplay, build_next_url};

/// How a listing paginates. `All` renders every ref (history, which has no
/// "load more"); `Paged` slices one batch and builds the next-page nav URL
/// (browse streams lens).
pub(crate) enum Pagination<'a> {
    All,
    Paged {
        base: &'a str,
        page: usize,
        batch: usize,
        params: &'a ListQuery,
    },
}

/// Which grouping header the listing renders above cards. The caller decides
/// the mode in one expression; the module owns the algorithms and the period
/// seed, so the choice lives in exactly one place.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Headers {
    None,
    /// Month headers ("March 2026") for chronological views, seeded from the
    /// card just before the page slice so a page starting mid-month doesn't
    /// repeat the header.
    Period,
    /// Run-length game headers ("Elden Ring · 3 streams") keyed by each card's
    /// header key (the watched chapter). Refs must already be ordered so each
    /// game forms one contiguous run.
    Series,
}

/// One built page of a listing.
pub(crate) struct Listing {
    pub vods: Vec<VodDisplay>,
    pub has_more: bool,
    pub next_url: String,
}

impl Listing {
    /// Build one page from `refs` (already in final display order). `build`
    /// turns a ref into its `VodDisplay` and its header key (the key is only
    /// read under [`Headers::Series`]). Pagination happens before `build`, so
    /// only the rendered slice pays for chapter segments and tags.
    pub(crate) fn build<R>(
        refs: &[R],
        pagination: Pagination<'_>,
        headers: Headers,
        build: impl Fn(&R) -> (VodDisplay, Option<String>),
    ) -> Listing {
        let total = refs.len();
        let (start, end, has_more, next_url) = match pagination {
            Pagination::All => (0, total, false, String::new()),
            Pagination::Paged {
                base,
                page,
                batch,
                params,
            } => {
                let raw_start = page.saturating_mul(batch);
                let end = raw_start.saturating_add(batch).min(total);
                let has_more = end < total;
                let next_url = build_next_url(base, page.saturating_add(1), params);
                (raw_start.min(total), end, has_more, next_url)
            }
        };

        // Period headers need the month of the card immediately before the
        // slice; build that one extra card (only when paginating past the
        // first card) to seed the run so the first card doesn't repeat the
        // previous page's month header.
        let seed = match headers {
            Headers::Period if start > 0 => Some(build(&refs[start - 1]).0.created_at),
            _ => None,
        };

        let mut displays = Vec::with_capacity(end - start);
        let mut keys = Vec::with_capacity(end - start);
        for r in &refs[start..end] {
            let (display, key) = build(r);
            displays.push(display);
            keys.push(key);
        }

        match headers {
            Headers::None => {}
            Headers::Period => assign_period_headers_seeded(&mut displays, seed.as_deref()),
            Headers::Series => assign_series_headers(&mut displays, &keys),
        }

        Listing {
            vods: displays,
            has_more,
            next_url,
        }
    }
}

/// Insert a calendar-month header (e.g. "March 2026") before the first card of
/// each new month. `prev_stream_time` is the stream time of the card
/// immediately BEFORE this slice (the last card of the previous page), so a
/// page that starts mid-month doesn't repeat the month header. The caller only
/// selects [`Headers::Period`] for chronological views, so no sort gate is
/// needed here.
fn assign_period_headers_seeded(displays: &mut [VodDisplay], prev_stream_time: Option<&str>) {
    let mut current: Option<String> = prev_stream_time.map(month_year_long);
    for display in displays.iter_mut() {
        let label = month_year_long(&display.created_at);
        if current.as_deref() != Some(label.as_str()) {
            display.period_header = Some(label.clone());
            current = Some(label);
        }
    }
}

/// Insert a run-length game header ("Elden Ring · 3 streams") before the first
/// card of each contiguous run of the same key. Refs must arrive ordered so a
/// game forms one run; a `keys`/`displays` length mismatch no-ops.
fn assign_series_headers(displays: &mut [VodDisplay], keys: &[Option<String>]) {
    if displays.is_empty() || keys.len() != displays.len() {
        return;
    }

    let norm = |p: &Option<String>| p.as_deref().map(|s| s.to_lowercase());

    let mut run_start = 0usize;
    for i in 1..=displays.len() {
        let boundary = i == displays.len() || norm(&keys[i]) != norm(&keys[run_start]);
        if boundary {
            let count = i - run_start;
            let label_name = keys[run_start].as_deref().unwrap_or("Untagged").to_string();
            let noun = if count == 1 { "stream" } else { "streams" };
            displays[run_start].period_header = Some(format!("{label_name} · {count} {noun}"));
            run_start = i;
        }
    }
}

fn month_year_long(created_at: &str) -> String {
    let Some(date_part) = created_at.get(..10) else {
        return created_at.to_string();
    };
    let parts: Vec<&str> = date_part.split('-').collect();
    if parts.len() != 3 {
        return date_part.to_string();
    }
    format!("{} {}", month_long(parts[1]), parts[0])
}

fn month_long(month_part: &str) -> &str {
    match month_part {
        "01" => "January",
        "02" => "February",
        "03" => "March",
        "04" => "April",
        "05" => "May",
        "06" => "June",
        "07" => "July",
        "08" => "August",
        "09" => "September",
        "10" => "October",
        "11" => "November",
        "12" => "December",
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test input: the ref the builder sees. `key` is the header key (Series).
    struct In {
        id: &'static str,
        created_at: &'static str,
        key: Option<&'static str>,
    }

    fn input(id: &'static str, created_at: &'static str, key: Option<&'static str>) -> In {
        In {
            id,
            created_at,
            key,
        }
    }

    fn make_display(id: &str, created_at: &str) -> VodDisplay {
        VodDisplay {
            id: id.into(),
            display_title: "t".into(),
            formatted_date: String::new(),
            formatted_date_short: String::new(),
            duration: None,
            thumbnail_url: None,
            chapter_segments: vec![],
            created_at: created_at.into(),
            match_label: None,
            status_label: None,
            progress_seconds: None,
            history_state: None,
            chapter_names: vec![],
            duration_seconds: 0,
            period_header: None,
            watch_url: format!("/watch/{id}"),
        }
    }

    fn build_input(i: &In) -> (VodDisplay, Option<String>) {
        (make_display(i.id, i.created_at), i.key.map(String::from))
    }

    fn headers(refs: &[In], pagination: Pagination<'_>, mode: Headers) -> Vec<Option<String>> {
        Listing::build(refs, pagination, mode, build_input)
            .vods
            .into_iter()
            .map(|d| d.period_header)
            .collect()
    }

    #[test]
    fn none_mode_leaves_every_card_unheadered() {
        let refs = [
            input("1", "2024-03-10T00:00:00Z", None),
            input("2", "2024-01-01T00:00:00Z", None),
        ];
        let got = headers(&refs, Pagination::All, Headers::None);
        assert!(got.iter().all(Option::is_none));
    }

    #[test]
    fn period_headers_group_by_month() {
        let refs = [
            input("1", "2024-03-10T00:00:00Z", None),
            input("2", "2024-03-05T00:00:00Z", None),
            input("3", "2024-01-20T00:00:00Z", None),
            input("4", "2024-01-15T00:00:00Z", None),
        ];
        let got = headers(&refs, Pagination::All, Headers::Period);
        assert_eq!(got[0].as_deref(), Some("March 2024"));
        assert!(got[1].is_none());
        assert_eq!(got[2].as_deref(), Some("January 2024"));
        assert!(got[3].is_none());
    }

    #[test]
    fn period_headers_split_consecutive_months() {
        let refs = [
            input("1", "2024-04-01T00:00:00Z", None),
            input("2", "2024-03-31T00:00:00Z", None),
        ];
        let got = headers(&refs, Pagination::All, Headers::Period);
        assert_eq!(got[0].as_deref(), Some("April 2024"));
        assert_eq!(got[1].as_deref(), Some("March 2024"));
    }

    #[test]
    fn period_seed_suppresses_repeat_month_across_a_page_boundary() {
        let params = ListQuery::default();
        // Page 1 (batch 1) renders ref[1]; the module seeds from ref[0].
        let same_month = [
            input("0", "2024-03-31T00:00:00Z", None),
            input("1", "2024-03-10T00:00:00Z", None),
        ];
        let got = headers(
            &same_month,
            Pagination::Paged {
                base: "/x",
                page: 1,
                batch: 1,
                params: &params,
            },
            Headers::Period,
        );
        assert_eq!(got.len(), 1);
        assert!(
            got[0].is_none(),
            "same-month page-2 card must not repeat header"
        );

        let new_month = [
            input("0", "2024-04-01T00:00:00Z", None),
            input("1", "2024-03-10T00:00:00Z", None),
        ];
        let got = headers(
            &new_month,
            Pagination::Paged {
                base: "/x",
                page: 1,
                batch: 1,
                params: &params,
            },
            Headers::Period,
        );
        assert_eq!(got[0].as_deref(), Some("March 2024"));
    }

    #[test]
    fn series_headers_run_length_with_count() {
        let refs = [
            input("1", "2024-01-01T00:00:00Z", Some("Elden Ring")),
            input("2", "2024-01-01T00:00:00Z", Some("Elden Ring")),
            input("3", "2024-01-01T00:00:00Z", Some("Dark Souls")),
            input("4", "2024-01-01T00:00:00Z", Some("Elden Ring")),
        ];
        let got = headers(&refs, Pagination::All, Headers::Series);
        assert_eq!(got[0].as_deref(), Some("Elden Ring · 2 streams"));
        assert!(got[1].is_none());
        assert_eq!(got[2].as_deref(), Some("Dark Souls · 1 stream"));
        assert_eq!(got[3].as_deref(), Some("Elden Ring · 1 stream"));
    }

    #[test]
    fn series_headers_untagged_and_case_insensitive() {
        let refs = [
            input("1", "2024-01-01T00:00:00Z", None),
            input("2", "2024-01-01T00:00:00Z", None),
            input("3", "2024-01-01T00:00:00Z", Some("Elden Ring")),
            input("4", "2024-01-01T00:00:00Z", Some("elden ring")),
        ];
        let got = headers(&refs, Pagination::All, Headers::Series);
        assert_eq!(got[0].as_deref(), Some("Untagged · 2 streams"));
        assert!(got[1].is_none());
        assert_eq!(got[2].as_deref(), Some("Elden Ring · 2 streams"));
        assert!(got[3].is_none());
    }

    #[test]
    fn paged_reports_has_more_and_next_url() {
        let params = ListQuery::default();
        let refs: Vec<In> = (0..5)
            .map(|i| match i {
                0 => input("0", "2024-01-05T00:00:00Z", None),
                1 => input("1", "2024-01-04T00:00:00Z", None),
                2 => input("2", "2024-01-03T00:00:00Z", None),
                3 => input("3", "2024-01-02T00:00:00Z", None),
                _ => input("4", "2024-01-01T00:00:00Z", None),
            })
            .collect();

        let first = Listing::build(
            &refs,
            Pagination::Paged {
                base: "/x",
                page: 0,
                batch: 2,
                params: &params,
            },
            Headers::None,
            build_input,
        );
        assert_eq!(first.vods.len(), 2);
        assert!(first.has_more);
        assert_eq!(first.next_url, "/x?page=1");

        let last = Listing::build(
            &refs,
            Pagination::Paged {
                base: "/x",
                page: 2,
                batch: 2,
                params: &params,
            },
            Headers::None,
            build_input,
        );
        assert_eq!(last.vods.len(), 1);
        assert!(!last.has_more);
    }

    #[test]
    fn all_renders_everything_without_nav() {
        let refs = [
            input("1", "2024-01-02T00:00:00Z", None),
            input("2", "2024-01-01T00:00:00Z", None),
        ];
        let listing = Listing::build(&refs, Pagination::All, Headers::None, build_input);
        assert_eq!(listing.vods.len(), 2);
        assert!(!listing.has_more);
        assert!(listing.next_url.is_empty());
    }

    #[test]
    fn out_of_range_page_is_empty_not_a_panic() {
        let params = ListQuery::default();
        let refs = [input("1", "2024-01-01T00:00:00Z", None)];
        let listing = Listing::build(
            &refs,
            Pagination::Paged {
                base: "/x",
                page: 9,
                batch: 2,
                params: &params,
            },
            Headers::Period,
            build_input,
        );
        assert!(listing.vods.is_empty());
        assert!(!listing.has_more);
    }
}
