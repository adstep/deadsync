//! Pure render path for the title menu — the hot-reload target.
//!
//! Every value this code needs that used to be a process-global read now comes
//! in through `HostContext` (resolved host-side by `super::build_host_context`).
//! This module performs **no** engine-global read of its own and keeps no
//! persistent state (the render caches live on the host-owned `State`).
//!
//! KNOWN IMPURITY (intentionally deferred): the component builders called below
//! (`visual_style_bg`, `logo`, `menu_list`, `screen_bar`) and the `act!` text
//! actors still resolve textures/fonts through engine globals
//! (`assets::texture_registry_generation()`) and bake `&'static str`
//! texture/font keys. In-process this is identical to today. Before the runtime
//! can ever *unload* an old cdylib it must be addressed — favored approach:
//! render emits actors carrying texture *keys* only and the host re-resolves
//! handles after return (keeps asset lifetime, registry generation and
//! `&'static` keys host-owned).

use deadsync::act;
use deadsync::engine::present::actors::{Actor, TextAlign};
use deadsync::engine::present::color;
use deadsync::screens::components::menu::logo::{self, LogoParams};
use deadsync::screens::components::menu::menu_list::{self};
use deadsync::screens::components::shared::{screen_bar, visual_style_bg};
use deadsync::screens::menu::state::{
    ArrowCloudStatusKey, GrooveStatusKey, HostContext, State, StatusTextCache,
};
use deadsync_online::arrowcloud::ConnectionError as ArrowCloudError;
use deadsync_online::groovestats::ConnectionError as GrooveStatsError;
use std::sync::Arc;

const NORMAL_COLOR_HEX: &str = "#888888";

const MENU_BELOW_LOGO: f32 = 29.0;
const MENU_ROW_SPACING: f32 = 28.0;

const INFO_PX: f32 = 15.0;
const INFO_GAP: f32 = 5.0;
const INFO_MARGIN_ABOVE: f32 = 20.0;
const STATUS_BASE_X: f32 = 10.0;
const STATUS_BASE_Y: f32 = 15.0;
const STATUS_ZOOM: f32 = 0.8;
const STATUS_LINE_HEIGHT: f32 = 18.0;
const STATUS_BLOCK_GAP: f32 = 6.0;

pub fn clear_render_cache(state: &State) {
    *state.info_text_cache.borrow_mut() = None;
    *state.groovestats_text_cache.borrow_mut() = None;
    *state.arrowcloud_text_cache.borrow_mut() = None;
}

fn sync_i18n_cache(state: &State, revision: u64) {
    if state.i18n_revision.get() == revision {
        return;
    }
    clear_render_cache(state);
    state.i18n_revision.set(revision);
}

fn groove_error_text(ctx: &HostContext, kind: GrooveStatsError) -> Arc<str> {
    match kind {
        GrooveStatsError::Disabled => (ctx.tr)("Menu", "Disabled"),
        GrooveStatsError::MachineOffline => (ctx.tr)("Menu", "MachineOffline"),
        GrooveStatsError::CannotConnect => (ctx.tr)("Menu", "CannotConnect"),
        GrooveStatsError::TimedOut => (ctx.tr)("Menu", "TimedOut"),
        GrooveStatsError::InvalidResponse => (ctx.tr)("Menu", "FailedToLoad"),
    }
}

fn arrowcloud_error_text(ctx: &HostContext, kind: ArrowCloudError) -> Arc<str> {
    match kind {
        ArrowCloudError::Disabled => (ctx.tr)("Menu", "Disabled"),
        ArrowCloudError::TimedOut => (ctx.tr)("Menu", "TimedOut"),
        ArrowCloudError::HostBlocked => (ctx.tr)("Menu", "HostBlocked"),
        ArrowCloudError::CannotConnect => (ctx.tr)("Menu", "CannotConnect"),
    }
}

#[inline(always)]
fn menu_info_text(state: &State, ctx: &HostContext) -> Arc<str> {
    if let Some((cached_tag, text)) = state.info_text_cache.borrow().as_ref()
        && cached_tag == &ctx.banner_tag
    {
        return text.clone();
    }

    let mut version_line =
        (ctx.tr_fmt)("Menu", "VersionLine", &[("version", ctx.version.as_ref())]).to_string();
    if let Some(tag) = ctx.banner_tag.as_deref() {
        let suffix = (ctx.tr_fmt)("Menu", "UpdateAvailableSuffix", &[("version", tag)]);
        version_line.push(' ');
        version_line.push_str(&suffix);
    }
    let songs = ctx.song_count.to_string();
    let packs = ctx.pack_count.to_string();
    let courses = ctx.course_count.to_string();
    let summary = (ctx.tr_fmt)(
        "Menu",
        "SongSummary",
        &[("songs", &songs), ("packs", &packs), ("courses", &courses)],
    );
    let text = Arc::<str>::from(format!("{version_line}\n{summary}"));
    *state.info_text_cache.borrow_mut() = Some((ctx.banner_tag.clone(), text.clone()));
    text
}

