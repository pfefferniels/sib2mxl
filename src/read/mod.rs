//! Builds the [`Score`] model from a decoded score payload.
//!
//! Bars are numbered as in Sibelius: bar 0 of every stave holds the initial
//! clef, key and instrument, and bar `n` is measure `n`. Objects at position
//! 0xffff sit at the start of their bar, objects at 0x7f00 at its end.

mod accidentals;
mod chord;
mod figures;
mod link;
mod parts;
mod staff;
mod stems;
mod styles;
mod system;

use libsib::tree::Node;

use crate::error::{Error, Result};
use crate::fields::Fields;
use crate::model::{Credit, Key, Measure, Score, Ticks, Work};
use crate::text;
use styles::Styles;

const BAR_START: Ticks = 0xffff;
const BAR_END: Ticks = 0x7f00;

/// Tags of the bar objects the converter reads.
mod tag {
    pub const CLEF: i64 = 0x07;
    pub const BARLINE: i64 = 0x08;
    pub const TIME_SIGNATURE: i64 = 0x09;
    pub const KEY_SIGNATURE: i64 = 0x0a;
    pub const LINE: i64 = 0x0c;
    pub const ARPEGGIO: i64 = 0x0d;
    pub const CRESCENDO: i64 = 0x0e;
    pub const DIMINUENDO: i64 = 0x0f;
    pub const GLISSANDO: i64 = 0x10;
    pub const OCTAVA: i64 = 0x11;
    pub const PEDAL: i64 = 0x12;
    pub const ENDING: i64 = 0x13;
    pub const SLUR: i64 = 0x14;
    pub const TRILL: i64 = 0x15;
    pub const TUPLET: i64 = 0x18;
    pub const LYRIC: i64 = 0x19;
    pub const RITARDANDO: i64 = 0x1b;
    pub const SYMBOL: i64 = 0x1f;
    pub const SYSTEM_TEXT: i64 = 0x21;
    pub const TEXT: i64 = 0x22;
    pub const GUITAR_FRAME: i64 = 0x24;
    pub const REHEARSAL_MARK: i64 = 0x26;
    pub const INSTRUMENT_CHANGE: i64 = 0x27;
    pub const BAR_REST: i64 = 0x28;
    pub const NOTE_REST: i64 = 0x29;
}

pub fn score(root: &Node) -> Result<Score> {
    let music = root
        .child("Score")
        .and_then(|score| score.child("Music"))
        .ok_or(Error::Missing("music"))?;
    let system = music.child("SystemStave").ok_or(Error::Missing("system stave"))?;
    let summary = Summary(music.child("Summary"));
    let styles = Styles::read(music);
    let measures = system::measures(music, system, &summary, &styles);
    let timeline = Timeline::new(&measures);
    let staves = music.children("Staves");
    if staves.is_empty() {
        return Err(Error::Missing("staves"));
    }
    let staffs = staves
        .iter()
        .map(|stave| staff::read(stave, &measures, &timeline, &styles))
        .collect();
    let (parts, groups) = parts::assemble(music, &staves, staffs, &styles);
    Ok(Score {
        work: work(system, &summary, &styles),
        layout: styles.layout,
        credits: credits(system, &summary, &styles),
        measures,
        parts,
        groups,
    })
}

/// The file's summary (title, composer, …), which text refers to through
/// wildcards such as `\$Title\`.
struct Summary<'a>(Option<&'a Node>);

impl Summary<'_> {
    fn get(&self, name: &str) -> Option<String> {
        self.0?.string(name).filter(|text| !text.is_empty())
    }

    fn plain(&self, raw: &str) -> String {
        text::plain(&text::runs(raw, |name| self.get(name)))
    }
}

