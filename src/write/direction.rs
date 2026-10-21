//! `<direction>` and `<figured-bass>` elements.

use super::note::Voice;
use crate::model::{Direction, DirectionKind, Figures, Harmony, Spelled};
use crate::xml::Xml;

pub fn direction(xml: &mut Xml, direction: &Direction, voice: &Voice) {
    let placement = if direction.above { "above" } else { "below" };
    xml.element("direction", &[("placement", placement)], |xml| {
        direction
            .kinds
            .iter()
            .for_each(|kind| xml.element("direction-type", &[], |xml| self::kind(xml, kind)));
        voice.write(xml);
        if let Some(tempo) = direction.tempo {
            xml.empty("sound", &[("tempo", &format!("{}", (tempo * 100.0).round() / 100.0))]);
        }
    });
}

fn kind(xml: &mut Xml, kind: &DirectionKind) {
    match kind {
        DirectionKind::Words(runs) => runs.iter().for_each(|run| {
            let font = super::font_attributes(&run.font);
            let attributes: Vec<(&str, &str)> = font.iter().map(|(name, value)| (*name, value.as_str())).collect();
            xml.text("words", &attributes, &run.text);
        }),
        DirectionKind::Dynamics(dynamic) => xml.element("dynamics", &[], |xml| xml.empty(dynamic, &[])),
        DirectionKind::Rehearsal(text) => xml.text("rehearsal", &[], text),
        DirectionKind::Metronome { unit, per_minute } => xml.element("metronome", &[], |xml| {
            xml.text("beat-unit", &[], unit.type_name());
            (0..unit.dots).for_each(|_| xml.empty("beat-unit-dot", &[]));
            xml.text("per-minute", &[], per_minute);
        }),
        DirectionKind::Wedge(kind) => xml.empty("wedge", &[("type", kind)]),
        DirectionKind::OctaveShift { kind, size } => {
            xml.empty("octave-shift", &[("type", kind), ("size", &size.to_string())])
        }
        DirectionKind::Bracket {
            kind,
            number,
            line_end,
            line_type,
        } => xml.empty(
            "bracket",
            &[
                ("type", kind),
                ("number", &number.to_string()),
                ("line-end", line_end),
                ("line-type", line_type),
            ],
        ),
        DirectionKind::Segno => xml.empty("segno", &[]),
        DirectionKind::Coda => xml.empty("coda", &[]),
        DirectionKind::Pedal { kind, line } => {
            xml.empty("pedal", &[("type", kind), ("line", if *line { "yes" } else { "no" })])
        }
    }
}

const STEPS: [&str; 7] = ["C", "D", "E", "F", "G", "A", "B"];

/// A chord symbol. "No chord" still needs a root, which it hides.
pub fn harmony(xml: &mut Xml, harmony: &Harmony, voice: &Voice) {
    xml.element("harmony", &[("print-frame", "no")], |xml| {
        match harmony.root {
            Some(root) => note_name(xml, "root", root),
            None => xml.element("root", &[], |xml| xml.text("root-step", &[("text", "")], "C")),
        }
        xml.text("kind", &[("text", &harmony.text)], harmony.kind);
        if let Some(bass) = harmony.bass {
            note_name(xml, "bass", bass);
        }
        harmony.degrees.iter().for_each(|degree| {
            xml.element("degree", &[], |xml| {
                xml.text("degree-value", &[], &degree.value.to_string());
                xml.text("degree-alter", &[], &degree.alter.to_string());
                xml.text("degree-type", &[], degree.kind);
            })
        });
        if let Some(frame) = &harmony.frame {
            xml.element("frame", &[], |xml| {
                xml.text("frame-strings", &[], &frame.strings.to_string());
                xml.text("frame-frets", &[], &frame.frets.to_string());
                if frame.first_fret > 1 {
                    xml.text("first-fret", &[], &frame.first_fret.to_string());
                }
                frame.notes.iter().for_each(|note| {
                    xml.element("frame-note", &[], |xml| {
                        xml.text("string", &[], &note.string.to_string());
                        xml.text("fret", &[], &note.fret.to_string());
                        if let Some(barre) = note.barre {
                            xml.empty("barre", &[("type", barre)]);
                        }
                    })
                });
            });
        }
        if let Some(staff) = voice.staff {
            xml.text("staff", &[], &staff.to_string());
        }
    });
}

/// `<root>` or `<bass>` with its step and alteration.
fn note_name(xml: &mut Xml, element: &str, name: Spelled) {
    xml.element(element, &[], |xml| {
        xml.text(&format!("{element}-step"), &[], STEPS[usize::from(name.step)]);
        if name.alter != 0 {
            xml.text(&format!("{element}-alter"), &[], &name.alter.to_string());
        }
    });
}

pub fn figured_bass(xml: &mut Xml, figures: &Figures) {
    xml.element("figured-bass", &[], |xml| {
        figures.figures.iter().for_each(|figure| {
            xml.element("figure", &[], |xml| {
                if let Some(prefix) = figure.prefix {
                    xml.text("prefix", &[], prefix);
                }
                if let Some(number) = &figure.number {
                    xml.text("figure-number", &[], number);
                }
                if let Some(suffix) = figure.suffix {
                    xml.text("suffix", &[], suffix);
                }
                if figure.extend {
                    xml.empty("extend", &[]);
                }
            })
        });
    });
}
