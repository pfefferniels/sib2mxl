//! The house style: text, line and clef styles, and instrument types with the
//! stave types they use. Objects in recent files name their styles by id, such
//! as `text.staff.expression`; files of the first versions index lists of
//! named styles instead, whose names are translated to the same ids here.

use libsib::tree::Node;

use crate::fields::Fields;
use crate::model::{Instrument, Interval};

pub struct Styles {
    text: Vec<String>,
    line: Vec<String>,
    line_text: Vec<Option<String>>,
    clef: Vec<String>,
    instruments: Vec<InstrumentType>,
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
        Self {
            text: ids("TextStyles", old_text_id),
            line: ids("LineStyles", old_line_id),
            line_text: music
                .children("LineStyles")
                .into_iter()
                .map(|style| style.nested_string("Text"))
                .collect(),
            clef: ids("ClefStyles", old_clef_id),
            instruments: music
                .children("InstrumentList")
                .into_iter()
                .filter_map(|instrument| instrument_type(music, instrument))
                .collect(),
        }
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

    pub fn line(&self, item: &Node) -> Option<String> {
        resolve(item, "LineStyle", &self.line)
    }

    pub fn clef(&self, item: &Node) -> Option<String> {
        resolve(item, "ClefStyle", &self.clef)
    }

    /// The text a line style draws, such as `rit.`.
    pub fn line_text(&self, id: &str) -> Option<&str> {
        let index = self.line.iter().position(|line| line == id)?;
        self.line_text.get(index)?.as_deref()
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
