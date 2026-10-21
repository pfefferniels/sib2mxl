//! The house style: text, line and clef styles, and instrument types with the
//! stave types they use. Objects in recent files name their styles by id, such
//! as `text.staff.expression`; files of the first versions index lists of
//! named styles instead, whose names are translated to the same ids here.

use libsib::tree::Node;

use crate::fields::Fields;
use crate::model::{Font, Instrument, Interval, Layout};

pub struct Styles {
    pub layout: Layout,
    text: Vec<String>,
    text_fonts: Vec<Font>,
    line: Vec<String>,
    lines: Vec<LineStyle>,
    clef: Vec<String>,
    instruments: Vec<InstrumentType>,
    /// The open strings of each tablature stave type, lowest first, in quarter tones.
    tunings: Vec<(String, Vec<i64>)>,
}

/// How a line style draws a line, and its caption.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LineStyle {
    pub text: Option<String>,
    /// `solid`, `dashed`, `dotted` or `wavy`.
    pub line_type: &'static str,
    /// How each end looks: `none`, `up` or `down` (a hook), or `arrow`.
    pub start: &'static str,
    pub end: &'static str,
    pub vertical: bool,
}

pub struct InstrumentType {
    id: String,
    pub full_name: Option<String>,
    pub short_name: Option<String>,
    pub instrument: Instrument,
    pub percussion: bool,
}

/// Stave type categories.
const PERCUSSION: i64 = 1;

impl Styles {
    pub fn read(music: &Node) -> Self {
        let ids = |list: &str, translate: fn(&str) -> String| -> Vec<String> {
            music
                .children(list)
                .into_iter()
                .map(|style| {
                    style
                        .style("Id")
                        .filter(|id| !id.is_empty())
                        .unwrap_or_else(|| translate(&style.string("Name").unwrap_or_default()))
                })
                .collect()
        };
        let layout = layout(music);
        Self {
            layout,
            text: ids("TextStyles", old_text_id),
            text_fonts: music
                .children("TextStyles")
                .into_iter()
                .map(|style| font(style, &layout))
                .collect(),
            line: ids("LineStyles", old_line_id),
            lines: music.children("LineStyles").into_iter().map(line_style).collect(),
            clef: ids("ClefStyles", old_clef_id),
            instruments: music
                .children("InstrumentList")
                .into_iter()
                .filter_map(|instrument| instrument_type(music, instrument))
                .collect(),
            tunings: music
                .children("StaveTypes")
                .into_iter()
                .filter_map(|stave_type| {
                    let pitches = stave_type.numbers("Pitches");
                    Some((stave_type.style("Id")?, pitches)).filter(|(_, pitches)| !pitches.is_empty())
                })
                .collect(),
        }
    }

    /// The open strings of a tablature stave type, lowest first.
    pub fn tuning(&self, stave_type: &str) -> Option<&[i64]> {
        self.tunings
            .iter()
            .find(|(id, _)| id == stave_type)
            .map(|(_, pitches)| pitches.as_slice())
    }

    /// The type of the instrument that an instrument change switches to.
    pub fn instrument_type(&self, change: &Node) -> Option<&InstrumentType> {
        let id = change.child("Instance")?.style("InstrumentId")?;
        self.instruments.iter().find(|instrument| instrument.id == id)
    }

    /// The text style of styled text.
    pub fn text(&self, styled: &Node) -> Option<String> {
        resolve(styled, "TextStyle", &self.text)
    }

    /// The font a text style sets its text in.
    pub fn text_font(&self, id: &str) -> Font {
        self.text
            .iter()
            .position(|text| text == id)
            .and_then(|index| self.text_fonts.get(index))
            .cloned()
            .unwrap_or_default()
    }

    pub fn line(&self, item: &Node) -> Option<String> {
        resolve(item, "LineStyle", &self.line)
    }

    /// The clef style of a clef, as shown at concert pitch or transposed.
    pub fn clef(&self, item: &Node, transposed: bool) -> Option<String> {
        transposed
            .then(|| resolve(item, "TransposingClefStyle", &self.clef))
            .flatten()
            .or_else(|| resolve(item, "ClefStyle", &self.clef))
    }

