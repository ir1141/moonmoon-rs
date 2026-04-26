use super::{
    ListQuery, Section, VOD_BATCH_SIZE, VodDisplay, assign_period_headers, filter_vod_displays,
    find_game_image, get_chapter_start, paginate_with_nav, render_template, vod_has_game,
};
use crate::SharedState;
use askama::Template;
use axum::extract::{Path, Query, State};
use axum::response::IntoResponse;
use std::sync::Arc;

#[derive(Template)]
#[template(path = "vods.html")]
struct VodsPageTemplate {
    game_name: String,
    game_image: Option<String>,
    vod_count: usize,
    vods: Vec<VodDisplay>,
    sort: String,
    from: Option<String>,
    to: Option<String>,
    has_more: bool,
    next_url: String,
    show_game_tags: bool,
    active_section: Section,
}

#[derive(Template)]
#[template(path = "vods_grid.html")]
struct VodsGridTemplate {
    vods: Vec<VodDisplay>,
    has_more: bool,
    next_url: String,
    show_game_tags: bool,
}

#[derive(Template)]
#[template(path = "all_streams.html")]
struct AllStreamsPageTemplate {
    total_count: usize,
    vods: Vec<VodDisplay>,
    sort: String,
    from: Option<String>,
    to: Option<String>,
    has_more: bool,
    next_url: String,
    show_game_tags: bool,
    active_section: Section,
}

async fn prepare_game_vods(
    state: &SharedState,
    name: &str,
    params: &ListQuery,
    sort: &str,
) -> (Vec<VodDisplay>, usize, bool, String) {
    let vods = {
        let guard = state.vods.read().await;
        Arc::clone(&*guard)
    };
    let mut displays: Vec<VodDisplay> = vods
        .iter()
        .filter(|v| vod_has_game(v, name))
        .map(|v| VodDisplay::from_vod_with(v, get_chapter_start(v, name), Some(name)))
        .collect();
    let vod_count = displays.len();
    filter_vod_displays(&mut displays, params);
    assign_period_headers(&mut displays, sort);
    let base = format!("/game/{}/vods", urlencoding::encode(name));
    let (paged, has_more, next_url) = paginate_with_nav(displays, &base, VOD_BATCH_SIZE, params);
    (paged, vod_count, has_more, next_url)
}

pub async fn game_vods_page(
    State(state): State<SharedState>,
    Path(name): Path<String>,
    Query(params): Query<ListQuery>,
) -> impl IntoResponse {
    let game_image = {
        let guard = state.games.read().await;
        find_game_image(&guard, &name)
    };
    let sort = params.sort.clone().unwrap_or("newest".to_string());
    let (paged, vod_count, has_more, next_url) =
        prepare_game_vods(&state, &name, &params, &sort).await;

    render_template(&VodsPageTemplate {
        game_name: name,
        game_image,
        vod_count,
        vods: paged,
        sort,
        from: params.from,
        to: params.to,
        has_more,
        next_url,
        show_game_tags: false,
        active_section: Section::None,
    })
}

pub async fn game_vods_grid(
    State(state): State<SharedState>,
    Path(name): Path<String>,
    Query(params): Query<ListQuery>,
) -> impl IntoResponse {
    let sort = params.sort.clone().unwrap_or("newest".to_string());
    let (paged, _, has_more, next_url) = prepare_game_vods(&state, &name, &params, &sort).await;

    render_template(&VodsGridTemplate {
        vods: paged,
        has_more,
        next_url,
        show_game_tags: false,
    })
}

async fn prepare_all_streams(
    state: &SharedState,
    params: &ListQuery,
) -> (Vec<VodDisplay>, usize, bool, String) {
    let vods = {
        let guard = state.vods.read().await;
        Arc::clone(&*guard)
    };
    let total_count = vods.len();
    let mut displays: Vec<VodDisplay> = vods.iter().map(VodDisplay::from_vod).collect();
    filter_vod_displays(&mut displays, params);
    let (paged, has_more, next_url) =
        paginate_with_nav(displays, "/streams/vods", VOD_BATCH_SIZE, params);
    (paged, total_count, has_more, next_url)
}

pub async fn all_streams_page(
    State(state): State<SharedState>,
    Query(params): Query<ListQuery>,
) -> impl IntoResponse {
    let sort = params.sort.clone().unwrap_or("newest".to_string());
    let (paged, total_count, has_more, next_url) = prepare_all_streams(&state, &params).await;

    render_template(&AllStreamsPageTemplate {
        total_count,
        vods: paged,
        sort,
        from: params.from,
        to: params.to,
        has_more,
        next_url,
        show_game_tags: true,
        active_section: Section::Streams,
    })
}

pub async fn all_streams_grid(
    State(state): State<SharedState>,
    Query(params): Query<ListQuery>,
) -> impl IntoResponse {
    let (paged, _, has_more, next_url) = prepare_all_streams(&state, &params).await;

    render_template(&VodsGridTemplate {
        vods: paged,
        has_more,
        next_url,
        show_game_tags: true,
    })
}
