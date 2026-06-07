use super::{
    ListFilterConfig, ListMetadata, ListQuery, Section, VOD_BATCH_SIZE, VodDisplay,
    archive_date_bounds, assign_period_headers, date_preset_state,
    filter_vod_displays_with_metadata, find_game_image, get_chapter_start,
    list_sort_options_grouped, paginate_with_nav, render_template, selected_sort_option,
    vod_has_game,
};
use crate::SharedState;
use crate::middleware::CspNonce;
use askama::Template;
use axum::extract::{Extension, Path, Query, State};
use axum::response::IntoResponse;
use std::sync::Arc;

#[derive(Template)]
#[template(path = "vods.html")]
struct VodsPageTemplate {
    game_name: String,
    game_image: Option<String>,
    metadata: ListMetadata,
    filter: ListFilterConfig,
    search: Option<String>,
    vods: Vec<VodDisplay>,
    from: Option<String>,
    to: Option<String>,
    has_more: bool,
    next_url: String,
    show_game_tags: bool,
    is_filtered: bool,
    active_section: Section,
    nonce: String,
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

#[derive(Template)]
#[template(path = "all_streams.html")]
struct AllStreamsPageTemplate {
    metadata: ListMetadata,
    filter: ListFilterConfig,
    search: Option<String>,
    vods: Vec<VodDisplay>,
    from: Option<String>,
    to: Option<String>,
    has_more: bool,
    next_url: String,
    show_game_tags: bool,
    is_filtered: bool,
    active_section: Section,
    nonce: String,
}

struct PreparedVodList {
    vods: Vec<VodDisplay>,
    metadata: ListMetadata,
    has_more: bool,
    next_url: String,
    archive_min_date: String,
    archive_max_date: String,
}

async fn prepare_game_vods(
    state: &SharedState,
    name: &str,
    params: &ListQuery,
    sort: &str,
) -> PreparedVodList {
    let vods = {
        let guard = state.vods.read().await;
        Arc::clone(&*guard)
    };
    let (archive_min_date, archive_max_date) = archive_date_bounds(&vods);
    let displays: Vec<VodDisplay> = vods
        .iter()
        .filter(|v| vod_has_game(v, name))
        .map(|v| VodDisplay::from_vod_with(v, get_chapter_start(v, name), Some(name)))
        .collect();
    let page_base = format!("/game/{}", urlencoding::encode(name));
    let grid_base = format!("{page_base}/vods");
    let filtered = filter_vod_displays_with_metadata(displays, params, &page_base);
    let mut displays = filtered.vods;
    assign_period_headers(&mut displays, sort);
    let (paged, has_more, next_url) =
        paginate_with_nav(displays, &grid_base, VOD_BATCH_SIZE, params);
    PreparedVodList {
        vods: paged,
        metadata: filtered.metadata,
        has_more,
        next_url,
        archive_min_date,
        archive_max_date,
    }
}

pub async fn game_vods_page(
    State(state): State<SharedState>,
    Extension(nonce): Extension<CspNonce>,
    Path(name): Path<String>,
    Query(params): Query<ListQuery>,
) -> impl IntoResponse {
    let game_image = {
        let guard = state.games.read().await;
        find_game_image(&guard, &name)
    };
    let search = params.search.clone();
    let from = params.from.clone();
    let to = params.to.clone();
    let sort = params.sort.clone().unwrap_or("newest".to_string());
    let page_base = format!("/game/{}", urlencoding::encode(&name));
    let game_search_label = format!("Search {name} streams");
    let prepared = prepare_game_vods(&state, &name, &params, &sort).await;
    let is_filtered = prepared.metadata.is_filtered;
    let date_state = date_preset_state(
        &from,
        &to,
        &prepared.archive_min_date,
        &prepared.archive_max_date,
    );
    let sort_specs = [
        ("newest", "Newest First", false),
        ("oldest", "Oldest First", false),
        ("longest", "Longest", true),
        ("shortest", "Shortest", false),
    ];
    let (selected_sort_value, selected_sort_label) = selected_sort_option(&sort, &sort_specs);

    render_template(&VodsPageTemplate {
        game_name: name,
        game_image,
        metadata: prepared.metadata,
        filter: ListFilterConfig {
            form_id: "vod-filters",
            toolbar_class: "vod-toolbar",
            action: page_base.clone(),
            title: "Filter streams",
            search_placeholder: "Search streams...",
            search_label: game_search_label,
            sort_label: "Sort streams",
            hx_get: page_base,
            results_id: "vod-results",
            loading_id: "vod-loading",
            sort_options: list_sort_options_grouped(selected_sort_value, &sort_specs),
            selected_sort_value,
            selected_sort_label,
            archive_min_date: prepared.archive_min_date,
            archive_max_date: prepared.archive_max_date,
            date_preset: date_state.active,
            show_custom_dates: date_state.show_custom,
        },
        search,
        vods: prepared.vods,
        from,
        to,
        has_more: prepared.has_more,
        next_url: prepared.next_url,
        show_game_tags: false,
        is_filtered,
        active_section: Section::None,
        nonce: nonce.0,
    })
}

pub async fn game_vods_grid(
    State(state): State<SharedState>,
    Path(name): Path<String>,
    Query(params): Query<ListQuery>,
) -> impl IntoResponse {
    let sort = params.sort.clone().unwrap_or("newest".to_string());
    let prepared = prepare_game_vods(&state, &name, &params, &sort).await;

    render_template(&VodsGridTemplate {
        is_filtered: prepared.metadata.is_filtered,
        vods: prepared.vods,
        has_more: prepared.has_more,
        next_url: prepared.next_url,
        show_game_tags: false,
    })
}

async fn prepare_all_streams(state: &SharedState, params: &ListQuery) -> PreparedVodList {
    let vods = {
        let guard = state.vods.read().await;
        Arc::clone(&*guard)
    };
    let (archive_min_date, archive_max_date) = archive_date_bounds(&vods);
    let displays: Vec<VodDisplay> = vods.iter().map(VodDisplay::from_vod).collect();
    let filtered = filter_vod_displays_with_metadata(displays, params, "/streams");
    let (paged, has_more, next_url) =
        paginate_with_nav(filtered.vods, "/streams/vods", VOD_BATCH_SIZE, params);
    PreparedVodList {
        vods: paged,
        metadata: filtered.metadata,
        has_more,
        next_url,
        archive_min_date,
        archive_max_date,
    }
}

pub async fn all_streams_page(
    State(state): State<SharedState>,
    Extension(nonce): Extension<CspNonce>,
    Query(params): Query<ListQuery>,
) -> impl IntoResponse {
    let search = params.search.clone();
    let from = params.from.clone();
    let to = params.to.clone();
    let sort = params.sort.clone().unwrap_or("newest".to_string());
    let prepared = prepare_all_streams(&state, &params).await;
    let is_filtered = prepared.metadata.is_filtered;
    let date_state = date_preset_state(
        &from,
        &to,
        &prepared.archive_min_date,
        &prepared.archive_max_date,
    );
    let sort_specs = [
        ("newest", "Newest First", false),
        ("oldest", "Oldest First", false),
        ("longest", "Longest", true),
        ("shortest", "Shortest", false),
    ];
    let (selected_sort_value, selected_sort_label) = selected_sort_option(&sort, &sort_specs);

    render_template(&AllStreamsPageTemplate {
        metadata: prepared.metadata,
        filter: ListFilterConfig {
            form_id: "vod-filters",
            toolbar_class: "vod-toolbar",
            action: "/streams".to_string(),
            title: "Filter streams",
            search_placeholder: "Search streams...",
            search_label: "Search streams".to_string(),
            sort_label: "Sort streams",
            hx_get: "/streams".to_string(),
            results_id: "vod-results",
            loading_id: "vod-loading",
            sort_options: list_sort_options_grouped(selected_sort_value, &sort_specs),
            selected_sort_value,
            selected_sort_label,
            archive_min_date: prepared.archive_min_date,
            archive_max_date: prepared.archive_max_date,
            date_preset: date_state.active,
            show_custom_dates: date_state.show_custom,
        },
        search,
        vods: prepared.vods,
        from,
        to,
        has_more: prepared.has_more,
        next_url: prepared.next_url,
        show_game_tags: true,
        is_filtered,
        active_section: Section::Streams,
        nonce: nonce.0,
    })
}

pub async fn all_streams_grid(
    State(state): State<SharedState>,
    Query(params): Query<ListQuery>,
) -> impl IntoResponse {
    let prepared = prepare_all_streams(&state, &params).await;

    render_template(&VodsGridTemplate {
        is_filtered: prepared.metadata.is_filtered,
        vods: prepared.vods,
        has_more: prepared.has_more,
        next_url: prepared.next_url,
        show_game_tags: true,
    })
}
