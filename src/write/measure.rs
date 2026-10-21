//! One measure of one part: barlines, attributes, and each voice of each
//! staff in turn, with `<backup>` between them.

use std::collections::BTreeMap;

use super::Clock;
use super::direction;
use super::note::{self, Voice};
use crate::model::{
    Barline, Clef, Direction, Event, Figures, Instrument, Interval, Key, Part, Repeat, Score, Staff, StaffMeasure,
    Ticks,
};
use crate::xml::Xml;

/// Positions inside tuplets are rounded to whole ticks.
const SLACK: Ticks = 2;

pub fn write(xml: &mut Xml, score: &Score, part: usize, index: usize, clock: &Clock) {
    let measure = &score.measures[index];
    let first = usize::from(!score.measures.first().is_some_and(|m| m.pickup));
    let number = (index + first).to_string();
    let mut attributes = vec![("number", number.as_str())];
    if measure.pickup {
        attributes.push(("implicit", "yes"));
    }
    xml.element("measure", &attributes, |xml| {
        barline(xml, "left", &measure.left);
        self::attributes(xml, score, &score.parts[part], index, clock);
        voices(xml, score, part, index, clock);
        barline(xml, "right", &measure.right);
    });
}

fn barline(xml: &mut Xml, location: &str, barline: &Barline) {
    if barline.is_empty() {
        return;
    }
    xml.element("barline", &[("location", location)], |xml| {
        if let Some(style) = barline.style {
            xml.text("bar-style", &[], style);
        }
        if let Some(ending) = &barline.ending {
            let text = match ending.kind {
                "start" => format!("{}.", ending.number),
                _ => String::new(),
            };
            xml.text("ending", &[("number", &ending.number), ("type", ending.kind)], &text);
        }
        if let Some(repeat) = barline.repeat {
            let direction = match repeat {
                Repeat::Forward => "forward",
                Repeat::Backward => "backward",
            };
            xml.empty("repeat", &[("direction", direction)]);
        }
    });
}

/// Divisions and staves in the first measure; key, time, clefs and
/// transpositions where they change.
fn attributes(xml: &mut Xml, score: &Score, part: &Part, index: usize, clock: &Clock) {
    let measure = &score.measures[index];
    let staves = part.staves.len();
    let clefs: Vec<(usize, &Clef)> = part
        .staves
        .iter()
        .enumerate()
        .flat_map(|(staff, content)| {
            content.measures[index]
                .clefs
                .iter()
                .filter(|(at, _)| *at == 0)
                .map(move |(_, clef)| (staff, clef))
        })
        .collect();
    let changes: Vec<(usize, Instrument)> = part
        .staves
        .iter()
        .enumerate()
        .filter_map(|(staff, content)| {
            let (_, instrument) = content.instruments.iter().find(|(from, _)| *from == index)?;
            Some((staff, *instrument))
        })
        .collect();
    let first = index == 0;
    if !first && measure.key.is_none() && measure.time.is_none() && clefs.is_empty() && changes.is_empty() {
        return;
    }
    let instrument = part
        .staves
        .first()
        .map_or_else(Instrument::default, |staff| instrument_at(staff, index));
    let key = measure.key.or_else(|| {
        let keys = score.measures[..=index].iter().rev().find_map(|m| m.key);
        let transposed = instrument != Instrument::default();
        keys.or(transposed.then_some(Key { fifths: 0, major: true }))
            .filter(|_| first || changes.iter().any(|&(staff, _)| staff == 0))
    });
    xml.element("attributes", &[], |xml| {
        if first {
            xml.text("divisions", &[], &clock.divisions().to_string());
        }
        if let Some(key) = key.map(|key| written_key(key, instrument)) {
            xml.element("key", &[], |xml| {
                xml.text("fifths", &[], &key.fifths.to_string());
                xml.text("mode", &[], if key.major { "major" } else { "minor" });
            });
        }
        if let Some(time) = &measure.time {
            let symbol: Vec<(&str, &str)> = time.symbol.map(|symbol| ("symbol", symbol)).into_iter().collect();
            xml.element("time", &symbol, |xml| {
                xml.text("beats", &[], &time.beats);
                xml.text("beat-type", &[], &time.beat_type);
            });
        }
        if first && staves > 1 {
            xml.text("staves", &[], &staves.to_string());
        }
        clefs
            .iter()
            .for_each(|&(staff, clef)| self::clef(xml, clef, (staves > 1).then_some(staff + 1)));
        changes
            .iter()
            .filter(|(_, instrument)| !first || instrument.transposition != Interval::default())
            .for_each(|&(staff, instrument)| {
                transpose(xml, instrument.transposition, (staves > 1).then_some(staff + 1))
            });
    });
}

