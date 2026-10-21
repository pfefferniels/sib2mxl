//! Writes the [`Score`] model as partwise MusicXML 4.0.

mod direction;
mod measure;
mod note;

use crate::model::{Content, Credit, Event, Font, Key, Layout, QUARTER, Score, Ticks};
use crate::xml::Xml;

const DOCTYPE: &str = r#"<!DOCTYPE score-partwise PUBLIC "-//Recordare//DTD MusicXML 4.0 Partwise//EN" "http://www.musicxml.org/dtds/partwise.dtd">"#;

pub fn musicxml(score: &Score) -> String {
    let clock = Clock::for_score(score);
    let mut xml = Xml::new(DOCTYPE);
    xml.element("score-partwise", &[("version", "4.0")], |xml| {
        work(xml, score);
        defaults(xml, &score.layout);
        credits(xml, score);
        part_list(xml, score);
        score.parts.iter().enumerate().for_each(|(part, _)| {
            let keys: Vec<Vec<Key>> = score.parts[part]
                .staves
                .iter()
                .map(|staff| staff.written_keys(&score.measures))
                .collect();
            xml.element("part", &[("id", &part_id(part))], |xml| {
                (0..score.measures.len()).for_each(|measure| measure::write(xml, score, part, measure, &clock, &keys));
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
            [("accidental", "yes"), ("beam", "yes"), ("stem", "no")]
                .into_iter()
                .for_each(|(element, supported)| xml.empty("supports", &[("element", element), ("type", supported)]));
        });
    });
}

fn tenths(value: f64) -> String {
    format!("{}", (value * 100.0).round() / 100.0)
}

fn defaults(xml: &mut Xml, layout: &Layout) {
    xml.element("defaults", &[], |xml| {
        xml.element("scaling", &[], |xml| {
            xml.text("millimeters", &[], &tenths(layout.staff_height));
            xml.text("tenths", &[], "40");
        });
        xml.element("page-layout", &[], |xml| {
            xml.text("page-height", &[], &tenths(layout.tenths(layout.page_height)));
            xml.text("page-width", &[], &tenths(layout.tenths(layout.page_width)));
            xml.element("page-margins", &[("type", "both")], |xml| {
                [
                    ("left-margin", layout.left),
                    ("right-margin", layout.right),
                    ("top-margin", layout.top),
                    ("bottom-margin", layout.bottom),
                ]
                .into_iter()
                .for_each(|(name, mm)| xml.text(name, &[], &tenths(layout.tenths(mm))));
            });
        });
    });
}

/// Credits on the first page: the dedication, title and subtitle centred
/// from the top margin down, the composer and arranger to the right and the
/// lyricist to the left below them, and the copyright at the bottom.
fn credits(xml: &mut Xml, score: &Score) {
    let layout = &score.layout;
    let height = |credit: &Credit| layout.tenths(credit.font.size.unwrap_or(12.0) * 25.4 / 72.0) * 1.3;
    let centred: Vec<&Credit> = ["dedication", "title", "subtitle"]
        .iter()
        .flat_map(|kind| score.credits.iter().filter(move |c| c.kind == *kind))
        .collect();
    let top = layout.tenths(layout.page_height - layout.top);
    let tops: Vec<f64> = centred
        .iter()
        .scan(top, |y, credit| {
            let at = *y;
            *y -= height(credit);
            Some(at)
        })
        .collect();
    let below = top - centred.iter().map(|c| height(c)).sum::<f64>();
    let width = layout.tenths(layout.page_width);
    let placed = centred
        .iter()
        .zip(tops)
        .map(|(credit, y)| (*credit, width / 2.0, y, "center", "top"))
        .chain(score.credits.iter().filter_map(|credit| match credit.kind {
            "composer" | "arranger" => Some((credit, width - layout.tenths(layout.right), below, "right", "top")),
            "lyricist" => Some((credit, layout.tenths(layout.left), below, "left", "top")),
            "rights" => Some((credit, width / 2.0, layout.tenths(layout.bottom), "center", "bottom")),
            _ => None,
        }));
    placed.for_each(|(credit, x, y, justify, valign)| {
        xml.element("credit", &[("page", "1")], |xml| {
            if credit.kind != "dedication" {
                xml.text("credit-type", &[], credit.kind);
            }
            let (x, y) = (tenths(x), tenths(y));
            let mut attributes = vec![
                ("default-x", x.as_str()),
                ("default-y", y.as_str()),
                ("justify", justify),
                ("valign", valign),
            ];
            let font = font_attributes(&credit.font);
            attributes.extend(font.iter().map(|(name, value)| (*name, value.as_str())));
            xml.text("credit-words", &attributes, &credit.text);
        });
    });
}

/// MusicXML font attributes for the parts of a font that are set.
pub fn font_attributes(font: &Font) -> Vec<(&'static str, String)> {
    [
        ("font-family", font.family.clone()),
        ("font-size", font.size.map(|size| format!("{size}"))),
        ("font-weight", font.bold.then(|| "bold".to_string())),
        ("font-style", font.italic.then(|| "italic".to_string())),
        ("underline", font.underline.then(|| "1".to_string())),
    ]
    .into_iter()
    .filter_map(|(name, value)| Some((name, value?)))
    .collect()
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
                ..Note::default()
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