#[inline(always)]
fn groove_service_name(ctx: &HostContext, boogie: bool) -> Arc<str> {
    if boogie {
        (ctx.tr)("Menu", "BoogieStatsName")
    } else {
        (ctx.tr)("Menu", "GrooveStatsName")
    }
}

fn build_groovestats_text(
    ctx: &HostContext,
    key: GrooveStatusKey,
) -> StatusTextCache<GrooveStatusKey, 3> {
    let mut lines = [None, None, None];
    let (main, line_count) = match key {
        GrooveStatusKey::Pending { boogie } => {
            let service = groove_service_name(ctx, boogie);
            (
                (ctx.tr_fmt)("Menu", "ServicePending", &[("service", service.as_ref())]),
                0,
            )
        }
        GrooveStatusKey::Error { boogie, kind } => {
            lines[0] = Some(groove_error_text(ctx, kind));
            if kind == GrooveStatsError::Disabled {
                ((ctx.tr)("Menu", "GrooveStatsDisabled"), 1)
            } else {
                let service = groove_service_name(ctx, boogie);
                (
                    (ctx.tr_fmt)(
                        "Menu",
                        "ServiceNotConnected",
                        &[("service", service.as_ref())],
                    ),
                    1,
                )
            }
        }
        GrooveStatusKey::Connected {
            boogie,
            disabled_mask,
        } => {
            if disabled_mask == 0 {
                let service = groove_service_name(ctx, boogie);
                (
                    (ctx.tr_fmt)("Menu", "ServiceConnected", &[("service", service.as_ref())]),
                    0,
                )
            } else if disabled_mask == 0b111 {
                ((ctx.tr)("Menu", "GrooveStatsDisabled"), 0)
            } else {
                let mut line_count = 0;
                if disabled_mask & 0b001 != 0 {
                    lines[line_count] = Some((ctx.tr)("Menu", "GetScoresDisabled"));
                    line_count += 1;
                }
                if disabled_mask & 0b010 != 0 {
                    lines[line_count] = Some((ctx.tr)("Menu", "LeaderboardDisabled"));
                    line_count += 1;
                }
                if disabled_mask & 0b100 != 0 {
                    lines[line_count] = Some((ctx.tr)("Menu", "AutoSubmitDisabled"));
                    line_count += 1;
                }
                ((ctx.tr)("Menu", "GrooveStatsWarn"), line_count)
            }
        }
    };
    StatusTextCache {
        key,
        main,
        lines,
        line_count,
    }
}

fn groovestats_text(state: &State, ctx: &HostContext) -> StatusTextCache<GrooveStatusKey, 3> {
    let key = ctx.groove_key;
    if let Some(cache) = state.groovestats_text_cache.borrow().as_ref()
        && cache.key == key
    {
        return cache.clone();
    }
    let cache = build_groovestats_text(ctx, key);
    *state.groovestats_text_cache.borrow_mut() = Some(cache.clone());
    cache
}

fn build_arrowcloud_text(
    ctx: &HostContext,
    key: ArrowCloudStatusKey,
) -> StatusTextCache<ArrowCloudStatusKey, 1> {
    let mut lines = [None];
    let (main, line_count) = match key {
        ArrowCloudStatusKey::Pending => ((ctx.tr)("Menu", "ArrowCloudPending"), 0),
        ArrowCloudStatusKey::Connected => ((ctx.tr)("Menu", "ArrowCloudConnected"), 0),
        ArrowCloudStatusKey::Error(kind) => {
            lines[0] = Some(arrowcloud_error_text(ctx, kind));
            ((ctx.tr)("Menu", "ArrowCloudDisabled"), 1)
        }
    };
    StatusTextCache {
        key,
        main,
        lines,
        line_count,
    }
}

fn arrowcloud_text(state: &State, ctx: &HostContext) -> StatusTextCache<ArrowCloudStatusKey, 1> {
    let key = ctx.arrowcloud_key;
    if let Some(cache) = state.arrowcloud_text_cache.borrow().as_ref()
        && cache.key == key
    {
        return cache.clone();
    }
    let cache = build_arrowcloud_text(ctx, key);
    *state.arrowcloud_text_cache.borrow_mut() = Some(cache.clone());
    cache
}

#[inline(always)]
fn status_text_actor(
    text: Arc<str>,
    align_x: f32,
    x: f32,
    y: f32,
    zoom: f32,
    alpha: f32,
    align_text: TextAlign,
) -> Actor {
    // TODO: `font("miso")` bakes a hot-owned `&'static str`; route through a
    // lib-owned font-key value on `HostContext` before old cdylibs are unloaded.
    let mut actor = act!(text:
        font("miso"):
        settext(text):
        align(align_x, 0.0):
        xy(x, y):
        zoom(zoom):
        z(200)
    );
    if let Actor::Text {
        color,
        align_text: text_align,
        ..
    } = &mut actor
    {
        color[3] = alpha;
        *text_align = align_text;
    }
    actor
}