fn instrument_at(staff: &Staff, measure: usize) -> Instrument {
    staff
        .instruments
        .iter()
        .rev()
        .find(|(from, _)| *from <= measure)
        .map_or_else(Instrument::default, |&(_, instrument)| instrument)
}

/// The key as written for an instrument: transposed, enharmonically within
/// seven sharps or flats, or none for instruments written without key signatures.
fn written_key(concert: Key, instrument: Instrument) -> Key {
    if !instrument.key_signature {
        return Key { fifths: 0, major: true };
    }
    let fifths = match i32::from(concert.fifths) + instrument.transposition.key_shift() {
        sharps if sharps > 7 => sharps - 12,
        flats if flats < -7 => flats + 12,
        fifths => fifths,
    };
    Key {
        fifths: fifths as i8,
        ..concert
    }
}

/// The interval from written to sounding pitch, in whole octaves and the rest.
fn transpose(xml: &mut Xml, interval: Interval, staff: Option<usize>) {
    let semitones = interval.chrom2 / 2;
    let octaves = semitones / 12;
    let number = staff.map(|staff| staff.to_string());
    let attributes: Vec<(&str, &str)> = number.iter().map(|n| ("number", n.as_str())).collect();
    xml.element("transpose", &attributes, |xml| {
        xml.text("diatonic", &[], &(interval.diatonic - 7 * octaves).to_string());
        xml.text("chromatic", &[], &(semitones - 12 * octaves).to_string());
        if octaves != 0 {
            xml.text("octave-change", &[], &octaves.to_string());
        }
    });
}

fn clef(xml: &mut Xml, clef: &Clef, staff: Option<usize>) {
    let number = staff.map(|staff| staff.to_string());
    let attributes: Vec<(&str, &str)> = number.iter().map(|n| ("number", n.as_str())).collect();
    xml.element("clef", &attributes, |xml| {
        xml.text("sign", &[], clef.sign);
        if let Some(line) = clef.line {
            xml.text("line", &[], &line.to_string());
        }
        if clef.octave_change != 0 {
            xml.text("clef-octave-change", &[], &clef.octave_change.to_string());
        }
    });
}

/// What one voice of one staff holds in a measure, in time order.
struct Stream<'a> {
    voice: Voice,
    percussion: bool,
    items: Vec<(Ticks, Item<'a>)>,
}

