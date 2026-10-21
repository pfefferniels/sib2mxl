//! `<note>` elements: one per note of a chord, or one for a rest.

use crate::model::{Articulations, Beam, Content, Event, Note, Pitch};
use crate::xml::Xml;

/// The MusicXML voice of a stream and the staff it is on, if the part has more than one.
pub struct Voice {
    pub number: u8,
    pub staff: Option<usize>,
}

impl Voice {
    pub fn write(&self, xml: &mut Xml) {
        xml.text("voice", &[], &self.number.to_string());
        if let Some(staff) = self.staff {
            xml.text("staff", &[], &staff.to_string());
        }
    }
}

#[derive(Clone, Copy)]
enum Head<'a> {
    Note(&'a Note),
    Rest,
    MeasureRest,
}

pub fn event(xml: &mut Xml, event: &Event, voice: &Voice, duration: u64, percussion: bool) {
    let note = |xml: &mut Xml, head, chord| self::note(xml, event, head, chord, voice, duration, percussion);
    match &event.content {
        Content::Chord(notes) => notes
            .iter()
            .enumerate()
            .for_each(|(index, n)| note(xml, Head::Note(n), index > 0)),
        Content::Rest => note(xml, Head::Rest, false),
        Content::MeasureRest => note(xml, Head::MeasureRest, false),
    }
}

/// One `<note>`. Beams, articulations, slurs, tuplets and lyrics belong to
/// the whole chord and go on its first note only.
fn note(xml: &mut Xml, event: &Event, head: Head, chord: bool, voice: &Voice, duration: u64, percussion: bool) {
    let hidden: &[(&str, &str)] = if event.hidden { &[("print-object", "no")] } else { &[] };
    xml.element("note", hidden, |xml| {
        if let Some(grace) = event.grace {
            xml.empty("grace", if grace.slash { &[("slash", "yes")] } else { &[] });
        }
        if chord {
            xml.empty("chord", &[]);
        }
        match head {
            Head::Note(note) if percussion => unpitched(xml, note.pitch),
            Head::Note(note) => pitch(xml, note.pitch),
            Head::Rest => xml.empty("rest", &[]),
            Head::MeasureRest => xml.empty("rest", &[("measure", "yes")]),
        }
        if event.grace.is_none() {
            xml.text("duration", &[], &duration.to_string());
        }
        ties(head).iter().for_each(|&kind| xml.empty("tie", &[("type", kind)]));
        xml.text("voice", &[], &voice.number.to_string());
        if !matches!(head, Head::MeasureRest) {
            xml.text(
                "type",
                if event.cue { &[("size", "cue")] } else { &[] },
                event.value.type_name(),
            );
            (0..event.value.dots).for_each(|_| xml.empty("dot", &[]));
        }
        time_modification(xml, event);
        if let Some(staff) = voice.staff {
            xml.text("staff", &[], &staff.to_string());
        }
        if !chord {
            beams(xml, &event.beams);
        }
        notations(xml, event, head, chord);
        if !chord {
            lyrics(xml, event);
        }
    });
}

const STEPS: [&str; 7] = ["C", "D", "E", "F", "G", "A", "B"];

fn pitch(xml: &mut Xml, pitch: Pitch) {
    xml.element("pitch", &[], |xml| {
        xml.text("step", &[], STEPS[usize::from(pitch.step)]);
        if pitch.alter2 != 0 {
            let alter = match pitch.alter2 % 2 {
                0 => (pitch.alter2 / 2).to_string(),
                _ => (f64::from(pitch.alter2) / 2.0).to_string(),
            };
            xml.text("alter", &[], &alter);
        }
        xml.text("octave", &[], &pitch.octave.to_string());
    });
}

fn unpitched(xml: &mut Xml, pitch: Pitch) {
    xml.element("unpitched", &[], |xml| {
        xml.text("display-step", &[], STEPS[usize::from(pitch.step)]);
        xml.text("display-octave", &[], &pitch.octave.to_string());
    });
}

fn ties(head: Head) -> Vec<&'static str> {
    match head {
        Head::Note(note) => [(note.tie_stop, "stop"), (note.tie_start, "start")]
            .into_iter()
            .filter_map(|(tied, kind)| tied.then_some(kind))
            .collect(),
        _ => Vec::new(),
    }
}

fn time_modification(xml: &mut Xml, event: &Event) {
    if event.tuplets.is_empty() {
        return;
    }
    let (actual, normal) = super::ratio(event);
    xml.element("time-modification", &[], |xml| {
        xml.text("actual-notes", &[], &actual.to_string());
        xml.text("normal-notes", &[], &normal.to_string());
    });
}

