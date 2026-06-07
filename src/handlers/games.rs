use super::{
    GAME_BATCH_SIZE, ListFilterConfig, ListMetadata, ListQuery, Section, archive_date_bounds,
    date_preset_state, filter_games_with_metadata, list_sort_options_grouped, paginate_with_nav,
    render_template, selected_sort_option,
};
use crate::SharedState;
use crate::middleware::CspNonce;
use crate::vods::Game;
use askama::Template;
use axum::extract::{Extension, Query, State};
use axum::response::IntoResponse;
use std::sync::Arc;

#[derive(Template)]
#[template(path = "games.html")]
struct GamesPageTemplate {
    games: Vec<Game>,
    metadata: ListMetadata,
    filter: ListFilterConfig,
    search: Option<String>,
    from: Option<String>,
    to: Option<String>,
    has_more: bool,
    next_url: String,
    show_recency: bool,
    show_oldest_recency: bool,
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

struct PreparedGames {
    games: Vec<Game>,
    metadata: ListMetadata,
    has_more: bool,
    next_url: String,
    archive_min_date: String,
    archive_max_date: String,
}

async fn prepare_games(state: &SharedState, params: &ListQuery) -> PreparedGames {
    let games = {
        let guard = state.games.read().await;
        Arc::clone(&*guard)
    };
    let vods = {
        let guard = state.vods.read().await;
        Arc::clone(&*guard)
    };
    let (archive_min_date, archive_max_date) = archive_date_bounds(&vods);
    let filtered = filter_games_with_metadata(&games, &vods, params, "/games");
    let (paged, has_more, next_url) =
        paginate_with_nav(filtered.games, "/games/grid", GAME_BATCH_SIZE, params);
    PreparedGames {
        games: paged,
        metadata: filtered.metadata,
        has_more,
        next_url,
        archive_min_date,
        archive_max_date,
    }
}

pub async fn games_page(
    State(state): State<SharedState>,
    Extension(nonce): Extension<CspNonce>,
    Query(params): Query<ListQuery>,
) -> impl IntoResponse {
    let search = params.search.clone();
    let sort = params.sort.clone().unwrap_or("recent".to_string());
    let prepared = prepare_games(&state, &params).await;
    let is_filtered = prepared.metadata.is_filtered;
    let from = params.from.clone();
    let to = params.to.clone();
    let date_state = date_preset_state(
        &from,
        &to,
        &prepared.archive_min_date,
        &prepared.archive_max_date,
    );
    let sort_specs = [
        ("recent", "Latest stream", false),
        ("oldest", "Oldest stream", false),
        ("most", "Most streams", true),
        ("fewest", "Fewest streams", false),
        ("az", "A - Z", true),
        ("za", "Z - A", false),
    ];
    let (selected_sort_value, selected_sort_label) = selected_sort_option(&sort, &sort_specs);
    render_template(&GamesPageTemplate {
        games: prepared.games,
        metadata: prepared.metadata,
        filter: ListFilterConfig {
            form_id: "games-filters",
            toolbar_class: "games-toolbar",
            action: "/games".to_string(),
            title: "Filter games",
            search_placeholder: "Search games...",
            search_label: "Search games".to_string(),
            sort_label: "Sort games",
            hx_get: "/games".to_string(),
            results_id: "games-results",
            loading_id: "games-loading",
            sort_options: list_sort_options_grouped(selected_sort_value, &sort_specs),
            selected_sort_value,
            selected_sort_label,
            archive_min_date: prepared.archive_min_date,
            archive_max_date: prepared.archive_max_date,
            date_preset: date_state.active,
            show_custom_dates: date_state.show_custom,
        },
        search,
        from,
        to,
        has_more: prepared.has_more,
        next_url: prepared.next_url,
        show_recency: matches!(selected_sort_value, "recent" | "oldest"),
        show_oldest_recency: selected_sort_value == "oldest",
        is_filtered,
        active_section: Section::Games,
        nonce: nonce.0,
    })
}

pub async fn games_grid(
    State(state): State<SharedState>,
    Query(params): Query<ListQuery>,
) -> impl IntoResponse {
    let sort = params.sort.as_deref().unwrap_or("recent");
    let prepared = prepare_games(&state, &params).await;
    let is_filtered = prepared.metadata.is_filtered;
    render_template(&GamesGridTemplate {
        games: prepared.games,
        has_more: prepared.has_more,
        next_url: prepared.next_url,
        show_recency: matches!(sort, "recent" | "oldest"),
        show_oldest_recency: sort == "oldest",
        is_filtered,
    })
}
