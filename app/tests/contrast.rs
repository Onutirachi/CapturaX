//! Contrast (FR-018): every text pair reaches 4.5:1 and every control icon 3:1, in both themes.
//!
//! Reads the color tokens of `ui/theme.slint` (the `Palettes` global) and checks the declared
//! pairs below with the WCAG relative luminance formula. A background is a stack of layers from
//! the bottom up; semi-transparent layers are blended over what is under them. Panels over the
//! video are checked over a white and a black video.

use std::collections::HashMap;
use std::path::Path;

/// A color as straight RGBA in 0..=1.
#[derive(Clone, Copy, Debug)]
struct Rgba {
    r: f64,
    g: f64,
    b: f64,
    a: f64,
}

impl Rgba {
    fn parse(hex: &str) -> Option<Self> {
        let hex = hex.strip_prefix('#')?;
        let byte = |i: usize| {
            u8::from_str_radix(hex.get(i..i + 2)?, 16)
                .ok()
                .map(|v| v as f64 / 255.0)
        };
        match hex.len() {
            6 => Some(Self {
                r: byte(0)?,
                g: byte(2)?,
                b: byte(4)?,
                a: 1.0,
            }),
            8 => Some(Self {
                r: byte(0)?,
                g: byte(2)?,
                b: byte(4)?,
                a: byte(6)?,
            }),
            _ => None,
        }
    }

    /// This color laid over an opaque `under`.
    fn over(self, under: Rgba) -> Rgba {
        let mix = |top: f64, bottom: f64| top * self.a + bottom * (1.0 - self.a);
        Rgba {
            r: mix(self.r, under.r),
            g: mix(self.g, under.g),
            b: mix(self.b, under.b),
            a: 1.0,
        }
    }

