//! One stave: its events, clefs and text, and the objects that link events.

use libsib::tree::Node;

use super::link::{self, Span, SpanKind, Syllable};
use super::styles::InstrumentType;
use super::{BAR_END, Styles, Timeline, position, tag, text_style, voice};
use crate::fields::Fields;
use crate::model::{
    Articulations, BeamMode, Clef, Content, Direction, DirectionKind, Event, Figure, Figures, Grace, Instrument,
    Interval, Measure, Note, NoteValue, Pitch, Staff, StaffMeasure, Ticks,
};
use crate::text;

pub fn read(stave: &Node, measures: &[Measure], timeline: &Timeline, styles: &Styles) -> Staff {
    let bars = stave.children("Bars");
    let initial_clef = bars
        .first()
        .and_then(|bar| {
            bar.children("Items")
                .into_iter()
                .filter(|item| is(item, tag::CLEF))
                .next_back()
        })
        .and_then(|item| clef(item, styles));
    let changes = instrument_changes(&bars, measures.len(), styles);
    let instruments: Vec<(usize, Instrument)> = changes
        .iter()
        .map(|&(measure, kind)| (measure, kind.instrument))
        .collect();
    let transposition = |measure: usize| {
        instruments
            .iter()
            .rev()
            .find(|&&(from, _)| from <= measure)
            .map_or_else(Interval::default, |(_, instrument)| instrument.transposition)
    };
    let (mut contents, spans): (Vec<StaffMeasure>, Vec<Vec<Span>>) = measures
        .iter()
        .enumerate()
        .map(|(index, measure)| {
            bars.get(index + 1).map_or_else(Default::default, |bar| {
                self::measure(bar, index, measure.length, transposition(index), styles)
            })
        })
        .unzip();
    if let (Some(clef), Some(first)) = (initial_clef, contents.first_mut()) {
        first.clefs.insert(0, (0, clef));
    }
    let percussion = changes.first().is_some_and(|(_, kind)| kind.percussion);
    let mut staff = Staff {
        percussion: percussion || initial_clef.is_some_and(|clef| clef.sign == "percussion"),
        instruments,
        measures: contents,
    };
    link::resolve(&mut staff, &spans.into_iter().flatten().collect::<Vec<_>>(), timeline);
    staff
}

/// The instrument types a stave switches to, with the measure from which each
/// holds: a change at the end of a bar holds from the next.
fn instrument_changes<'a>(bars: &[&Node], count: usize, styles: &'a Styles) -> Vec<(usize, &'a InstrumentType)> {
    bars.iter()
        .enumerate()
        .flat_map(|(bar, content)| {
            content
                .children("Items")
                .into_iter()
                .filter(|item| is(item, tag::INSTRUMENT_CHANGE))
                .map(move |item| (bar, item))
        })
        .filter_map(|(bar, item)| {
            let at_end = item.number("Sr") == Some(i64::from(BAR_END));
            let measure = if bar == 0 || at_end { bar } else { bar - 1 };
            (measure < count).then_some((measure, styles.instrument_type(item)?))
        })
        .collect()
}

fn is(item: &Node, tag: i64) -> bool {
    item.number("tag") == Some(tag)
}

fn measure(
    bar: &Node,
    index: usize,
    length: Ticks,
    transposition: Interval,
    styles: &Styles,
) -> (StaffMeasure, Vec<Span>) {
    let mut content = StaffMeasure::default();
    let mut spans = Vec::new();
    bar.children("Items").into_iter().for_each(|item| {
        let at = position(item, length);
        match item.number("tag").unwrap_or(-1) {
            tag::NOTE_REST => content.events.push(note_rest(item, at, transposition)),
            tag::BAR_REST => content.events.push(bar_rest(item)),
            tag::CLEF => content.clefs.extend(clef(item, styles).map(|clef| (at, clef))),
            tag::SYMBOL => {
                content.directions.extend(symbol_direction(item, at));
                spans.extend(span(item, tag::SYMBOL, index, at, styles));
            }
            tag::TEXT => match stave_text(item, at, styles) {
                Some(Text::Direction(direction)) => content.directions.push(direction),
                Some(Text::Figures(figures)) => content.figures.push(figures),
                None => {}
            },
            kind => spans.extend(span(item, kind, index, at, styles)),
        }
    });
    content.events.sort_by_key(|event| event.at);
    (content, spans)
}

