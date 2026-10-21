//! The accidentals a staff shows. Sibelius draws them as the key, earlier
//! accidentals in the bar and ties require, unless a note's accidental is set
//! to be hidden, cautionary or bracketed.

use std::collections::HashMap;

use crate::model::{Accidental, AccidentalStyle, Content, Event, Key, Measure, Staff};

pub fn spell_out(staff: &mut Staff, measures: &[Measure]) {
    if staff.percussion {
        return;
    }
    let keys = staff.written_keys(measures);
    staff
        .measures
        .iter_mut()
        .zip(keys)
        .for_each(|(measure, key)| spell_measure(&mut measure.events, key));
}

/// Goes through a measure's notes in time order, all voices together. An
/// accidental holds for its step and octave to the end of the bar; a note
/// tied over the barline shows none and leaves the next one to show it again.
fn spell_measure(events: &mut [Event], key: Key) {
    let mut order: Vec<usize> = (0..events.len()).collect();
    order.sort_by_key(|&index| events[index].at);
    let mut in_force: HashMap<(u8, i8), i8> = HashMap::new();
    order.into_iter().for_each(|index| {
        let Content::Chord(notes) = &mut events[index].content else {
            return;
        };
        notes.iter_mut().for_each(|note| {
            let place = (note.pitch.step, note.pitch.octave);
            let expected = in_force.get(&place).copied().unwrap_or(key.alter2(note.pitch.step));
            let required = note.pitch.alter2 != expected && !note.tie_stop;
            if !note.tie_stop {
                in_force.insert(place, note.pitch.alter2);
            }
            let shown = match note.style {
                AccidentalStyle::Hidden => false,
                AccidentalStyle::Cautionary | AccidentalStyle::Bracketed => true,
                AccidentalStyle::Normal => required,
            };
            note.accidental = shown.then(|| Accidental {
                name: name(note.pitch.alter2),
                cautionary: note.style == AccidentalStyle::Cautionary,
                parentheses: note.style == AccidentalStyle::Bracketed,
            });
        });
    });
}

/// The MusicXML accidental for an alteration in quarter tones.
fn name(alter2: i8) -> &'static str {
    match alter2 {
        -4 => "flat-flat",
        -3 => "three-quarters-flat",
        -2 => "flat",
        -1 => "quarter-flat",
        1 => "quarter-sharp",
        2 => "sharp",
        3 => "three-quarters-sharp",
        4 => "double-sharp",
        _ => "natural",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Note, Pitch};

    fn chord(at: u32, step: u8, alter2: i8, tie_stop: bool) -> Event {
        let pitch = Pitch {
            step,
            alter2,
            octave: 4,
        };
        let note = Note {
            pitch,
            tie_stop,
            ..Note::default()
        };
        Event {
            at,
            content: Content::Chord(vec![note]),
            ..Event::default()
        }
    }

    fn shown(events: &[Event]) -> Vec<Option<&'static str>> {
        events
            .iter()
            .map(|e| match &e.content {
                Content::Chord(notes) => notes[0].accidental.map(|a| a.name),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn accidentals_hold_to_the_end_of_the_bar() {
        // F♯ in G major needs nothing; F natural does, and again F♯ after it.
        let g_major = Key { fifths: 1, major: true };
        let mut events = vec![
            chord(0, 3, 2, false),
            chord(256, 3, 0, false),
            chord(512, 3, 0, false),
            chord(768, 3, 2, false),
        ];
        spell_measure(&mut events, g_major);
        assert_eq!(shown(&events), vec![None, Some("natural"), None, Some("sharp")]);
    }

    #[test]
    fn tied_notes_show_none_and_leave_the_key_in_force() {
        let mut events = vec![chord(0, 0, 2, true), chord(256, 0, 2, false)];
        spell_measure(&mut events, Key::C_MAJOR);
        assert_eq!(shown(&events), vec![None, Some("sharp")]);
    }
}
