use super::{
    GAME_BATCH_SIZE, GamesGridTemplate, ListFilterConfig, ListMetadata, ListQuery, Section,
    VOD_BATCH_SIZE, VodDisplay, VodsGridTemplate, assign_period_headers_seeded, build_next_url,
    date_preset_state, filter_games_with_metadata, filter_vods_with_metadata, get_chapter_start,
    list_sort_options_grouped, paginate_with_nav, render_template, selected_sort_option,
    vod_has_game, vod_stream_time,
};
use crate::SharedState;
use crate::middleware::CspNonce;
use crate::vods::{Game, chapter_color_idx};
use askama::Template;
use axum::extract::{Extension, Path, Query, RawQuery, State};
use axum::response::{IntoResponse, Redirect, Response};
use std::sync::Arc;

const GAME_SORTS: [(&str, &str, bool); 6] = [
    ("recent", "Latest stream", false),
    ("oldest", "Oldest stream", false),
    ("most", "Most streams", true),
    ("fewest", "Fewest streams", false),
    ("az", "A - Z", true),
    ("za", "Z - A", false),
];
const STREAM_SORTS: [(&str, &str, bool); 4] = [
    ("newest", "Newest First", false),
    ("oldest", "Oldest First", false),
    ("longest", "Longest", true),
    ("shortest", "Shortest", false),
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Lens {
    Games,
    Streams,
}

/// Resolve the active lens. An explicit `lens` always wins; a non-empty `game`
/// drilldown only forces the streams lens when no explicit lens was given.
fn resolve_lens(lens: Option<&str>, game: Option<&str>) -> Lens {
    match lens {
        Some("streams") => Lens::Streams,
        Some("games") => Lens::Games,
        _ if game.is_some_and(|g| !g.is_empty()) => Lens::Streams,
        _ => Lens::Games,
    }
}

/// `&search=…&from=…&to=…` for the values that survive a lens switch (sort and
/// game are intentionally dropped). Empty when nothing is carried.
fn carry_filters(params: &ListQuery) -> String {
    let mut out = String::new();
    for (key, value) in [
        ("search", &params.search),
        ("from", &params.from),
        ("to", &params.to),
    ] {
        if let Some(v) = value.as_deref().filter(|v| !v.is_empty()) {
            out.push('&');
            out.push_str(key);
            out.push('=');
            out.push_str(&urlencoding::encode(v));
        }
    }
    out
}

#[derive(Template)]
#[template(path = "browse.html")]
struct BrowsePageTemplate {
    is_games_lens: bool,
    games: Vec<Game>,
    vods: Vec<VodDisplay>,
    game: Option<String>,
    game_color_idx: u8,
    metadata: ListMetadata,
    filter: ListFilterConfig,
    search: Option<String>,
    from: Option<String>,
    to: Option<String>,
    has_more: bool,
    next_url: String,
    lens_games_url: String,
    lens_streams_url: String,
    clear_game_url: String,
    show_recency: bool,
    show_oldest_recency: bool,
    show_game_tags: bool,
    show_subtitle: bool,
    is_filtered: bool,
    active_section: Section,
    nonce: String,
}

struct PreparedBrowse {
    games: Vec<Game>,
    vods: Vec<VodDisplay>,
    metadata: ListMetadata,
    has_more: bool,
    next_url: String,
    archive_min_date: String,
    archive_max_date: String,
    show_recency: bool,
    show_oldest_recency: bool,
    show_game_tags: bool,
    show_subtitle: bool,
}

async fn prepare_browse(
    state: &SharedState,
    lens: Lens,
    game: Option<&str>,
    params: &ListQuery,
    sort: &str,
) -> PreparedBrowse {
    let vods = {
        let guard = state.vods.read().await;
        Arc::clone(&*guard)
    };
    let (archive_min_date, archive_max_date) = state.date_bounds.read().await.clone();

    match lens {
        Lens::Games => {
            let games = {
                let guard = state.games.read().await;
                Arc::clone(&*guard)
            };
            let filtered =
                filter_games_with_metadata(&games, &vods, params, "/browse?lens=games", sort);
            let (paged, has_more, next_url) =
                paginate_with_nav(filtered.games, "/browse/grid", GAME_BATCH_SIZE, params);
            PreparedBrowse {
                games: paged,
                vods: Vec::new(),
                metadata: filtered.metadata,
                has_more,
                next_url,
                archive_min_date,
                archive_max_date,
                show_recency: matches!(sort, "recent" | "oldest"),
                show_oldest_recency: sort == "oldest",
                show_game_tags: false,
                show_subtitle: false,
            }
        }
        Lens::Streams => {
            let (refs, metadata) = match game {
                Some(name) => filter_vods_with_metadata(
                    vods.iter().filter(|v| vod_has_game(v, name)),
                    params,
                    "/browse?lens=streams",
                ),
                None => filter_vods_with_metadata(vods.iter(), params, "/browse?lens=streams"),
            };

            // Paginate BEFORE building VodDisplays so chapter segments and game
            // tags are only computed for the cards actually rendered.
            let total = refs.len();
            let page = params.page.unwrap_or(0);
            let start = page.saturating_mul(VOD_BATCH_SIZE);
            let end = start.saturating_add(VOD_BATCH_SIZE).min(total);
            let has_more = end < total;
            let next_url = build_next_url("/browse/grid", page.saturating_add(1), params);
            let prev_stream_time = start
                .checked_sub(1)
                .and_then(|i| refs.get(i))
                .map(|r| vod_stream_time(r.vod).to_string());
            let page_refs = if start >= total {
                &refs[0..0]
            } else {
                &refs[start..end]
            };

            let mut displays: Vec<VodDisplay> = page_refs
                .iter()
                .map(|r| {
                    let mut display = match game {
                        Some(name) => VodDisplay::from_vod_with(
                            r.vod,
                            get_chapter_start(r.vod, name),
                            Some(name),
                        ),
                        None => VodDisplay::from_vod(r.vod),
                    };
                    display.match_label = r.match_label.clone();
                    display
                })
                .collect();
            // Month grouping only makes sense for the unfiltered, chronological
            // streams view; a game filter renders a flat grid (no headers).
            if game.is_none() {
                assign_period_headers_seeded(&mut displays, sort, prev_stream_time.as_deref());
            }

            PreparedBrowse {
                games: Vec::new(),
                vods: displays,
                metadata,
                has_more,
                next_url,
                archive_min_date,
                archive_max_date,
                show_recency: false,
                show_oldest_recency: false,
                show_game_tags: game.is_none(),
                show_subtitle: game.is_none(),
            }
        }
    }
}

/// Sort spec slice for a lens, plus the lens's default sort key.
fn sort_context(is_games: bool) -> (&'static [(&'static str, &'static str, bool)], &'static str) {
    if is_games {
        (&GAME_SORTS, "most")
    } else {
        (&STREAM_SORTS, "newest")
    }
}

/// Resolve the game drilldown, lens, `is_games`, and effective sort from the
/// query, pinning the resolved sort back onto `params`. The pin matters because
/// the shared list helpers otherwise default the games lens to "recent", so the
/// grid would order by recency while the toolbar and pagination URLs say "most".
/// Shared by `browse_page` and `browse_grid` so the two never disagree.
fn resolve_browse_params(params: &mut ListQuery) -> (Option<String>, Lens, bool, String) {
    let game = params.game.clone().filter(|s| !s.is_empty());
    let lens = resolve_lens(params.lens.as_deref(), game.as_deref());
    // A stray game param on the games lens would otherwise leak into the
    // drilldown chip and clear-game URLs (every real drilldown URL carries
    // lens=streams, so this only drops junk).
    let game = if lens == Lens::Streams { game } else { None };
    let is_games = lens == Lens::Games;
    let (_, default_sort) = sort_context(is_games);
    let sort = params
        .sort
        .clone()
        .unwrap_or_else(|| default_sort.to_string());
    params.sort = Some(sort.clone());
    (game, lens, is_games, sort)
}

pub async fn browse_page(
    State(state): State<SharedState>,
    Extension(nonce): Extension<CspNonce>,
    Query(mut params): Query<ListQuery>,
) -> impl IntoResponse {
    let (game, lens, is_games, sort) = resolve_browse_params(&mut params);
    let (sort_specs, _) = sort_context(is_games);
    let (selected_sort_value, selected_sort_label) = selected_sort_option(&sort, sort_specs);

    let prepared = prepare_browse(&state, lens, game.as_deref(), &params, &sort).await;

    let date_state = date_preset_state(
        &params.from,
        &params.to,
        &prepared.archive_min_date,
        &prepared.archive_max_date,
    );

    // The lens and the game drilldown ride in hidden form inputs (see
    // list_filters.html), so both htmx filter requests and the no-JS form submit
    // keep the right lens; the form target is just the bare page route.
    let hx_get = "/browse".to_string();

    let carried = carry_filters(&params);
    let lens_games_url = format!("/browse?lens=games{carried}");
    let lens_streams_url = format!("/browse?lens=streams{carried}");
    // The game chip's ✕ clears only the game, keeping the streams lens plus any
    // search/sort/date filters (unlike "Clear filters", which drops everything).
    let clear_game_url = format!(
        "/browse?lens=streams&sort={}{carried}",
        urlencoding::encode(&sort)
    );

    let game_color_idx = game.as_deref().map(chapter_color_idx).unwrap_or(0);
    let is_filtered = game.is_some() || prepared.metadata.is_filtered;

    render_template(&BrowsePageTemplate {
        is_games_lens: is_games,
        games: prepared.games,
        vods: prepared.vods,
        game: game.clone(),
        game_color_idx,
        metadata: prepared.metadata,
        filter: ListFilterConfig {
            form_id: "browse-filters",
            toolbar_class: "vod-toolbar",
            action: hx_get.clone(),
            title: "Filter archive",
            search_placeholder: if is_games {
                "Search games..."
            } else {
                "Search streams..."
            },
            search_label: if is_games {
                "Search games".to_string()
            } else {
                "Search streams".to_string()
            },
            sort_label: "Sort",
            hx_get,
            results_id: "browse-results",
            loading_id: "browse-loading",
            sort_options: list_sort_options_grouped(selected_sort_value, sort_specs),
            selected_sort_value,
            selected_sort_label,
            archive_min_date: prepared.archive_min_date,
            archive_max_date: prepared.archive_max_date,
            date_preset: date_state.active,
            show_custom_dates: date_state.show_custom,
        },
        search: params.search.clone(),
        from: params.from.clone(),
        to: params.to.clone(),
        has_more: prepared.has_more,
        next_url: prepared.next_url,
        lens_games_url,
        lens_streams_url,
        clear_game_url,
        show_recency: prepared.show_recency,
        show_oldest_recency: prepared.show_oldest_recency,
        show_game_tags: prepared.show_game_tags,
        show_subtitle: prepared.show_subtitle,
        is_filtered,
        active_section: Section::Browse,
        nonce: nonce.0,
    })
}

pub async fn browse_grid(
    State(state): State<SharedState>,
    Query(mut params): Query<ListQuery>,
) -> Response {
    let (game, lens, is_games, sort) = resolve_browse_params(&mut params);
    let prepared = prepare_browse(&state, lens, game.as_deref(), &params, &sort).await;

    if is_games {
        render_template(&GamesGridTemplate {
            games: prepared.games,
            has_more: prepared.has_more,
            next_url: prepared.next_url,
            show_recency: prepared.show_recency,
            show_oldest_recency: prepared.show_oldest_recency,
            is_filtered: prepared.metadata.is_filtered,
        })
    } else {
        render_template(&VodsGridTemplate {
            vods: prepared.vods,
            has_more: prepared.has_more,
            next_url: prepared.next_url,
            show_game_tags: prepared.show_game_tags,
            show_subtitle: prepared.show_subtitle,
            is_filtered: prepared.metadata.is_filtered,
        })
    }
}

/// Build the `/browse` URL an old route redirects to, preserving its original
/// query string verbatim.
fn browse_redirect_target(lens: &str, game: Option<&str>, raw_query: Option<&str>) -> String {
    let mut url = format!("/browse?lens={lens}");
    if let Some(g) = game {
        url.push_str("&game=");
        url.push_str(&urlencoding::encode(g));
    }
    if let Some(q) = raw_query.filter(|q| !q.is_empty()) {
        url.push('&');
        url.push_str(q);
    }
    url
}

pub async fn games_redirect(RawQuery(query): RawQuery) -> Redirect {
    Redirect::temporary(&browse_redirect_target("games", None, query.as_deref()))
}

pub async fn streams_redirect(RawQuery(query): RawQuery) -> Redirect {
    Redirect::temporary(&browse_redirect_target("streams", None, query.as_deref()))
}

pub async fn game_redirect(Path(name): Path<String>, RawQuery(query): RawQuery) -> Redirect {
    Redirect::temporary(&browse_redirect_target(
        "streams",
        Some(&name),
        query.as_deref(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn games_lens_defaults_to_most_streams() {
        let (_, games_default) = sort_context(true);
        assert_eq!(games_default, "most");
        let (_, streams_default) = sort_context(false);
        assert_eq!(streams_default, "newest");
    }

    #[test]
    fn resolve_lens_defaults_to_games() {
        assert!(matches!(resolve_lens(None, None), Lens::Games));
        assert!(matches!(resolve_lens(Some("bogus"), None), Lens::Games));
        assert!(matches!(resolve_lens(Some("games"), None), Lens::Games));
    }

    #[test]
    fn resolve_lens_streams_explicit_or_via_game() {
        assert!(matches!(resolve_lens(Some("streams"), None), Lens::Streams));
        assert!(matches!(resolve_lens(None, Some("Sekiro")), Lens::Streams));
        // empty game does not force streams
        assert!(matches!(resolve_lens(None, Some("")), Lens::Games));
    }

    #[test]
    fn resolve_lens_explicit_games_wins_over_game_param() {
        assert!(matches!(
            resolve_lens(Some("games"), Some("Elden Ring")),
            Lens::Games
        ));
        assert!(matches!(
            resolve_lens(Some("streams"), Some("Sekiro")),
            Lens::Streams
        ));
        assert!(matches!(resolve_lens(None, Some("Sekiro")), Lens::Streams));
    }

    #[test]
    fn resolve_browse_params_drops_game_for_games_lens() {
        let mut params = ListQuery {
            lens: Some("games".into()),
            game: Some("Elden Ring".into()),
            ..Default::default()
        };
        let (game, lens, is_games, _) = resolve_browse_params(&mut params);
        assert!(game.is_none());
        assert!(matches!(lens, Lens::Games));
        assert!(is_games);
    }

    #[test]
    fn carry_filters_encodes_only_nonempty() {
        let params = ListQuery {
            search: Some("dark souls".into()),
            from: Some("2026-01-01".into()),
            to: Some(String::new()),
            ..Default::default()
        };
        let out = carry_filters(&params);
        assert!(out.contains("&search=dark%20souls"));
        assert!(out.contains("&from=2026-01-01"));
        assert!(!out.contains("to="));
    }

    #[test]
    fn browse_redirect_target_builds_expected_urls() {
        assert_eq!(
            browse_redirect_target("games", None, None),
            "/browse?lens=games"
        );
        assert_eq!(
            browse_redirect_target("streams", None, Some("from=2026-06-01")),
            "/browse?lens=streams&from=2026-06-01"
        );
        assert_eq!(
            browse_redirect_target("streams", Some("Elden Ring"), Some("sort=oldest")),
            "/browse?lens=streams&game=Elden%20Ring&sort=oldest"
        );
        assert_eq!(
            browse_redirect_target("streams", Some("C++"), None),
            "/browse?lens=streams&game=C%2B%2B"
        );
    }
}
