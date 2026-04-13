use super::{
    ListQuery, VOD_BATCH_SIZE, VodDisplay, build_next_url, filter_vod_displays, find_game_image,
    get_chapter_start, paginate, render_template, vod_has_game,
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
}

pub async fn game_vods_page(
    State(state): State<SharedState>,
    Path(name): Path<String>,
    Query(params): Query<ListQuery>,
) -> impl IntoResponse {
    let (vods, games) = {
        let v = state.vods.read().await;
        let g = state.games.read().await;
        (Arc::clone(&*v), Arc::clone(&*g))
    };

    let game_image = find_game_image(&games, &name);

    let sort = params.sort.clone().unwrap_or_else(|| "newest".to_string());
    let mut displays: Vec<VodDisplay> = vods
        .iter()
        .filter(|v| vod_has_game(v, &name))
        .map(|v| {
            let mut d = VodDisplay::from_vod(v);
            d.chapter_start = get_chapter_start(v, &name);
            d
        })
        .collect();
    let vod_count = displays.len();
    filter_vod_displays(&mut displays, &params);

    let page = params.page.unwrap_or(0);
    let total = displays.len();
    let paged = paginate(displays, page, VOD_BATCH_SIZE);
    let has_more = (page + 1) * VOD_BATCH_SIZE < total;
    let next_url = build_next_url(
        &format!("/game/{}/vods", urlencoding::encode(&name)),
        page + 1,
        &params,
    );

    let tmpl = VodsPageTemplate {
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
    };
    render_template(&tmpl)
}

pub async fn game_vods_grid(
    State(state): State<SharedState>,
    Path(name): Path<String>,
    Query(params): Query<ListQuery>,
) -> impl IntoResponse {
    let vods = {
        let guard = state.vods.read().await;
        Arc::clone(&*guard)
    };

    let mut displays: Vec<VodDisplay> = vods
        .iter()
        .filter(|v| vod_has_game(v, &name))
        .map(|v| {
            let mut d = VodDisplay::from_vod(v);
            d.chapter_start = get_chapter_start(v, &name);
            d
        })
        .collect();
    filter_vod_displays(&mut displays, &params);

    let page = params.page.unwrap_or(0);
    let total = displays.len();
    let paged = paginate(displays, page, VOD_BATCH_SIZE);
    let has_more = (page + 1) * VOD_BATCH_SIZE < total;
    let next_url = build_next_url(
        &format!("/game/{}/vods", urlencoding::encode(&name)),
        page + 1,
        &params,
    );

    let tmpl = VodsGridTemplate {
        vods: paged,
        has_more,
        next_url,
        show_game_tags: false,
    };
    render_template(&tmpl)
}

pub async fn all_streams_page(
    State(state): State<SharedState>,
    Query(params): Query<ListQuery>,
) -> impl IntoResponse {
    let vods = {
        let guard = state.vods.read().await;
        Arc::clone(&*guard)
    };
    let total_count = vods.len();
    let sort = params.sort.clone().unwrap_or_else(|| "newest".to_string());

    let mut displays: Vec<VodDisplay> = vods.iter().map(VodDisplay::from_vod).collect();
    filter_vod_displays(&mut displays, &params);

    let page = params.page.unwrap_or(0);
    let total = displays.len();
    let paged = paginate(displays, page, VOD_BATCH_SIZE);
    let has_more = (page + 1) * VOD_BATCH_SIZE < total;
    let next_url = build_next_url("/streams/vods", page + 1, &params);

    let tmpl = AllStreamsPageTemplate {
        total_count,
        vods: paged,
        sort,
        from: params.from,
        to: params.to,
        has_more,
        next_url,
        show_game_tags: true,
    };
    render_template(&tmpl)
}

pub async fn all_streams_grid(
    State(state): State<SharedState>,
    Query(params): Query<ListQuery>,
) -> impl IntoResponse {
    let vods = {
        let guard = state.vods.read().await;
        Arc::clone(&*guard)
    };

    let mut displays: Vec<VodDisplay> = vods.iter().map(VodDisplay::from_vod).collect();
    filter_vod_displays(&mut displays, &params);

    let page = params.page.unwrap_or(0);
    let total = displays.len();
    let paged = paginate(displays, page, VOD_BATCH_SIZE);
    let has_more = (page + 1) * VOD_BATCH_SIZE < total;
    let next_url = build_next_url("/streams/vods", page + 1, &params);

    let tmpl = VodsGridTemplate {
        vods: paged,
        has_more,
        next_url,
        show_game_tags: true,
    };
    render_template(&tmpl)
}
