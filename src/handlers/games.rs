use super::{
    GAME_BATCH_SIZE, ListQuery, Section, build_next_url, filter_games, paginate, render_template,
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

pub async fn games_page(
    State(state): State<SharedState>,
    Query(params): Query<ListQuery>,
) -> impl IntoResponse {
    let games = {
        let guard = state.games.read().await;
        Arc::clone(&*guard)
    };
    let sort = params.sort.clone().unwrap_or_else(|| "most".to_string());
    let filtered = filter_games(games.to_vec(), &params);
    let page = params.page.unwrap_or(0);
    let total = filtered.len();
    let paged = paginate(filtered, page, GAME_BATCH_SIZE);
    let has_more = (page + 1) * GAME_BATCH_SIZE < total;
    let next_url = build_next_url("/games", page + 1, &params);
    let tmpl = GamesPageTemplate {
        games: paged,
        sort,
        from: params.from,
        to: params.to,
        has_more,
        next_url,
        active_section: Section::Games,
    };
    render_template(&tmpl)
}

pub async fn games_grid(
    State(state): State<SharedState>,
    Query(params): Query<ListQuery>,
) -> impl IntoResponse {
    let games = {
        let guard = state.games.read().await;
        Arc::clone(&*guard)
    };
    let filtered = filter_games(games.to_vec(), &params);
    let page = params.page.unwrap_or(0);
    let total = filtered.len();
    let paged = paginate(filtered, page, GAME_BATCH_SIZE);
    let has_more = (page + 1) * GAME_BATCH_SIZE < total;
    let next_url = build_next_url("/games", page + 1, &params);
    let tmpl = GamesGridTemplate {
        games: paged,
        has_more,
        next_url,
    };
    render_template(&tmpl)
}