/// Title and credits as printed on the first page, else as in the summary.
fn work(system: &Node, summary: &Summary, styles: &Styles) -> Work {
    let page_text = |role: &str| {
        let style = format!("text.system.page_aligned.{role}");
        let text = system
            .children("Bars")
            .into_iter()
            .flat_map(|bar| bar.children("Items"))
            .filter(|item| item.number("tag") == Some(tag::SYSTEM_TEXT))
            .find(|item| text_style(item, styles).is_some_and(|s| s == style || s.starts_with(&format!("{style}."))))
            .and_then(|item| item.child("StyledString")?.string("Text"))?;
        let text = summary.plain(&text).split_whitespace().collect::<Vec<_>>().join(" ");
        (!text.is_empty()).then_some(text)
    };
    Work {
        title: page_text("title").or_else(|| summary.get("Title")),
        composer: page_text("composer").or_else(|| summary.get("Composer")),
        lyricist: page_text("lyricist").or_else(|| summary.get("Lyricist")),
        arranger: summary.get("Arranger"),
        rights: page_text("copyright").or_else(|| summary.get("Copyright")),
    }
}

/// Page-aligned text of the first page, in the order Sibelius lists it.
fn credits(system: &Node, summary: &Summary, styles: &Styles) -> Vec<Credit> {
    const KINDS: [(&str, &str); 7] = [
        ("title", "title"),
        ("subtitle", "subtitle"),
        ("composer", "composer"),
        ("lyricist", "lyricist"),
        ("arranger", "arranger"),
        ("copyright", "rights"),
        ("dedication", "dedication"),
    ];
    let first_bars = system.children("Bars").into_iter().take(2);
    first_bars
        .flat_map(|bar| bar.children("Items"))
        .filter(|item| item.number("tag") == Some(tag::SYSTEM_TEXT) && !item.flag("HideInScore"))
        .filter_map(|item| {
            let style = text_style(item, styles)?;
            let role = style.strip_prefix("text.system.page_aligned.")?;
            let (_, kind) = KINDS.iter().find(|(name, _)| role.split('.').next() == Some(*name))?;
            let text = summary.plain(&item.child("StyledString")?.string("Text")?);
            let text = text.trim();
            (!text.is_empty()).then(|| Credit {
                kind,
                text: text.to_string(),
                font: styles.text_font(&style),
            })
        })
        .collect()
}

fn key(item: &Node) -> Key {
    Key {
        fifths: item.number("Sharps").unwrap_or(0) as i8,
        major: item.number("Major").is_none_or(|major| major != 0),
    }
}

/// The measure a key signature changes: one in bar 0 holds for measure 1;
/// elsewhere one at the start of a bar changes it, and one at its end (or
/// within it) the next.
fn key_measure(bar: usize, item: &Node) -> usize {
    let sr = item.number("Sr").unwrap_or(0) as Ticks;
    match bar {
        0 => 0,
        _ if sr == 0 || sr == BAR_START => bar - 1,
        _ => bar,
    }
}

/// An object's position in a bar of length `length`.
fn position(item: &Node, length: Ticks) -> Ticks {
    match item.number("Sr").unwrap_or(0) as Ticks {
        BAR_START => 0,
        BAR_END => length,
        sr => sr.min(length),
    }
}

fn text_style(item: &Node, styles: &Styles) -> Option<String> {
    styles.text(item.child("StyledString")?)
}

/// The voice an object belongs to: the lowest of the voices it applies to.
fn voice(item: &Node) -> u8 {
    let voices = item.number("Voices").unwrap_or(1) as u8;
    (voices.max(1).trailing_zeros().min(3) + 1) as u8
}

/// Where each measure starts, to follow objects that run across barlines.
pub struct Timeline {
    starts: Vec<Ticks>,
    lengths: Vec<Ticks>,
}

impl Timeline {
    fn new(measures: &[Measure]) -> Self {
        let lengths: Vec<Ticks> = measures.iter().map(|m| m.length).collect();
        let starts = lengths
            .iter()
            .scan(0, |at, length| {
                let start = *at;
                *at += length;
                Some(start)
            })
            .collect();
        Self { starts, lengths }
    }

    fn absolute(&self, measure: usize, at: Ticks) -> Ticks {
        self.starts.get(measure).map_or(0, |start| start + at)
    }

    /// The measure and offset of a position where something ends: a position
    /// on a barline belongs to the measure before it.
    fn locate_end(&self, absolute: Ticks) -> Option<(usize, Ticks)> {
        let measure = self.starts.partition_point(|&start| start < absolute).saturating_sub(1);
        let start = *self.starts.get(measure)?;
        Some((measure, (absolute - start).min(self.lengths[measure])))
    }
}
