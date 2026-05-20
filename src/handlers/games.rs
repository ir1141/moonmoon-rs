use super::{
    GAME_BATCH_SIZE, ListFilterConfig, ListMetadata, ListQuery, Section,
    filter_games_with_metadata, list_sort_options, paginate_with_nav, render_template,
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
    is_filtered: bool,
}

struct PreparedGames {
    games: Vec<Game>,
    metadata: ListMetadata,
    has_more: bool,
    next_url: String,
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
    let filtered = filter_games_with_metadata(&games, &vods, params, "/games");
    let (paged, has_more, next_url) =
        paginate_with_nav(filtered.games, "/games/grid", GAME_BATCH_SIZE, params);
    PreparedGames {
        games: paged,
        metadata: filtered.metadata,
        has_more,
        next_url,
    }
}

pub async fn games_page(
    State(state): State<SharedState>,
    Extension(nonce): Extension<CspNonce>,
    Query(params): Query<ListQuery>,
) -> impl IntoResponse {
    let search = params.search.clone();
    let sort = params.sort.clone().unwrap_or("most".to_string());
    let prepared = prepare_games(&state, &params).await;
    let is_filtered = prepared.metadata.is_filtered;
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
            sort_options: list_sort_options(
                &sort,
                &[
                    ("most", "Most streams"),
                    ("fewest", "Fewest streams"),
                    ("az", "A - Z"),
                    ("za", "Z - A"),
                ],
            ),
        },
        search,
        from: params.from,
        to: params.to,
        has_more: prepared.has_more,
        next_url: prepared.next_url,
        is_filtered,
        active_section: Section::Games,
        nonce: nonce.0,
    })
}

pub async fn games_grid(
    State(state): State<SharedState>,
    Query(params): Query<ListQuery>,
) -> impl IntoResponse {
    let prepared = prepare_games(&state, &params).await;
    let is_filtered = prepared.metadata.is_filtered;
    render_template(&GamesGridTemplate {
        games: prepared.games,
        has_more: prepared.has_more,
        next_url: prepared.next_url,
        is_filtered,
    })
}