fn beams(xml: &mut Xml, beams: &[Beam]) {
    beams.iter().zip(1..).for_each(|(beam, level)| {
        let state = match beam {
            Beam::Begin => "begin",
            Beam::Continue => "continue",
            Beam::End => "end",
            Beam::ForwardHook => "forward hook",
            Beam::BackwardHook => "backward hook",
        };
        xml.text("beam", &[("number", &level.to_string())], state);
    });
}

const ARTICULATIONS: [(u16, &str); 6] = [
    (Articulations::STACCATO, "staccato"),
    (Articulations::STACCATISSIMO, "staccatissimo"),
    (Articulations::WEDGE, "spiccato"),
    (Articulations::TENUTO, "tenuto"),
    (Articulations::ACCENT, "accent"),
    (Articulations::MARCATO, "strong-accent"),
];

const TECHNICAL: [(u16, &str); 4] = [
    (Articulations::HARMONIC, "harmonic"),
    (Articulations::PLUS, "stopped"),
    (Articulations::UP_BOW, "up-bow"),
    (Articulations::DOWN_BOW, "down-bow"),
];

const FERMATAS: [(u16, &str); 3] = [
    (Articulations::PAUSE, "normal"),
    (Articulations::SQUARE_PAUSE, "square"),
    (Articulations::TRI_PAUSE, "angled"),
];

fn marked(articulations: Articulations, table: &[(u16, &'static str)]) -> Vec<&'static str> {
    table
        .iter()
        .filter(|(bit, _)| articulations.has(*bit))
        .map(|&(_, name)| name)
        .collect()
}

fn notations(xml: &mut Xml, event: &Event, head: Head, chord: bool) {
    let ties = ties(head);
    let whole = (!chord).then_some(event);
    let articulations = whole.map_or_else(Vec::new, |e| marked(e.articulations, &ARTICULATIONS));
    let technical = whole.map_or_else(Vec::new, |e| marked(e.articulations, &TECHNICAL));
    let fermatas = whole.map_or_else(Vec::new, |e| marked(e.articulations, &FERMATAS));
    let ornaments = whole.map_or(&[][..], |e| &e.ornaments);
    let tremolo = whole.map_or(0, |e| e.tremolo);
    let slurs = whole.map_or(&[][..], |e| &e.slurs);
    let tuplets = whole.map_or(&[][..], |e| &e.tuplets);
    let marks_tuplet = tuplets.iter().any(|t| t.start || t.stop);
    if ties.is_empty()
        && articulations.is_empty()
        && technical.is_empty()
        && fermatas.is_empty()
        && ornaments.is_empty()
        && tremolo == 0
        && slurs.is_empty()
        && !marks_tuplet
    {
        return;
    }
    xml.element("notations", &[], |xml| {
        ties.iter().for_each(|&kind| xml.empty("tied", &[("type", kind)]));
        let mut slurs = slurs.to_vec();
        slurs.sort_by_key(|slur| slur.start);
        slurs.iter().for_each(|slur| {
            let kind = if slur.start { "start" } else { "stop" };
            xml.empty("slur", &[("type", kind), ("number", &slur.number.to_string())]);
        });
        let levels = || tuplets.iter().zip(1..);
        levels().filter(|(t, _)| t.start).for_each(|(tuplet, level)| {
            xml.element("tuplet", &[("type", "start"), ("number", &level.to_string())], |xml| {
                [("tuplet-actual", tuplet.actual), ("tuplet-normal", tuplet.normal)]
                    .into_iter()
                    .for_each(|(name, count)| {
                        xml.element(name, &[], |xml| xml.text("tuplet-number", &[], &count.to_string()))
                    });
            })
        });
        levels()
            .filter(|(t, _)| t.stop)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .for_each(|(_, level)| xml.empty("tuplet", &[("type", "stop"), ("number", &level.to_string())]));
        group(xml, "articulations", &articulations);
        group(xml, "technical", &technical);
        if !ornaments.is_empty() || tremolo > 0 {
            xml.element("ornaments", &[], |xml| {
                ornaments.iter().for_each(|ornament| xml.empty(ornament, &[]));
                if tremolo > 0 {
                    xml.text("tremolo", &[("type", "single")], &tremolo.to_string());
                }
            });
        }
        fermatas
            .iter()
            .for_each(|&shape| xml.text("fermata", &[("type", "upright")], shape));
    });
}

fn group(xml: &mut Xml, name: &str, members: &[&str]) {
    if !members.is_empty() {
        xml.element(name, &[], |xml| {
            members.iter().for_each(|member| xml.empty(member, &[]))
        });
    }
}

fn lyrics(xml: &mut Xml, event: &Event) {
    event.lyrics.iter().for_each(|lyric| {
        xml.element("lyric", &[("number", &lyric.verse.to_string())], |xml| {
            xml.text("syllabic", &[], lyric.syllabic);
            xml.text("text", &[], &lyric.text);
            if lyric.extend {
                xml.empty("extend", &[]);
            }
        });
    });
}
