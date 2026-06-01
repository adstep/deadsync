use crate::act;
use crate::engine::present::actors::{Actor, TextContent};
use crate::engine::space::screen_height;

/// Zoom applied to the step-artist text. Exposed so callers can compute
/// rendered widths and clip windows in the same post-zoom space the renderer
/// lays out (see `shared::scrolling_text`).
pub const ARTIST_ZOOM: f32 = 0.8;

/// `maxwidth` of the Expanded artist text (pre-zoom). The rendered clip window is
/// `ARTIST_EXPANDED_MAXWIDTH * ARTIST_ZOOM`; callers pass that as the scroll
/// `box_w` so overflow detection matches the on-screen window.
pub const ARTIST_EXPANDED_MAXWIDTH: f32 = 175.0;

/// Logical line spacing of the `miso` font (see `assets/fonts/miso/_miso light.ini`,
/// `LineSpacing=24`). Used to stack the Expanded box's per-line actors so they
/// match the engine's multi-line layout. Coupled to the `font("miso")` +
/// `ARTIST_ZOOM` used when rendering those lines below.
const ARTIST_LINE_SPACING_LOGICAL: f32 = 24.0;

/// One displayed line of the Expanded step-artist box, with its independent
/// reset-loop scroll state.
#[derive(Clone)]
pub struct StepArtistLine {
    pub text: TextContent,
    /// Horizontal scroll offset (rendered px) applied via `addx`. `0.0` =
    /// unscrolled. Only used when `clip_width` is `Some`.
    pub scroll_offset: f32,
    /// When `Some(w)`, the line is rendered full-width (no `maxwidth`) and clipped
    /// to a window of rendered width `w`, with `scroll_offset` applied. When
    /// `None`, the line renders with the classic `maxwidth` squish.
    pub clip_width: Option<f32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepArtistBarLayout {
    Legacy,
    Expanded,
}

pub struct StepArtistBarParams {
    pub x0: f32,
    pub center_y: f32,
    pub layout: StepArtistBarLayout,
    pub accent_color: [f32; 4],
    pub z_base: i16,
    pub label_text: TextContent,
    pub label_max_width: f32,
    pub artist_text: TextContent,
    pub artist_x_offset: f32,
    pub artist_max_width: f32,
    pub artist_color: [f32; 4],
    /// Horizontal scroll offset (rendered px) for the Legacy artist text. Only
    /// applied when `artist_clip_width` is `Some`. `0.0` = unscrolled.
    pub artist_scroll_offset: f32,
    /// When `Some(w)`, the Legacy artist text is rendered full-width (no
    /// `maxwidth` squishing) and clipped to a window of rendered width `w`
    /// anchored at the artist origin, with `artist_scroll_offset` applied. When
    /// `None`, the artist text renders exactly as before (maxwidth + zoom).
    pub artist_clip_width: Option<f32>,
    /// Per-line text + scroll state for the Expanded layout (one entry per visible
    /// value). Ignored by the Legacy layout.
    pub artist_lines: Vec<StepArtistLine>,
}

pub fn push(out: &mut Vec<Actor>, p: StepArtistBarParams) {
    let z_text = p.z_base.saturating_add(1);
    match p.layout {
        StepArtistBarLayout::Legacy => {
            let comp_h = screen_height() / 28.0;
            out.push(act!(quad:
                align(0.5, 0.5):
                xy(p.x0 + 113.0, p.center_y):
                setsize(175.0, comp_h):
                z(p.z_base):
                diffuse(p.accent_color[0], p.accent_color[1], p.accent_color[2], 1.0)
            ));
            out.push(act!(text:
                font("miso"):
                settext(p.label_text):
                align(0.0, 0.5):
                xy(p.x0 + 30.0, p.center_y):
                maxwidth(p.label_max_width):
                zoom(0.8):
                z(z_text):
                diffuse(0.0, 0.0, 0.0, 1.0)
            ));
            if let Some(clip_w) = p.artist_clip_width {
                let clip_x = p.x0 + p.artist_x_offset;
                let clip_y = p.center_y - comp_h;
                let clip_h = comp_h * 2.0;
                out.push(act!(text:
                    font("miso"):
                    settext(p.artist_text):
                    align(0.0, 0.5):
                    xy(p.x0 + p.artist_x_offset, p.center_y):
                    addx(p.artist_scroll_offset):
                    clip(clip_x, clip_y, clip_w, clip_h):
                    zoom(ARTIST_ZOOM):
                    z(z_text):
                    diffuse(
                        p.artist_color[0],
                        p.artist_color[1],
                        p.artist_color[2],
                        p.artist_color[3]
                    )
                ));
            } else {
                out.push(act!(text:
                    font("miso"):
                    settext(p.artist_text):
                    align(0.0, 0.5):
                    xy(p.x0 + p.artist_x_offset, p.center_y):
                    maxwidth(p.artist_max_width):
                    zoom(ARTIST_ZOOM):
                    z(z_text):
                    diffuse(
                        p.artist_color[0],
                        p.artist_color[1],
                        p.artist_color[2],
                        p.artist_color[3]
                    )
                ));
            }
        }
        StepArtistBarLayout::Expanded => {
            let comp_h = screen_height() / 8.0;
            let line_count = p.artist_lines.len();
            let fade_bottom = match line_count {
                1 => 0.8,
                2 => 0.5,
                _ => 0.0,
            };
            out.push(act!(quad:
                align(0.5, 0.5):
                xy(p.x0 + 120.0, p.center_y + 18.0):
                setsize(190.0, comp_h):
                fadebottom(fade_bottom):
                z(p.z_base):
                diffuse(p.accent_color[0], p.accent_color[1], p.accent_color[2], 1.0)
            ));
            out.push(act!(text:
                font("miso"):
                settext(p.label_text):
                align(0.0, 0.5):
                xy(p.x0 + 30.0, p.center_y):
                maxwidth(p.label_max_width):
                zoom(0.8):
                z(z_text):
                diffuse(0.0, 0.0, 0.0, 1.0)
            ));
            // Each value is its own single-line actor so overflowing lines can be
            // clipped and scrolled independently. They are stacked by the miso
            // line pitch so the column matches the engine's multi-line layout.
            let line_advance = ARTIST_LINE_SPACING_LOGICAL * ARTIST_ZOOM;
            let top_y = p.center_y - 6.0;
            let line_x = p.x0 + 70.0;
            for (i, line) in p.artist_lines.iter().enumerate() {
                let line_y = top_y + i as f32 * line_advance;
                if let Some(clip_w) = line.clip_width {
                    let clip_pad = 2.0;
                    out.push(act!(text:
                        font("miso"):
                        settext(line.text.clone()):
                        align(0.0, 0.0):
                        xy(line_x, line_y):
                        addx(line.scroll_offset):
                        clip(line_x, line_y - clip_pad, clip_w, line_advance + 2.0 * clip_pad):
                        zoom(0.8):
                        z(z_text):
                        diffuse(0.0, 0.0, 0.0, 1.0)
                    ));
                } else {
                    out.push(act!(text:
                        font("miso"):
                        settext(line.text.clone()):
                        align(0.0, 0.0):
                        xy(line_x, line_y):
                        maxwidth(ARTIST_EXPANDED_MAXWIDTH):
                        zoom(0.8):
                        z(z_text):
                        diffuse(0.0, 0.0, 0.0, 1.0)
                    ));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::present::actors::TextContent;

    fn params(layout: StepArtistBarLayout) -> StepArtistBarParams {
        StepArtistBarParams {
            x0: 0.0,
            center_y: 0.0,
            layout,
            accent_color: [1.0, 0.0, 0.0, 1.0],
            z_base: 10,
            label_text: TextContent::Static("STEPS"),
            label_max_width: 40.0,
            artist_text: TextContent::Static("a very long chart description"),
            artist_x_offset: 75.0,
            artist_max_width: 124.0,
            artist_color: [0.0, 0.0, 0.0, 1.0],
            artist_scroll_offset: 0.0,
            artist_clip_width: None,
            artist_lines: Vec::new(),
        }
    }

    fn text_width_flags(actors: &[Actor]) -> Vec<(Option<f32>, bool)> {
        actors
            .iter()
            .filter_map(|actor| {
                if let Actor::Text {
                    max_width,
                    max_w_pre_zoom,
                    ..
                } = actor
                {
                    Some((*max_width, *max_w_pre_zoom))
                } else {
                    None
                }
            })
            .collect()
    }

    #[test]
    fn legacy_step_artist_maxwidth_is_unzoomed_like_itgmania() {
        let mut actors = Vec::new();
        push(&mut actors, params(StepArtistBarLayout::Legacy));

        assert_eq!(
            text_width_flags(&actors),
            vec![(Some(40.0), true), (Some(124.0), true)]
        );
    }

    #[test]
    fn expanded_step_artist_maxwidth_is_unzoomed_like_arrow_cloud() {
        let mut actors = Vec::new();
        let mut p = params(StepArtistBarLayout::Expanded);
        // Two fitting (unclipped) lines render with the classic maxwidth squish.
        p.artist_lines = vec![
            StepArtistLine {
                text: TextContent::Static("step artist"),
                scroll_offset: 0.0,
                clip_width: None,
            },
            StepArtistLine {
                text: TextContent::Static("description"),
                scroll_offset: 0.0,
                clip_width: None,
            },
        ];
        push(&mut actors, p);

        assert_eq!(
            text_width_flags(&actors),
            vec![(Some(40.0), true), (Some(175.0), true), (Some(175.0), true)]
        );
    }

    #[test]
    fn expanded_step_artist_clipped_line_drops_maxwidth_and_carries_offset() {
        let mut actors = Vec::new();
        let mut p = params(StepArtistBarLayout::Expanded);
        p.artist_lines = vec![StepArtistLine {
            text: TextContent::Static("a very long chart description"),
            scroll_offset: -20.0,
            clip_width: Some(140.0),
        }];
        push(&mut actors, p);

        // The clipped line is the last text actor.
        let line = actors
            .iter()
            .filter_map(|a| match a {
                Actor::Text {
                    max_width,
                    clip,
                    offset,
                    ..
                } => Some((*max_width, *clip, *offset)),
                _ => None,
            })
            .last()
            .expect("artist line actor");

        let (max_width, clip, offset) = line;
        assert_eq!(max_width, None, "clip path must not set maxwidth");
        // x0=0, line_x=70, addx(-20) => offset.x = 50
        assert_eq!(offset[0], 50.0, "scroll offset must be applied via addx");
        let clip = clip.expect("clip path must set a clip rect");
        assert_eq!(clip[2], 140.0, "clip width must match");
    }

    #[test]
    fn legacy_step_artist_with_clip_drops_maxwidth_and_carries_offset() {
        let mut actors = Vec::new();
        let mut p = params(StepArtistBarLayout::Legacy);
        p.artist_clip_width = Some(99.2);
        p.artist_scroll_offset = -12.0;
        push(&mut actors, p);

        // The artist text is the second (last) text actor.
        let artist = actors
            .iter()
            .filter_map(|a| match a {
                Actor::Text {
                    max_width,
                    clip,
                    offset,
                    ..
                } => Some((*max_width, *clip, *offset)),
                _ => None,
            })
            .last()
            .expect("artist text actor");

        let (max_width, clip, offset) = artist;
        assert_eq!(max_width, None, "clip path must not set maxwidth");
        // x0=0, artist_x_offset=75, addx(-12) => offset.x = 63
        assert_eq!(offset[0], 63.0, "scroll offset must be applied via addx");
        let clip = clip.expect("clip path must set a clip rect");
        assert_eq!(clip[2], 99.2, "clip width must match");
    }
}