#[derive(Clone, Copy)]
enum Item<'a> {
    Mark(Mark<'a>),
    Event(&'a Event),
}

/// What sits at a position without taking time.
#[derive(Clone, Copy)]
enum Mark<'a> {
    Clef(&'a Clef),
    Direction(&'a Direction),
    Figures(&'a Figures),
}

impl Item<'_> {
    /// Items at the same position go in this order.
    fn rank(self) -> u8 {
        match self {
            Item::Mark(Mark::Clef(_)) => 0,
            Item::Mark(Mark::Direction(_)) => 1,
            Item::Mark(Mark::Figures(_)) => 2,
            Item::Event(_) => 3,
        }
    }
}

fn voices(xml: &mut Xml, score: &Score, part: usize, index: usize, clock: &Clock) {
    let staves = &score.parts[part].staves;
    let system = &score.measures[index].directions;
    let streams: Vec<Stream> = staves
        .iter()
        .enumerate()
        .flat_map(|(staff, content)| {
            let system = if part == 0 && staff == 0 {
                system.as_slice()
            } else {
                Default::default()
            };
            let voice = |voice: u8| Voice {
                number: staff as u8 * 4 + voice,
                staff: (staves.len() > 1).then_some(staff + 1),
            };
            streams(&content.measures[index], system, voice, content.percussion)
        })
        .collect();
    let measure_length = score.measures[index].length;
    let length = clock.ticks(measure_length);
    if streams.is_empty() {
        forward(xml, length, None);
        return;
    }
    let last = streams.len() - 1;
    streams.iter().enumerate().for_each(|(position, stream)| {
        let mut end = self::stream(xml, stream, clock, measure_length);
        if position == 0 && end < length {
            forward(xml, length - end, Some(&stream.voice));
            end = length;
        }
        if position < last && end > 0 {
            backup(xml, end);
        }
    });
}

fn streams<'a>(
    content: &'a StaffMeasure,
    system: &'a [Direction],
    voice: impl Fn(u8) -> Voice,
    percussion: bool,
) -> Vec<Stream<'a>> {
    let events = content.events.iter().map(|e| (e.voice, e.at, Item::Event(e)));
    let directions = content
        .directions
        .iter()
        .chain(system)
        .map(|d| (d.voice, d.at, Item::Mark(Mark::Direction(d))));
    let figures = content
        .figures
        .iter()
        .map(|f| (f.voice, f.at, Item::Mark(Mark::Figures(f))));
    let clefs = content
        .clefs
        .iter()
        .filter(|(at, _)| *at > 0)
        .map(|(at, clef)| (1, *at, Item::Mark(Mark::Clef(clef))));
    let mut by_voice: BTreeMap<u8, Vec<(Ticks, Item)>> = BTreeMap::new();
    events
        .chain(directions)
        .chain(figures)
        .chain(clefs)
        .for_each(|(v, at, item)| by_voice.entry(v).or_default().push((at, item)));
    by_voice
        .into_iter()
        .map(|(v, mut items)| {
            items.sort_by_key(|&(at, item)| (at, item.rank()));
            Stream {
                voice: voice(v),
                percussion,
                items,
            }
        })
        .collect()
}

/// Writes a stream and returns where it ends. Events follow one another; a
/// gap becomes a `<forward>`. Other items sit at their position, reached by
/// moving forward, or back into the event sounding there and out again.
fn stream(xml: &mut Xml, stream: &Stream, clock: &Clock, measure_length: Ticks) -> u64 {
    let slack = clock.ticks(SLACK);
    stream.items.iter().fold(0, |now, &(at, item)| {
        let target = clock.ticks(at);
        match item {
            Item::Event(event) => {
                let now = move_to(xml, now, target, slack, &stream.voice);
                let duration = clock.duration(event, measure_length);
                note::event(xml, event, &stream.voice, duration, stream.percussion);
                now + duration
            }
            Item::Mark(mark) if target + slack < now => {
                backup(xml, now - target);
                write_mark(xml, mark, &stream.voice);
                forward(xml, now - target, Some(&stream.voice));
                now
            }
            Item::Mark(mark) => {
                let now = move_to(xml, now, target, slack, &stream.voice);
                write_mark(xml, mark, &stream.voice);
                now
            }
        }
    })
}

fn move_to(xml: &mut Xml, now: u64, target: u64, slack: u64, voice: &Voice) -> u64 {
    if target > now + slack {
        forward(xml, target - now, Some(voice));
        target
    } else if target + slack < now {
        backup(xml, now - target);
        target
    } else {
        now
    }
}

fn write_mark(xml: &mut Xml, mark: Mark, voice: &Voice) {
    match mark {
        Mark::Clef(clef) => xml.element("attributes", &[], |xml| self::clef(xml, clef, voice.staff)),
        Mark::Direction(d) => direction::direction(xml, d, voice),
        Mark::Figures(figures) => direction::figured_bass(xml, figures),
    }
}

fn forward(xml: &mut Xml, duration: u64, voice: Option<&Voice>) {
    xml.element("forward", &[], |xml| {
        xml.text("duration", &[], &duration.to_string());
        if let Some(voice) = voice {
            voice.write(xml);
        }
    });
}

fn backup(xml: &mut Xml, duration: u64) {
    xml.element("backup", &[], |xml| xml.text("duration", &[], &duration.to_string()));
}
