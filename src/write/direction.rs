//! `<direction>` and `<figured-bass>` elements.

use super::note::Voice;
use crate::model::{Direction, DirectionKind, Figures};
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
        DirectionKind::Words(text) => xml.text("words", &[], text),
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
        DirectionKind::Segno => xml.empty("segno", &[]),
        DirectionKind::Coda => xml.empty("coda", &[]),
        DirectionKind::Pedal { kind, line } => {
            xml.empty("pedal", &[("type", kind), ("line", if *line { "yes" } else { "no" })])
        }
    }
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
            })
        });
    });
}
