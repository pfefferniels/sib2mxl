//! Stem directions. Sibelius stores only whether a stem is flipped from the
//! direction it would have; that direction follows the usual rules, which are
//! reproduced here for the flipped stems: with several voices in a bar, voices
//! 1 and 3 point up and 2 and 4 down; otherwise the note (or, in a chord or
//! beamed group, the note) furthest from the middle line decides, and notes on
//! the line point down. Grace notes point up.

use crate::model::{Beam, Clef, Content, Event, Staff};

pub fn direct(staff: &mut Staff) {
    let mut clef = Clef {
        sign: "G",
        line: Some(2),
        octave_change: 0,
    };
    staff.measures.iter_mut().for_each(|measure| {
        let clefs = measure.clefs.clone();
        let several_voices = voices_with_notes(&measure.events) > 1;
        let middles: Vec<i32> = measure
            .events
            .iter()
            .map(|event| {
                let in_force = clefs
                    .iter()
                    .rev()
                    .find(|(at, _)| *at <= event.at)
                    .map_or(clef, |&(_, c)| c);
                middle_line(in_force)
            })
            .collect();
        let groups = beam_groups(&measure.events);
        let directions: Vec<Option<&'static str>> = (0..measure.events.len())
            .map(|index| {
                let event = &measure.events[index];
                event.flip_stem.then(|| {
                    let up = default_up(&measure.events, index, &groups, &middles, several_voices);
                    if up { "down" } else { "up" }
                })
            })
            .collect();
        measure
            .events
            .iter_mut()
            .zip(directions)
            .for_each(|(event, stem)| event.stem = stem);
        clef = clefs.last().map_or(clef, |&(_, c)| c);
    });
}

fn voices_with_notes(events: &[Event]) -> usize {
    let mut voices: Vec<u8> = events
        .iter()
        .filter(|e| matches!(e.content, Content::Chord(_)))
        .map(|e| e.voice)
        .collect();
    voices.sort_unstable();
    voices.dedup();
    voices.len()
}

/// The diatonic number ((octave + 1) × 7 + step, as Sibelius' `Diat`) of the
/// middle line under a clef.
fn middle_line(clef: Clef) -> i32 {
    let at_clef_line = match clef.sign {
        "F" => 4 * 7 + 3,
        "C" => 5 * 7,
        _ => 5 * 7 + 4,
    };
    let line = i32::from(clef.line.unwrap_or(3));
    at_clef_line + 2 * (3 - line) + 7 * i32::from(clef.octave_change)
}

/// For each event, the indices of the events beamed with it (itself alone if unbeamed).
fn beam_groups(events: &[Event]) -> Vec<Vec<usize>> {
    (0..events.len())
        .map(|index| {
            let event = &events[index];
            if event.beams.is_empty() {
                return vec![index];
            }
            let same_voice: Vec<usize> = (0..events.len())
                .filter(|&i| events[i].voice == event.voice && !events[i].beams.is_empty())
                .collect();
            let position = same_voice.iter().position(|&i| i == index).unwrap_or(0);
            let start = same_voice[..=position]
                .iter()
                .rposition(|&i| events[i].beams[0] == Beam::Begin)
                .unwrap_or(0);
            let end = same_voice[position..]
                .iter()
                .position(|&i| events[i].beams[0] == Beam::End)
                .map_or(same_voice.len() - 1, |offset| position + offset);
            same_voice[start..=end].to_vec()
        })
        .collect()
}

fn default_up(events: &[Event], index: usize, groups: &[Vec<usize>], middles: &[i32], several_voices: bool) -> bool {
    let event = &events[index];
    if event.grace.is_some() {
        return true;
    }
    if several_voices {
        return event.voice % 2 == 1;
    }
    let furthest = groups[index]
        .iter()
        .flat_map(|&i| offsets(&events[i], middles[i]))
        .max_by_key(|offset| (offset.abs(), *offset));
    furthest.is_some_and(|offset| offset < 0)
}

/// How far each note of a chord lies above (positive) or below the middle line.
fn offsets(event: &Event, middle: i32) -> Vec<i32> {
    match &event.content {
        Content::Chord(notes) => notes
            .iter()
            .map(|note| (i32::from(note.pitch.octave) + 1) * 7 + i32::from(note.pitch.step) - middle)
            .collect(),
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn middle_lines_of_common_clefs() {
        let clef = |sign, line, octave_change| Clef {
            sign,
            line: Some(line),
            octave_change,
        };
        assert_eq!(middle_line(clef("G", 2, 0)), 5 * 7 + 6);
        assert_eq!(middle_line(clef("F", 4, 0)), 4 * 7 + 1);
        assert_eq!(middle_line(clef("C", 3, 0)), 5 * 7);
        assert_eq!(middle_line(clef("G", 2, -1)), 4 * 7 + 6);
    }
}