pub fn get_actors(state: &State, ctx: &HostContext, alpha_multiplier: f32) -> Vec<Actor> {
    sync_i18n_cache(state, ctx.i18n_revision);
    let lp = LogoParams::default();
    let mut actors: Vec<Actor> = Vec::with_capacity(96);

    // 1) background component (never fades)
    let backdrop = if state.rainbow_mode {
        [1.0, 1.0, 1.0, 1.0]
    } else {
        [0.0, 0.0, 0.0, 1.0]
    };
    actors.extend(state.bg.build_at_elapsed(
        visual_style_bg::Params {
            active_color_index: state.active_color_index,
            backdrop_rgba: backdrop,
            alpha_mul: 1.0,
        },
        ctx.bg_elapsed_s,
    ));

    // If fully faded, don't create the other actors
    if alpha_multiplier <= 0.0 {
        return actors;
    }

    // 2) logo + info
    let info2_y_tl = lp.top_margin - INFO_MARGIN_ABOVE - INFO_PX;
    let info1_y_tl = info2_y_tl - INFO_PX - INFO_GAP;

    let logo_actors = logo::build_logo_default();
    for mut actor in logo_actors {
        if let Actor::Sprite { tint, .. } = &mut actor {
            tint[3] *= alpha_multiplier;
        }
        actors.push(actor);
    }

    let mut info_color = [1.0, 1.0, 1.0, 1.0];
    info_color[3] *= alpha_multiplier;

    actors.push(act!(text:
        align(0.5, 0.0): xy(ctx.screen_center_x, info1_y_tl): zoom(0.8):
        font("miso"): settext(menu_info_text(state, ctx)): horizalign(center):
        diffuse(info_color[0], info_color[1], info_color[2], info_color[3])
    ));

    // 3) menu list
    let base_y = lp.top_margin + lp.target_h + MENU_BELOW_LOGO;
    let mut selected = color::menu_selected_rgba(state.active_color_index);
    let mut normal = color::rgba_hex(NORMAL_COLOR_HEX);
    selected[3] *= alpha_multiplier;
    normal[3] *= alpha_multiplier;

    let menu_labels = [
        (ctx.tr)("Menu", "Gameplay"),
        (ctx.tr)("Menu", "Options"),
        (ctx.tr)("Menu", "Exit"),
    ];

    let params = menu_list::MenuParams {
        options: &menu_labels,
        selected_index: state.selected_index,
        start_center_y: base_y,
        row_spacing: MENU_ROW_SPACING,
        selected_color: selected,
        normal_color: normal,
        font: ctx.menu_font,
    };
    actors.extend(menu_list::build_vertical_menu(params));

    // --- footer bar ---
    let mut footer_fg = [1.0, 1.0, 1.0, 1.0];
    footer_fg[3] *= alpha_multiplier;
    let event_mode = (ctx.tr)("Common", "EventMode");
    let press_start = (ctx.tr)("Common", "PressStart");

    actors.push(screen_bar::build_title_menu(screen_bar::ScreenBarParams {
        title: event_mode.as_ref(),
        title_placement: screen_bar::ScreenBarTitlePlacement::Center,
        position: screen_bar::ScreenBarPosition::Bottom,
        transparent: true,
        left_text: Some(press_start.as_ref()),
        center_text: None,
        right_text: Some(press_start.as_ref()),
        left_avatar: None,
        right_avatar: None,
        fg_color: footer_fg,
    }));

    // --- GrooveStats Info Pane (top-left) ---
    let gs_text = groovestats_text(state, ctx);
    actors.push(status_text_actor(
        gs_text.main.clone(),
        0.0,
        STATUS_BASE_X,
        STATUS_BASE_Y,
        STATUS_ZOOM,
        alpha_multiplier,
        TextAlign::Left,
    ));
    for line_idx in 0..gs_text.line_count {
        if let Some(text) = gs_text.lines[line_idx].as_ref() {
            actors.push(status_text_actor(
                text.clone(),
                0.0,
                STATUS_BASE_X,
                (STATUS_LINE_HEIGHT * (line_idx as f32 + 1.0)).mul_add(STATUS_ZOOM, STATUS_BASE_Y),
                STATUS_ZOOM,
                alpha_multiplier,
                TextAlign::Left,
            ));
        }
    }

    // --- Arrow Cloud Info Pane (below GrooveStats/BoogieStats) ---
    let ac_base_y = (STATUS_LINE_HEIGHT * (gs_text.line_count as f32 + 1.0))
        .mul_add(STATUS_ZOOM, STATUS_BASE_Y + STATUS_BLOCK_GAP);
    let ac_text = arrowcloud_text(state, ctx);
    actors.push(status_text_actor(
        ac_text.main.clone(),
        0.0,
        STATUS_BASE_X,
        ac_base_y,
        STATUS_ZOOM,
        alpha_multiplier,
        TextAlign::Left,
    ));
    for line_idx in 0..ac_text.line_count {
        if let Some(text) = ac_text.lines[line_idx].as_ref() {
            actors.push(status_text_actor(
                text.clone(),
                0.0,
                STATUS_BASE_X,
                (STATUS_LINE_HEIGHT * (line_idx as f32 + 1.0)).mul_add(STATUS_ZOOM, ac_base_y),
                STATUS_ZOOM,
                alpha_multiplier,
                TextAlign::Left,
            ));
        }
    }

    actors
}
