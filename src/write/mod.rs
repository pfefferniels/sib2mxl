//! Writes the [`Score`] model as partwise MusicXML 4.0.

mod direction;
mod measure;
mod note;

use crate::model::{Content, Event, QUARTER, Score, Ticks};
use crate::xml::Xml;

const DOCTYPE: &str = r#"<!DOCTYPE score-partwise PUBLIC "-//Recordare//DTD MusicXML 4.0 Partwise//EN" "http://www.musicxml.org/dtds/partwise.dtd">"#;

pub fn musicxml(score: &Score) -> String {
    let clock = Clock::for_score(score);
    let mut xml = Xml::new(DOCTYPE);
    xml.element("score-partwise", &[("version", "4.0")], |xml| {
        work(xml, score);
        part_list(xml, score);
        score.parts.iter().enumerate().for_each(|(part, _)| {
            xml.element("part", &[("id", &part_id(part))], |xml| {
                (0..score.measures.len()).for_each(|measure| measure::write(xml, score, part, measure, &clock));
            })
        });
    });
    xml.finish()
}

fn part_id(index: usize) -> String {
    format!("P{}", index + 1)
}

fn work(xml: &mut Xml, score: &Score) {
    let work = &score.work;
    if let Some(title) = &work.title {
        xml.element("work", &[], |xml| xml.text("work-title", &[], title));
    }
    xml.element("identification", &[], |xml| {
        [
            ("composer", &work.composer),
            ("lyricist", &work.lyricist),
            ("arranger", &work.arranger),
        ]
        .into_iter()
        .filter_map(|(kind, name)| Some((kind, name.as_ref()?)))
        .for_each(|(kind, name)| xml.text("creator", &[("type", kind)], name));
        if let Some(rights) = &work.rights {
            xml.text("rights", &[], rights);
        }
        xml.element("encoding", &[], |xml| {
            xml.text("software", &[], concat!("sib2mxl ", env!("CARGO_PKG_VERSION")));
            [("accidental", "no"), ("beam", "yes"), ("stem", "no")]
                .into_iter()
                .for_each(|(element, supported)| xml.empty("supports", &[("element", element), ("type", supported)]));
        });
    });
}

/// The parts, inside the brackets and braces that group them.
fn part_list(xml: &mut Xml, score: &Score) {
    let number = |group: usize| (group + 1).to_string();
    xml.element("part-list", &[], |xml| {
        score.parts.iter().enumerate().for_each(|(index, part)| {
            let mut starting: Vec<usize> = (0..score.groups.len())
                .filter(|&g| score.groups[g].first == index)
                .collect();
            starting.sort_by_key(|&g| std::cmp::Reverse(score.groups[g].last));
            starting.iter().for_each(|&g| {
                xml.element("part-group", &[("type", "start"), ("number", &number(g))], |xml| {
                    xml.text("group-symbol", &[], score.groups[g].symbol)
                })
            });
            xml.element("score-part", &[("id", &part_id(index))], |xml| {
                xml.text("part-name", &[], &part.name);
                if !part.abbreviation.is_empty() {
                    xml.text("part-abbreviation", &[], &part.abbreviation);
                }
            });
            let mut ending: Vec<usize> = (0..score.groups.len())
                .filter(|&g| score.groups[g].last == index)
                .collect();
            ending.sort_by_key(|&g| std::cmp::Reverse(score.groups[g].first));
            ending
                .iter()
                .for_each(|&g| xml.empty("part-group", &[("type", "stop"), ("number", &number(g))]));
        });
    });
}

/// MusicXML time: `divisions` per quarter note, fine enough for every note
/// in a tuplet to last a whole number of them.
pub struct Clock {
    per_tick: u64,
}

impl Clock {
    fn for_score(score: &Score) -> Self {
        let per_tick = score
            .parts
            .iter()
            .flat_map(|part| &part.staves)
            .flat_map(|staff| &staff.measures)
            .flat_map(|measure| &measure.events)
            .map(|event| ratio(event).0)
            .fold(1, lcm);
        Self { per_tick }
    }

    fn divisions(&self) -> u64 {
        u64::from(QUARTER) * self.per_tick
    }

    fn ticks(&self, ticks: Ticks) -> u64 {
        u64::from(ticks) * self.per_tick
    }

    fn duration(&self, event: &Event, measure_length: Ticks) -> u64 {
        match (&event.content, event.grace) {
            (_, Some(_)) => 0,
            (Content::MeasureRest, _) => self.ticks(measure_length),
            _ => {
                let (actual, normal) = ratio(event);
                self.ticks(event.value.ticks()) * normal / actual
            }
        }
    }
}

/// The actual and normal note counts of all tuplets around an event, multiplied.
fn ratio(event: &Event) -> (u64, u64) {
    event.tuplets.iter().fold((1, 1), |(actual, normal), tuplet| {
        (
            actual * u64::from(tuplet.actual.max(1)),
            normal * u64::from(tuplet.normal.max(1)),
        )
    })
}

fn gcd(a: u64, b: u64) -> u64 {
    if b == 0 { a } else { gcd(b, a % b) }
}

fn lcm(a: u64, b: u64) -> u64 {
    a / gcd(a, b) * b
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Measure, Note, NoteValue, Part, Pitch, Staff, StaffMeasure, TupletLevel};

    fn eighth(at: u32, tuplet: TupletLevel) -> Event {
        let pitch = Pitch {
            step: 0,
            alter2: 0,
            octave: 5,
        };
        Event {
            at,
            voice: 1,
            value: NoteValue { log: 7, dots: 0 },
            content: Content::Chord(vec![Note {
                pitch,
                chrom2: 144,
                tie_start: false,
                tie_stop: false,
            }]),
            tuplets: vec![tuplet],
            ..Event::default()
        }
    }

    #[test]
    fn triplets_get_whole_divisions() {
        let triplet = |start, stop| TupletLevel {
            actual: 3,
            normal: 2,
            start,
            stop,
        };
        let events = vec![
            eighth(0, triplet(true, false)),
            eighth(85, triplet(false, false)),
            eighth(170, triplet(false, true)),
        ];
        let score = Score {
            measures: vec![Measure {
                length: 256,
                ..Measure::default()
            }],
            parts: vec![Part {
                name: "Flute".into(),
                staves: vec![Staff {
                    measures: vec![StaffMeasure {
                        events,
                        ..StaffMeasure::default()
                    }],
                    ..Staff::default()
                }],
                ..Part::default()
            }],
            ..Score::default()
        };
        let xml = musicxml(&score);
        assert!(xml.contains("<divisions>768</divisions>"));
        assert_eq!(xml.matches("<duration>256</duration>").count(), 3);
        assert!(!xml.contains("<forward>"));
        assert_eq!(xml.matches("<tuplet type=").count(), 2);
    }
}