    /// A line style as the file defines it, or as its id describes the
    /// built-in styles, which files leave out unless they use them.
    pub fn line_style(&self, id: &str) -> LineStyle {
        self.line
            .iter()
            .position(|line| line == id)
            .and_then(|index| self.lines.get(index))
            .cloned()
            .unwrap_or_else(|| built_in_line_style(id))
    }
}

/// The document's page and staff size, as Layout > Document Setup sets them.
fn layout(music: &Node) -> Layout {
    let setup = music
        .child("Options")
        .and_then(|options| options.child("PageAndStaveMargins"));
    let millimetres = |name: &str, default: f64| {
        setup
            .and_then(|setup| setup.float(name))
            .filter(|&mm| mm > 0.0)
            .unwrap_or(default)
    };
    let default = Layout::default();
    Layout {
        staff_height: millimetres("StaveHeightmm", default.staff_height),
        page_width: millimetres("PageWidthmm", default.page_width),
        page_height: millimetres("PageHeightmm", default.page_height),
        left: millimetres("PageLeftMarginmm", default.left),
        right: millimetres("PageRightMarginmm", default.right),
        top: millimetres("PageTopMarginmm", default.top),
        bottom: millimetres("PageBottomMarginmm", default.bottom),
    }
}

/// A text style's font. Its height counts 32nds of a staff space.
fn font(style: &Node, layout: &Layout) -> Font {
    Font {
        family: style.string("FontName").filter(|name| !name.is_empty()),
        size: style
            .number("Height")
            .filter(|&height| height > 0)
            .map(|height| layout.points(height as f64)),
        bold: style.flag("Bold"),
        italic: style.flag("Italic"),
        underline: style.flag("Underline"),
    }
}

fn line_style(style: &Node) -> LineStyle {
    let end = |notch: &str, arrow: &str| match (style.number(arrow), style.signed(notch)) {
        (Some(arrow), _) if arrow != 0 => "arrow",
        (_, Some(dy)) if dy < 0 => "down",
        (_, Some(dy)) if dy > 0 => "up",
        _ => "none",
    };
    LineStyle {
        text: style
            .flag("CaptionEnabled")
            .then(|| style.child("CaptionRef")?.string("Text"))
            .flatten()
            .or_else(|| style.nested_string("Text")),
        line_type: match style.number("LineType") {
            Some(2) => "dotted",
            Some(3) => "dashed",
            _ => "solid",
        },
        start: end("StartCapNotchDy", "ArrowType"),
        end: end("EndCapNotchDy", "EndCapArrowType"),
        vertical: style.flag("Vertical"),
    }
}

fn built_in_line_style(id: &str) -> LineStyle {
    let hook = match () {
        _ if id.contains("bracket.above") => "down",
        _ if id.contains("bracket.below") => "up",
        _ => "none",
    };
    let (start, end) = match () {
        _ if id.contains("arrow") && id.contains("right.left") => ("arrow", "arrow"),
        _ if id.contains("arrow") => ("none", "arrow"),
        _ if id.ends_with(".start") => (hook, "none"),
        _ if id.ends_with(".end") => ("none", hook),
        _ => (hook, hook),
    };
    LineStyle {
        text: None,
        line_type: match () {
            _ if id.contains("dashed") => "dashed",
            _ if id.contains("dotted") => "dotted",
            _ if id.contains("vibrato") || id.contains("wavy") => "wavy",
            _ => "solid",
        },
        start,
        end,
        vertical: id.contains("vertical"),
    }
}

/// An instrument type, with the key signatures and category of its stave type.
fn instrument_type(music: &Node, instrument: &Node) -> Option<InstrumentType> {
    let stave_type = instrument.style("StaveTypeId").and_then(|id| {
        music
            .children("StaveTypes")
            .into_iter()
            .find(|stave_type| stave_type.style("Id").as_deref() == Some(id.as_str()))
    });
    Some(InstrumentType {
        id: instrument.style("Id")?,
        full_name: instrument.string("FullName"),
        short_name: instrument.string("ShortName"),
        instrument: Instrument {
            transposition: Interval {
                diatonic: instrument.signed("PartDiat").unwrap_or(0),
                chrom2: instrument.signed("PartChrom2").unwrap_or(0),
            },
            key_signature: stave_type.is_none_or(|stave_type| stave_type.flag("KeySignatures")),
        },
        percussion: stave_type.is_some_and(|stave_type| stave_type.number("Category") == Some(PERCUSSION)),
    })
}

