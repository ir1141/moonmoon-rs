use super::{
    GAME_BATCH_SIZE, ListQuery, Section, filter_games, paginate_with_nav, render_template,
};
use crate::SharedState;
use crate::vods::Game;
use askama::Template;
use axum::extract::{Query, State};
use axum::response::IntoResponse;
use std::sync::Arc;

#[derive(Template)]
#[template(path = "games.html")]
struct GamesPageTemplate {
    games: Vec<Game>,
    sort: String,
    from: Option<String>,
    to: Option<String>,
    has_more: bool,
    next_url: String,
    active_section: Section,
}

#[derive(Template)]
#[template(path = "games_grid.html")]
struct GamesGridTemplate {
    games: Vec<Game>,
    has_more: bool,
    next_url: String,
}

async fn prepare_games(state: &SharedState, params: &ListQuery) -> (Vec<Game>, bool, String) {
    let games = {
        let guard = state.games.read().await;
        Arc::clone(&*guard)
    };
    let filtered = filter_games(&games, params);
    paginate_with_nav(filtered, "/games", GAME_BATCH_SIZE, params)
}

pub async fn games_page(
    State(state): State<SharedState>,
    Query(params): Query<ListQuery>,
) -> impl IntoResponse {
    let sort = params.sort.clone().unwrap_or("most".to_string());
    let (paged, has_more, next_url) = prepare_games(&state, &params).await;
    render_template(&GamesPageTemplate {
        games: paged,
        sort,
        from: params.from,
        to: params.to,
        has_more,
        next_url,
        active_section: Section::Games,
    })
}

pub async fn games_grid(
    State(state): State<SharedState>,
    Query(params): Query<ListQuery>,
) -> impl IntoResponse {
    let (paged, has_more, next_url) = prepare_games(&state, &params).await;
    render_template(&GamesGridTemplate {
        games: paged,
        has_more,
        next_url,
    })
}