fn note_rest(item: &Node, at: Ticks, transposition: Interval) -> Event {
    let durn = item.number("Durn").unwrap_or(8) as u8;
    let trem = item.number("Trem").unwrap_or(0) as u8;
    let notes: Vec<Note> = item
        .children("Notes")
        .into_iter()
        .map(|n| note(n, transposition))
        .collect();
    Event {
        at,
        voice: voice(item),
        value: NoteValue {
            log: durn & 0x0f,
            dots: (durn >> 4) & 0x03,
        },
        content: if notes.is_empty() {
            Content::Rest
        } else {
            Content::Chord(notes)
        },
        grace: (durn & 0x40 != 0).then(|| Grace {
            slash: item.flag("SlashThroughStem"),
        }),
        cue: item.flag("CueNoteSize"),
        hidden: item.flag("HideInScore"),
        beam: match trem & 0x03 {
            1 => BeamMode::Start,
            2 => BeamMode::Continue,
            3 => BeamMode::Single,
            _ => BeamMode::None,
        },
        articulations: Articulations(item.number("Artic").unwrap_or(0) as u16),
        tremolo: (trem >> 2) & 0x07,
        ..Event::default()
    }
}

fn bar_rest(item: &Node) -> Event {
    Event {
        voice: voice(item),
        value: NoteValue { log: 10, dots: 0 },
        content: Content::MeasureRest,
        hidden: item.flag("HideInScore"),
        ..Event::default()
    }
}

/// A note of a chord, at written pitch. `Diat` counts diatonic steps from C
/// in octave -1 and `Chrom2` quarter tones from that C, both sounding, so the
/// pitch is spelled by `Diat` and altered by the difference. Where `Diat`
/// means something else, as the string on old tablature staves, the pitch is
/// spelled from `Chrom2` alone.
fn note(item: &Node, transposition: Interval) -> Note {
    let chrom2 = item.number("Chrom2").unwrap_or(120);
    let written = chrom2 - i64::from(transposition.chrom2);
    let spelled = spell(
        item.number("Diat").unwrap_or(35) - i64::from(transposition.diatonic),
        written,
    );
    Note {
        pitch: spelled
            .filter(|pitch| pitch.alter2.abs() <= 4)
            .unwrap_or_else(|| chromatic(written)),
        chrom2: chrom2 as u8,
        tie_start: item.number("Flags").unwrap_or(0) & 0x80 != 0,
        tie_stop: false,
    }
}

const NATURALS: [i64; 7] = [0, 2, 4, 5, 7, 9, 11];

fn spell(diat: i64, chrom2: i64) -> Option<Pitch> {
    let (octave, step) = (diat.div_euclid(7) - 1, diat.rem_euclid(7));
    let natural = 12 * (octave + 1) + NATURALS[step as usize];
    Some(Pitch {
        step: step as u8,
        alter2: i8::try_from(chrom2 - 2 * natural).ok()?,
        octave: octave as i8,
    })
}

/// The usual spelling of a chromatic pitch: sharps for C, F and G, flats for E and B.
fn chromatic(chrom2: i64) -> Pitch {
    const SPELLING: [(u8, i8); 12] = [
        (0, 0),
        (0, 1),
        (1, 0),
        (2, -1),
        (2, 0),
        (3, 0),
        (3, 1),
        (4, 0),
        (4, 1),
        (5, 0),
        (6, -1),
        (6, 0),
    ];
    let semitone = chrom2.div_euclid(2);
    let (step, alter) = SPELLING[semitone.rem_euclid(12) as usize];
    Pitch {
        step,
        alter2: alter * 2 + (chrom2.rem_euclid(2) as i8),
        octave: (semitone.div_euclid(12) - 1) as i8,
    }
}

/// Sibelius clef styles by the start of their id after `clef.`. Tablature is
/// written as guitar music, in the treble clef an octave down.
const CLEFS: [(&str, &str, Option<u8>); 16] = [
    ("percussion", "percussion", None),
    ("tab", "G", Some(2)),
    ("small.tab", "G", Some(2)),
    ("null", "none", None),
    ("violin.french", "G", Some(1)),
    ("french.violin", "G", Some(1)),
    ("mezzo-soprano", "C", Some(2)),
    ("treble", "G", Some(2)),
    ("sub-bass", "F", Some(5)),
    ("baritone.f", "F", Some(3)),
    ("bass", "F", Some(4)),
    ("baritone", "C", Some(5)),
    ("tenor", "C", Some(4)),
    ("alto", "C", Some(3)),
    ("soprano.mezzo", "C", Some(2)),
    ("soprano", "C", Some(1)),
];