fn resolve(item: &Node, field: &str, by_index: &[String]) -> Option<String> {
    item.style(field).filter(|id| !id.is_empty()).or_else(|| {
        let index = item.child(field)?.number("value")?;
        by_index.get(usize::try_from(index).ok()?).cloned()
    })
}

const OLD_TEXT_STYLES: [(&str, &str); 16] = [
    ("title", "text.system.page_aligned.title"),
    ("subtitle", "text.system.page_aligned.subtitle"),
    ("dedication", "text.system.page_aligned.dedication"),
    ("composer", "text.system.page_aligned.composer"),
    ("lyricist", "text.system.page_aligned.lyricist"),
    ("copyright", "text.system.page_aligned.copyright"),
    ("header", "text.system.page_aligned.header"),
    ("footer", "text.system.page_aligned.footer"),
    (
        "instrument name at top left",
        "text.system.page_aligned.instrnametopleft",
    ),
    ("tempo", "text.system.tempo"),
    ("metronome mark", "text.system.metronome"),
    ("bar numbers", "text.system.barnumber"),
    ("expression", "text.staff.expression"),
    ("figured bass", "text.staff.space.figuredbass"),
    ("fingering", "text.staff.space.fingering"),
    ("time signatures", "text.staff.timesig"),
];

/// `Lyrics verse 2` becomes `text.staff.space.hypen.lyrics.verse2` (sic).
fn old_text_id(name: &str) -> String {
    let name = name.to_lowercase();
    if let Some(verse) = name.strip_prefix("lyrics") {
        let verse = verse.trim().trim_start_matches("verse").trim();
        return format!(
            "text.staff.space.hypen.lyrics.verse{}",
            if verse.is_empty() { "1" } else { verse }
        );
    }
    OLD_TEXT_STYLES
        .iter()
        .find(|(old, _)| name.starts_with(old))
        .map_or_else(
            || format!("text.staff.{}", name.replace(' ', "_")),
            |(_, id)| id.to_string(),
        )
}

const OLD_LINE_STYLES: [(&str, &str); 9] = [
    ("Octave up", "line.staff.octava.plus8"),
    ("Octave down", "line.staff.octava.minus8"),
    ("2 octaves up", "line.staff.octava.plus15"),
    ("2 octaves down", "line.staff.octava.minus15"),
    ("Pedal", "line.staff.pedal"),
    ("1st ending", "line.system.repeat.1st"),
    ("2nd ending", "line.system.repeat.2nd"),
    ("Repeat ending (open)", "line.system.repeat.open"),
    ("Repeat ending (closed)", "line.system.repeat.closed"),
];

fn old_line_id(name: &str) -> String {
    OLD_LINE_STYLES.iter().find(|(old, _)| *old == name).map_or_else(
        || format!("line.{}", name.to_lowercase().replace(' ', "_")),
        |(_, id)| id.to_string(),
    )
}

/// `Treble (down 8)` becomes `clef.treble.down.8`.
fn old_clef_id(name: &str) -> String {
    let words: Vec<String> = name
        .to_lowercase()
        .replace(['(', ')'], "")
        .split_whitespace()
        .map(str::to_string)
        .collect();
    format!("clef.{}", words.join("."))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_names_become_ids() {
        assert_eq!(old_text_id("Lyrics verse 3"), "text.staff.space.hypen.lyrics.verse3");
        assert_eq!(old_text_id("Lyrics"), "text.staff.space.hypen.lyrics.verse1");
        assert_eq!(old_text_id("Expression"), "text.staff.expression");
        assert_eq!(old_clef_id("Treble (down 8)"), "clef.treble.down.8");
        assert_eq!(old_line_id("2 octaves down"), "line.staff.octava.minus15");
    }
}
