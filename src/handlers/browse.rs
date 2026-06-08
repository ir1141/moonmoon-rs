use super::{
    GAME_BATCH_SIZE, ListFilterConfig, ListMetadata, ListQuery, Section, VOD_BATCH_SIZE,
    VodDisplay, archive_date_bounds, assign_period_headers, date_preset_state,
    filter_games_with_metadata, filter_vod_displays_with_metadata, get_chapter_start,
    list_sort_options_grouped, paginate_with_nav, render_template, selected_sort_option,
    vod_has_game,
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

/// Resolve the active lens. A non-empty `game` drilldown forces the streams
/// lens; otherwise `lens=streams` selects streams and everything else (missing,
/// unknown, or `games`) defaults to the games overview.
fn resolve_lens(lens: Option<&str>, game: Option<&str>) -> Lens {
    if game.is_some_and(|g| !g.is_empty()) {
        return Lens::Streams;
    }
    match lens {
        Some("streams") => Lens::Streams,
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
    show_recency: bool,
    show_oldest_recency: bool,
    show_game_tags: bool,
    is_filtered: bool,
    active_section: Section,
    nonce: String,
}

#[derive(Template)]
#[template(path = "games_grid.html")]
struct GamesGridTemplate {
    games: Vec<Game>,
    has_more: bool,
    next_url: String,
    show_recency: bool,
    show_oldest_recency: bool,
    is_filtered: bool,
}

#[derive(Template)]
#[template(path = "vods_grid.html")]
struct VodsGridTemplate {
    vods: Vec<VodDisplay>,
    has_more: bool,
    next_url: String,
    show_game_tags: bool,
    is_filtered: bool,
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
    let (archive_min_date, archive_max_date) = archive_date_bounds(&vods);

    match lens {
        Lens::Games => {
            let games = {
                let guard = state.games.read().await;
                Arc::clone(&*guard)
            };
            let filtered = filter_games_with_metadata(&games, &vods, params, "/browse?lens=games");
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
            }
        }
        Lens::Streams => {
            let displays: Vec<VodDisplay> = match game {
                Some(name) => vods
                    .iter()
                    .filter(|v| vod_has_game(v, name))
                    .map(|v| VodDisplay::from_vod_with(v, get_chapter_start(v, name), Some(name)))
                    .collect(),
                None => vods.iter().map(VodDisplay::from_vod).collect(),
            };
            let filtered =
                filter_vod_displays_with_metadata(displays, params, "/browse?lens=streams");
            let mut displays = filtered.vods;
            assign_period_headers(&mut displays, sort);
            let (paged, has_more, next_url) =
                paginate_with_nav(displays, "/browse/grid", VOD_BATCH_SIZE, params);
            PreparedBrowse {
                games: Vec::new(),
                vods: paged,
                metadata: filtered.metadata,
                has_more,
                next_url,
                archive_min_date,
                archive_max_date,
                show_recency: false,
                show_oldest_recency: false,
                show_game_tags: game.is_none(),
            }
        }
    }
}

/// Sort spec slice for a lens, plus the lens's default sort key.
fn sort_context(is_games: bool) -> (&'static [(&'static str, &'static str, bool)], &'static str) {
    if is_games {
        (&GAME_SORTS, "recent")
    } else {
        (&STREAM_SORTS, "newest")
    }
}

pub async fn browse_page(
    State(state): State<SharedState>,
    Extension(nonce): Extension<CspNonce>,
    Query(params): Query<ListQuery>,
) -> impl IntoResponse {
    let game = params.game.clone().filter(|s| !s.is_empty());
    let lens = resolve_lens(params.lens.as_deref(), game.as_deref());
    let is_games = lens == Lens::Games;
    let lens_value = if is_games { "games" } else { "streams" };
    let (sort_specs, default_sort) = sort_context(is_games);
    let sort = params
        .sort
        .clone()
        .unwrap_or_else(|| default_sort.to_string());
    let (selected_sort_value, selected_sort_label) = selected_sort_option(&sort, sort_specs);

    let prepared = prepare_browse(&state, lens, game.as_deref(), &params, &sort).await;

    let date_state = date_preset_state(
        &params.from,
        &params.to,
        &prepared.archive_min_date,
        &prepared.archive_max_date,
    );

    // Filter requests (search/sort/date) and the no-JS form action carry the
    // lens (and game) in the URL so the server keeps rendering the right lens.
    let game_param = match game.as_deref() {
        Some(name) => format!("&game={}", urlencoding::encode(name)),
        None => String::new(),
    };
    let hx_get = format!("/browse?lens={lens_value}{game_param}");

    let carried = carry_filters(&params);
    let lens_games_url = format!("/browse?lens=games{carried}");
    let lens_streams_url = format!("/browse?lens=streams{carried}");

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
        show_recency: prepared.show_recency,
        show_oldest_recency: prepared.show_oldest_recency,
        show_game_tags: prepared.show_game_tags,
        is_filtered,
        active_section: Section::Browse,
        nonce: nonce.0,
    })
}

pub async fn browse_grid(
    State(state): State<SharedState>,
    Query(params): Query<ListQuery>,
) -> Response {
    let game = params.game.clone().filter(|s| !s.is_empty());
    let lens = resolve_lens(params.lens.as_deref(), game.as_deref());
    let is_games = lens == Lens::Games;
    let (_, default_sort) = sort_context(is_games);
    let sort = params
        .sort
        .clone()
        .unwrap_or_else(|| default_sort.to_string());
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
    fn resolve_lens_defaults_to_games() {
        assert!(matches!(resolve_lens(None, None), Lens::Games));
        assert!(matches!(resolve_lens(Some("bogus"), None), Lens::Games));
        assert!(matches!(resolve_lens(Some("games"), None), Lens::Games));
    }

    #[test]
    fn resolve_lens_streams_explicit_or_via_game() {
        assert!(matches!(resolve_lens(Some("streams"), None), Lens::Streams));
        assert!(matches!(
            resolve_lens(Some("games"), Some("Elden Ring")),
            Lens::Streams
        ));
        assert!(matches!(resolve_lens(None, Some("Sekiro")), Lens::Streams));
        // empty game does not force streams
        assert!(matches!(resolve_lens(None, Some("")), Lens::Games));
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