fn clef(item: &Node, styles: &Styles) -> Option<Clef> {
    let id = styles.clef(item)?;
    let name = id.strip_prefix("clef.")?;
    let (_, sign, line) = CLEFS
        .iter()
        .find(|(prefix, ..)| name.starts_with(prefix))
        .copied()
        .unwrap_or(("", "G", Some(2)));
    let octave_change = [("up.15", 2), ("up.8", 1), ("down.15", -2), ("down.8", -1), ("tab", -1)]
        .iter()
        .find(|(shift, _)| name.contains(shift))
        .map_or(0, |&(_, change)| change);
    Some(Clef {
        sign,
        line,
        octave_change,
    })
}

/// What a symbol from Sibelius' fixed symbol set means, by its index.
#[derive(Clone, Copy)]
enum Symbol {
    Ornament(&'static str),
    Articulation(u16),
    Segno,
    Coda,
    /// `start` or `stop`.
    Pedal(&'static str),
}

const SYMBOLS: [(i64, Symbol); 38] = [
    (1, Symbol::Segno),
    (13, Symbol::Segno),
    (2, Symbol::Coda),
    (14, Symbol::Coda),
    (48, Symbol::Pedal("start")),
    (49, Symbol::Pedal("start")),
    (50, Symbol::Pedal("stop")),
    (32, Symbol::Ornament("trill-mark")),
    (36, Symbol::Ornament("inverted-mordent")),
    (37, Symbol::Ornament("mordent")),
    (38, Symbol::Ornament("turn")),
    (39, Symbol::Ornament("inverted-turn")),
    (41, Symbol::Ornament("inverted-mordent")),
    (42, Symbol::Ornament("mordent")),
    (43, Symbol::Ornament("mordent")),
    (209, Symbol::Articulation(Articulations::STACCATO)),
    (210, Symbol::Articulation(Articulations::STACCATISSIMO)),
    (211, Symbol::Articulation(Articulations::WEDGE)),
    (212, Symbol::Articulation(Articulations::TENUTO)),
    (213, Symbol::Articulation(Articulations::ACCENT)),
    (214, Symbol::Articulation(Articulations::MARCATO)),
    (215, Symbol::Articulation(Articulations::HARMONIC)),
    (216, Symbol::Articulation(Articulations::PLUS)),
    (217, Symbol::Articulation(Articulations::UP_BOW)),
    (218, Symbol::Articulation(Articulations::DOWN_BOW)),
    (220, Symbol::Articulation(Articulations::SQUARE_PAUSE)),
    (221, Symbol::Articulation(Articulations::PAUSE)),
    (222, Symbol::Articulation(Articulations::TRI_PAUSE)),
    (225, Symbol::Articulation(Articulations::STACCATO)),
    (226, Symbol::Articulation(Articulations::STACCATISSIMO)),
    (227, Symbol::Articulation(Articulations::WEDGE)),
    (228, Symbol::Articulation(Articulations::TENUTO)),
    (229, Symbol::Articulation(Articulations::ACCENT)),
    (230, Symbol::Articulation(Articulations::MARCATO)),
    (231, Symbol::Articulation(Articulations::HARMONIC)),
    (236, Symbol::Articulation(Articulations::SQUARE_PAUSE)),
    (237, Symbol::Articulation(Articulations::PAUSE)),
    (238, Symbol::Articulation(Articulations::TRI_PAUSE)),
];

fn symbol(item: &Node) -> Option<Symbol> {
    let index = item.child("Symbol")?.number("value")?;
    SYMBOLS.iter().find(|(i, _)| *i == index).map(|&(_, symbol)| symbol)
}

fn symbol_direction(item: &Node, at: Ticks) -> Option<Direction> {
    let kind = match symbol(item)? {
        Symbol::Segno => DirectionKind::Segno,
        Symbol::Coda => DirectionKind::Coda,
        Symbol::Pedal(kind) => DirectionKind::Pedal { kind, line: false },
        Symbol::Ornament(_) | Symbol::Articulation(_) => return None,
    };
    (!item.flag("HideInScore")).then(|| Direction {
        at,
        voice: voice(item),
        above: !is_below(item),
        kinds: vec![kind],
        tempo: None,
    })
}

enum Text {
    Direction(Direction),
    Figures(Figures),
}

fn stave_text(item: &Node, at: Ticks, styles: &Styles) -> Option<Text> {
    const NOT_SHOWN: [&str; 3] = ["fingering", "timesig", "instrumentchange.warning"];
    let style = text_style(item, styles)?;
    let raw = item.child("StyledString")?.string("Text")?;
    if item.flag("HideInScore") || raw.starts_with('~') || NOT_SHOWN.iter().any(|s| style.contains(s)) {
        return None;
    }
    let runs = text::runs(&raw, |_| None);
    let voice = voice(item);
    if style.contains("figuredbass") {
        return figures(&text::plain(&runs), at, voice).map(Text::Figures);
    }
    let expression = style.starts_with("text.staff.expression");
    let kinds = match expression {
        true => text::expression(&runs),
        false => text::words(&runs).into_iter().collect(),
    };
    let above = !expression && !is_below(item);
    (!kinds.is_empty()).then_some({
        Text::Direction(Direction {
            at,
            voice,
            above,
            kinds,
            tempo: None,
        })
    })
}

/// Whether an object sits below the middle line, its offset being measured
/// from the top line in 32nds of a space.
fn is_below(item: &Node) -> bool {
    item.signed("Dy").unwrap_or(0) < -64
}

/// Figured bass, one figure per line from the top.
fn figures(text: &str, at: Ticks, voice: u8) -> Option<Figures> {
    let figures: Vec<Figure> = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(figure)
        .collect();
    (!figures.is_empty()).then_some(Figures { at, voice, figures })
}

fn figure(line: &str) -> Figure {
    let (prefix, rest) = match line.chars().next() {
        Some('#') => (Some("sharp"), &line[1..]),
        Some('b') => (Some("flat"), &line[1..]),
        Some('n') => (Some("natural"), &line[1..]),
        _ => (None, line),
    };
    Figure {
        prefix,
        number: (!rest.is_empty()).then(|| rest.to_string()),
    }
}

fn span(item: &Node, kind: i64, measure: usize, at: Ticks, styles: &Styles) -> Option<Span> {
    let line = || styles.line(item).unwrap_or_default();
    let kind = match kind {
        tag::SLUR => SpanKind::Slur,
        tag::TUPLET => SpanKind::Tuplet {
            actual: item.number("Left")? as u8,
            normal: item.number("Right")? as u8,
            length: item.number("TupletLength").unwrap_or(0) as Ticks,
        },
        tag::CRESCENDO => SpanKind::Wedge("crescendo"),
        tag::DIMINUENDO => SpanKind::Wedge("diminuendo"),
        tag::OCTAVA => {
            let line = line();
            SpanKind::Octave {
                kind: if line.contains("minus") { "up" } else { "down" },
                size: if line.contains("15") { 15 } else { 8 },
            }
        }
        tag::PEDAL => SpanKind::Pedal {
            line: !line().contains("noline"),
        },
        tag::TRILL => SpanKind::Ornament("trill-mark"),
        tag::SYMBOL => match symbol(item)? {
            Symbol::Ornament(name) => SpanKind::Ornament(name),
            Symbol::Articulation(bits) => SpanKind::Articulation(bits),
            Symbol::Segno | Symbol::Coda | Symbol::Pedal(_) => return None,
        },
        tag::LYRIC => SpanKind::Lyric(syllable(item, styles)?),
        _ => return None,
    };
    let shapes_time = matches!(kind, SpanKind::Tuplet { .. });
    (shapes_time || !item.flag("HideInScore")).then(|| Span {
        measure,
        at,
        voice: voice(item),
        above: !is_below(item),
        duration: item.number("Duration").unwrap_or(0) as Ticks,
        kind,
    })
}

fn syllable(item: &Node, styles: &Styles) -> Option<Syllable> {
    let styled = item.child("Text")?;
    let text = text::plain(&text::runs(&styled.string("Text")?, |_| None))
        .trim()
        .to_string();
    let style = styles.text(styled).unwrap_or_default();
    let verse = style
        .split('.')
        .find_map(|word| word.strip_prefix("verse")?.parse().ok())
        .unwrap_or(1);
    (!text.is_empty()).then(|| Syllable {
        verse,
        text,
        hyphen: item.number("SyllableType") == Some(0),
        melisma: item.number("NumNotes").unwrap_or(1) > 1,
    })
}