    fn luminance(self) -> f64 {
        let channel = |c: f64| {
            if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * channel(self.r) + 0.7152 * channel(self.g) + 0.0722 * channel(self.b)
    }
}

fn ratio(a: Rgba, b: Rgba) -> f64 {
    let (la, lb) = (a.luminance(), b.luminance());
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

/// What sits behind the layers of a pair.
#[derive(Clone, Copy)]
enum Behind {
    /// The layers start with an opaque surface.
    Surface,
    /// The layers are a panel over the video: checked over white and over black.
    Video,
}

/// One text or icon color on one background, and where the app draws it. A name is a palette
/// token, or a `#hex` color the UI writes directly.
struct Pair {
    color: &'static str,
    layers: &'static [&'static str],
    behind: Behind,
    /// 4.5:1 for text, 3:1 for the icons of controls (WCAG non-text contrast).
    min: f64,
    used: &'static str,
}

const TEXT: f64 = 4.5;
const ICON: f64 = 3.0;

const fn text(color: &'static str, layers: &'static [&'static str], used: &'static str) -> Pair {
    Pair {
        color,
        layers,
        behind: Behind::Surface,
        min: TEXT,
        used,
    }
}

const fn icon(color: &'static str, layers: &'static [&'static str], used: &'static str) -> Pair {
    Pair {
        color,
        layers,
        behind: Behind::Surface,
        min: ICON,
        used,
    }
}

const fn text_over_video(color: &'static str, layers: &'static [&'static str], used: &'static str) -> Pair {
    Pair {
        color,
        layers,
        behind: Behind::Video,
        min: TEXT,
        used,
    }
}

const fn icon_over_video(color: &'static str, layers: &'static [&'static str], used: &'static str) -> Pair {
    Pair {
        color,
        layers,
        behind: Behind::Video,
        min: ICON,
        used,
    }
}

/// Every text and control icon pair the interface draws. A new color or surface for text or an
/// icon adds a row here. Not listed: the toggle track and knob, the menu's scroll indicator and
/// the focus ring (states, not text or icons).
const PAIRS: &[Pair] = &[
    // Raised surfaces: settings panel and its rows, notice, menu.
    text("text", &["raised"], "setting row names, notice text, menu items"),
    // A row under the pointer: the soft accent gradient, strongest at its start.
    text("text", &["raised", "accent-soft"], "setting row hovered"),
    text("text", &["raised", "hover"], "menu item highlighted"),
    text("text", &["raised", "press"], "menu item chosen"),
    text("text-muted", &["raised"], "version in About, menu item detail"),
    text("text-muted", &["raised", "hover"], "menu item detail highlighted"),
    text("text-muted", &["raised", "press"], "menu item detail chosen"),
    // Top surfaces: tooltip, select.
    text("text", &["top"], "tooltip, select"),
    text("text", &["top", "hover"], "select hovered"),
    text("text", &["top", "press"], "select pressed"),
    text("text-muted", &["top"], "dropdown title"),
    text("text-muted", &["top", "hover"], "dropdown title hovered"),
    text("text-faint", &["raised"], "menu group headers"),
    // Base: the category column.
    text("text", &["base"], "settings panel title"),
    text("text", &["base", "hover"], "category hovered"),
    text("text-muted", &["base"], "category names"),
    text("accent-text", &["base", "accent-soft"], "category chosen"),
    // Accent as text on the light surfaces.
    text("accent-text", &["base"], "accent text on base"),
    text("accent-text", &["raised"], "accent text on raised"),
    text("accent-text", &["top"], "accent text on top"),
    // Ink on the accent.
    text("accent-ink", &["accent"], "accent buttons"),
    text("accent-ink", &["accent-hover"], "accent buttons hovered"),
    // Dock buttons on the panel over the video; a toggled tool sits on an opaque chip.
    icon_over_video("text", &["panel"], "dock button"),
    icon_over_video("text", &["panel", "hover"], "dock button hovered"),
    icon_over_video("text", &["panel", "press"], "dock button pressed"),
    icon("accent-text", &["raised", "accent-soft"], "dock tool toggled"),
    icon(
        "accent-text",
        &["raised", "accent-soft", "hover"],
        "dock tool toggled, hovered",
    ),
    icon(
        "accent-text",
        &["raised", "accent-soft", "press"],
        "dock tool toggled, pressed",
    ),
    icon("accent-ink", &["accent"], "primary tool"),
    icon("accent-ink", &["accent-hover"], "primary tool hovered"),
    // Title bar.
    icon("text-muted", &["deep"], "window buttons"),
    icon("text-muted", &["deep", "hover"], "window button hovered"),
    icon("text-muted", &["deep", "press"], "window button pressed"),
    icon("text-faint", &["deep"], "window buttons, window inactive"),
    icon("#ffffff", &["#c42b1c"], "close button hovered"),
    icon("#ffffff", &["#a32417"], "close button pressed"),
    // Settings panel.
    icon("text", &["base"], "settings icon by the title"),
    icon("text", &["raised"], "panel close button"),
    icon("text", &["raised", "hover"], "panel close button hovered"),
    icon("text", &["raised", "press"], "panel close button pressed"),
    icon("text-muted", &["base"], "category icons"),
    icon("text", &["base", "hover"], "category icon hovered"),
    icon("accent-text", &["base", "accent-soft"], "category icon chosen"),
    icon("text-muted", &["top"], "select chevron"),
    icon("text-muted", &["top", "hover"], "select chevron hovered"),
    icon("text-muted", &["top", "press"], "select chevron pressed"),
    // The notification position's corner icon (FR-032): the frame in the muted tone (as the
    // chevron above on the select), the mark in the accent text tone, on the select and in its
    // grid menu.
    icon("accent-text", &["top"], "corner mark on the select"),
    icon("accent-text", &["top", "hover"], "corner mark on the select hovered"),
    icon("text-muted", &["raised"], "corner frame in the grid"),
    icon("text-muted", &["raised", "hover"], "corner frame in the grid hovered"),
    icon("text-muted", &["raised", "press"], "corner frame in the grid chosen"),
    icon("accent-text", &["raised"], "corner mark in the grid"),
    icon("accent-text", &["raised", "hover"], "corner mark in the grid hovered"),
    icon("accent-text", &["raised", "press"], "corner mark in the grid chosen"),
    // State message, on the video area's ground; its secondary action is a top surface (listed
    // above) and its primary one the accent.
    text("text", &["deep"], "state message title"),
    text("text-muted", &["deep"], "state message detail"),
    text("on-black-text", &["ground-black"], "state message title on black"),
    text("on-black-muted", &["ground-black"], "state message detail on black"),
    icon(
        "on-black-muted",
        &["ground-black"],
        "state message icon and progress ring on black",
    ),
    icon("text-muted", &["deep"], "state message icon"),
    // Segmented control: its track is the base tone, the chosen segment the accent.
    text("text", &["base"], "segment"),
    text("text", &["base", "hover"], "segment hovered"),
    text("accent-ink", &["accent"], "segment chosen"),
    text("accent-ink", &["accent-hover"], "segment chosen hovered"),
    // Mode tags: white text on each fill (the rate tag is the text color on the menu).
    text("#ffffff", &["tag-1-1"], "1:1 tag"),
    text("#ffffff", &["tag-5-4"], "5:4 tag"),
    text("#ffffff", &["tag-11-9"], "11:9 tag"),
    text("#ffffff", &["tag-4-3"], "4:3 tag"),
    text("#ffffff", &["tag-3-2"], "3:2 tag"),
    text("#ffffff", &["tag-25-16"], "25:16 tag"),
    text("#ffffff", &["tag-16-10"], "16:10 tag"),
    text("#ffffff", &["tag-5-3"], "5:3 tag"),
    text("#ffffff", &["tag-16-9"], "16:9 tag"),
    text("#ffffff", &["tag-17-9"], "17:9 tag"),
    text("#ffffff", &["tag-21-9"], "21:9 tag"),
    text("#ffffff", &["tag-32-9"], "32:9 tag"),
    text("#ffffff", &["tag-custom"], "custom tag"),
    text("#ffffff", &["tag-aspect"], "aspect tag"),
    // Info bubbles over the video; their mode tag is a filled tag (listed above).
    text_over_video("info-text", &["info-bubble"], "info bubble text"),
    // Notice.
    icon("accent-text", &["raised"], "notice info icon"),
    icon("warn", &["raised"], "notice warning icon"),
    icon("text-muted", &["raised"], "notice close"),
    icon("text-muted", &["raised", "hover"], "notice close hovered"),
    icon("text-muted", &["raised", "press"], "notice close pressed"),
    // The device notice balloon (FR-027): its text is "notice text" above, Open is the accent
    // button, × the notice close.
    icon("ok", &["raised"], "device notice connected icon"),
    icon("text-muted", &["raised"], "device notice disconnected icon"),
];

/// Reads `name: #hex,` lines of each palette in the `Palettes` global.
fn palettes(source: &str) -> HashMap<String, HashMap<String, Rgba>> {
    let mut all = HashMap::new();
    let mut current: Option<(String, HashMap<String, Rgba>)> = None;
    for line in source.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("out property <Palette> ") {
            let name = rest.split(':').next().unwrap_or_default().trim().to_string();
            current = Some((name, HashMap::new()));
        } else if line == "};" {
            if let Some((name, tokens)) = current.take() {
                all.insert(name, tokens);
            }
        } else if let Some((_, tokens)) = current.as_mut()
            && let Some((key, value)) = line.trim_end_matches(',').split_once(':')
            && let Some(color) = Rgba::parse(value.trim())
        {
            tokens.insert(key.trim().to_string(), color);
        }
    }
    all
}

#[test]
fn pairs_reach_minimum_contrast() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("ui/theme.slint");
    let source = std::fs::read_to_string(&path).expect("read ui/theme.slint");
    let palettes = palettes(&source);
    let mut failures = Vec::new();

    for theme in ["dark", "light"] {
        let tokens = palettes
            .get(theme)
            .unwrap_or_else(|| panic!("palette `{theme}` not found in theme.slint"));
        let token = |name: &str| {
            if name.starts_with('#') {
                return Rgba::parse(name).unwrap_or_else(|| panic!("bad color `{name}`"));
            }
            *tokens
                .get(name)
                .unwrap_or_else(|| panic!("token `{name}` not found in the {theme} palette"))
        };

        for pair in PAIRS {
            let videos: &[(&str, Rgba)] = match pair.behind {
                Behind::Surface => &[(
                    "",
                    Rgba {
                        r: 0.0,
                        g: 0.0,
                        b: 0.0,
                        a: 1.0,
                    },
                )],
                Behind::Video => &[
                    (
                        " over a white video",
                        Rgba {
                            r: 1.0,
                            g: 1.0,
                            b: 1.0,
                            a: 1.0,
                        },
                    ),
                    (
                        " over a black video",
                        Rgba {
                            r: 0.0,
                            g: 0.0,
                            b: 0.0,
                            a: 1.0,
                        },
                    ),
                ],
            };
            for &(label, video) in videos {
                let first = token(pair.layers[0]);
                if matches!(pair.behind, Behind::Surface) {
                    assert!(
                        first.a == 1.0,
                        "{theme}: `{}` is not opaque; list the pair as a panel over the video",
                        pair.layers[0]
                    );
                }
                let background = pair.layers.iter().fold(video, |under, layer| token(layer).over(under));
                let color = token(pair.color).over(background);
                let r = ratio(color, background);
                if r < pair.min {
                    let kind = if pair.min == TEXT { "text" } else { "icon" };
                    failures.push(format!(
                        "{theme}: {kind} {} on {}{label} is {r:.2}:1, below {}:1 ({})",
                        pair.color,
                        pair.layers.join(" + "),
                        pair.min,
                        pair.used
                    ));
                }
            }
        }
    }

    assert!(
        failures.is_empty(),
        "pairs below their minimum contrast:\n{}",
        failures.join("\n")
    );
}

#[test]
fn ratio_matches_wcag_examples() {
    let white = Rgba::parse("#ffffff").unwrap();
    let black = Rgba::parse("#000000").unwrap();
    assert!((ratio(white, black) - 21.0).abs() < 1e-9);
    // #767676 on white is the usual 4.54:1 example.
    let grey = Rgba::parse("#767676").unwrap();
    assert!((ratio(grey, white) - 4.54).abs() < 0.01);
    // Half black over white blends to about #808080.
    let half = Rgba::parse("#00000080").unwrap().over(white);
    assert!((half.r - 127.0 / 255.0).abs() < 0.01);
}
