//! What runs between events of a stave: ties, slurs, tuplets, beams, lyrics,
//! and lines that become a start and a stop direction.

use std::cmp::Reverse;
use std::collections::BTreeMap;
use std::ops::Range;

use super::Timeline;
use crate::model::{Beam, BeamMode, Content, Direction, DirectionKind, Event, Lyric, Slur, Staff, Ticks, TupletLevel};

/// An object of a stave that refers to events by position.
pub struct Span {
    pub measure: usize,
    pub at: Ticks,
    pub voice: u8,
    pub above: bool,
    /// From the start to the last event covered (slurs, tuplets) or to the end
    /// of the line.
    pub duration: Ticks,
    pub kind: SpanKind,
}

pub enum SpanKind {
    Slur,
    Tuplet {
        actual: u8,
        normal: u8,
        length: Ticks,
    },
    Wedge(&'static str),
    Octave {
        kind: &'static str,
        size: u8,
    },
    Pedal {
        line: bool,
    },
    Ornament(&'static str),
    /// Articulation bits added by a symbol.
    Articulation(u16),
    Lyric(Syllable),
}

pub struct Syllable {
    pub verse: u8,
    pub text: String,
    /// The word goes on after this syllable.
    pub hyphen: bool,
    pub melisma: bool,
}

/// Positions inside tuplets are rounded to whole ticks.
const SLACK: Ticks = 2;

/// An event by measure and index.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct At {
    measure: usize,
    index: usize,
}

fn event(staff: &Staff, at: At) -> &Event {
    &staff.measures[at.measure].events[at.index]
}

fn event_mut(staff: &mut Staff, at: At) -> &mut Event {
    &mut staff.measures[at.measure].events[at.index]
}

pub fn resolve(staff: &mut Staff, spans: &[Span], timeline: &Timeline) {
    let voices: BTreeMap<u8, Vec<(Ticks, At)>> =
        (1..=4).map(|voice| (voice, in_voice(staff, voice, timeline))).collect();
    tuplets(staff, spans);
    let lengths = timeline.lengths.iter();
    staff
        .measures
        .iter_mut()
        .zip(lengths)
        .for_each(|(measure, &length)| implied_tuplets(&mut measure.events, length));
    staff.measures.iter_mut().for_each(|measure| beams(&mut measure.events));
    voices.values().for_each(|events| ties(staff, events));
    slurs(staff, spans, &voices, timeline);
    lines(staff, spans, timeline);
    marks(staff, spans);
    lyrics(staff, spans);
}

/// The events of one voice in time order, with their absolute positions.
fn in_voice(staff: &Staff, voice: u8, timeline: &Timeline) -> Vec<(Ticks, At)> {
    staff
        .measures
        .iter()
        .enumerate()
        .flat_map(|(measure, content)| {
            content
                .events
                .iter()
                .enumerate()
                .filter(move |(_, event)| event.voice == voice)
                .map(move |(index, event)| (timeline.absolute(measure, event.at), At { measure, index }))
        })
        .collect()
}

/// Marks the events each tuplet covers, outer tuplets before the ones nested in them.
fn tuplets(staff: &mut Staff, spans: &[Span]) {
    let mut tuplets: Vec<(&Span, u8, u8, Ticks)> = spans
        .iter()
        .filter_map(|span| match span.kind {
            SpanKind::Tuplet { actual, normal, length } => Some((span, actual, normal, length)),
            _ => None,
        })
        .collect();
    tuplets.sort_by_key(|&(span, .., length)| (span.measure, Reverse(length)));
    tuplets.into_iter().for_each(|(span, actual, normal, length)| {
        let events = &mut staff.measures[span.measure].events;
        let end = span.at + length.max(span.duration + 1);
        let members: Vec<usize> = events
            .iter()
            .enumerate()
            .filter(|(_, e)| e.voice == span.voice && e.grace.is_none() && e.at + 1 >= span.at && e.at < end)
            .map(|(index, _)| index)
            .collect();
        let (Some(&first), Some(&last)) = (members.first(), members.last()) else {
            return;
        };
        members.iter().for_each(|&index| {
            events[index].tuplets.push(TupletLevel {
                actual,
                normal,
                start: index == first,
                stop: index == last,
            })
        });
    });
}

/// Some files place notes as in a tuplet that is missing: each note starts
/// before the one before it ends. Sibelius goes by the positions, so such a
/// note gets the tuplet ratio, without a bracket, that fits the time it has.
fn implied_tuplets(events: &mut [Event], length: Ticks) {
    let ratios: Vec<(usize, TupletLevel)> = (1..=4)
        .flat_map(|voice| {
            let in_voice: Vec<usize> = (0..events.len())
                .filter(|&i| events[i].voice == voice && events[i].grace.is_none())
                .collect();
            let ends = in_voice.iter().skip(1).map(|&next| events[next].at).chain([length]);
            in_voice
                .iter()
                .zip(ends)
                .filter(|&(&index, _)| {
                    events[index].tuplets.is_empty() && events[index].content != Content::MeasureRest
                })
                .filter_map(|(&index, end)| {
                    Some((
                        index,
                        ratio_for(events[index].value.ticks(), end.checked_sub(events[index].at)?)?,
                    ))
                })
                .collect::<Vec<_>>()
        })
        .collect();
    ratios
        .into_iter()
        .for_each(|(index, level)| events[index].tuplets.push(level));
}

/// The simplest tuplet ratio that shortens `nominal` to `available`, if the
/// note does not fit as it is. Ratios such as 2:1 are no tuplets but a note
/// that overruns the bar.
fn ratio_for(nominal: Ticks, available: Ticks) -> Option<TupletLevel> {
    if available + SLACK >= nominal {
        return None;
    }
    (2..=12u8)
        .flat_map(|actual| (2..actual).map(move |normal| (actual, normal)))
        .find(|&(actual, normal)| {
            (nominal * u32::from(normal)).abs_diff(available * u32::from(actual)) <= u32::from(actual)
        })
        .map(|(actual, normal)| TupletLevel {
            actual,
            normal,
            start: false,
            stop: false,
        })
}

/// Beams in each voice of a measure, from Sibelius' beam modes.
fn beams(events: &mut [Event]) {
    (1..=4).for_each(|voice| {
        let indices: Vec<usize> = (0..events.len())
            .filter(|&i| events[i].voice == voice && events[i].grace.is_none())
            .collect();
        let in_order: Vec<&Event> = indices.iter().map(|&i| &events[i]).collect();
        let beamed: Vec<(usize, Vec<Beam>)> = beam_groups(&in_order)
            .into_iter()
            .flat_map(|group| {
                let levels = beam_levels(&in_order[group.clone()]);
                group.map(|position| indices[position]).zip(levels)
            })
            .collect();
        beamed
            .into_iter()
            .for_each(|(index, beams)| events[index].beams = beams);
    });
}

/// Runs of consecutive events that share a beam: a run starts at a note set
/// to start a beam (or to continue one when none is open) and goes on while
/// notes are set to continue it.
fn beam_groups(events: &[&Event]) -> Vec<Range<usize>> {
    let beamable = |event: &Event| matches!(event.content, Content::Chord(_)) && event.value.log <= 7;
    let mut groups = Vec::new();
    let mut open: Option<usize> = None;
    for (index, event) in events.iter().enumerate() {
        let continues = matches!(event.beam, BeamMode::Continue | BeamMode::Single);
        if open.is_some() && beamable(event) && continues {
            continue;
        }
        groups.extend(open.take().map(|start| start..index));
        if beamable(event) && event.beam != BeamMode::None {
            open = Some(index);
        }
    }
    groups.extend(open.map(|start| start..events.len()));
    groups.retain(|group| group.len() > 1);
    groups
}

/// The beam states of each note in a group, one per level: an eighth has one
/// beam, a sixteenth two, and so on. A note set to `Single` interrupts all but
/// the primary beam.
fn beam_levels(group: &[&Event]) -> Vec<Vec<Beam>> {
    let counts: Vec<u8> = group.iter().map(|event| 8 - event.value.log.min(7)).collect();
    let deepest = counts.iter().copied().max().unwrap_or(1);
    let linked = |a: usize, b: usize, level: u8| {
        counts[a] >= level && counts[b] >= level && (level == 1 || group[b].beam != BeamMode::Single)
    };
    let state = |index: usize, level: u8| {
        let before = index > 0 && linked(index - 1, index, level);
        let after = index + 1 < group.len() && linked(index, index + 1, level);
        match (before, after) {
            (false, true) => Beam::Begin,
            (true, true) => Beam::Continue,
            (true, false) => Beam::End,
            (false, false) if index + 1 == group.len() || (index > 0 && group[index - 1].value.dots > 0) => {
                Beam::BackwardHook
            }
            (false, false) => Beam::ForwardHook,
        }
    };
    (0..group.len())
        .map(|index| {
            (1..=deepest.min(counts[index]))
                .map(|level| state(index, level))
                .collect()
        })
        .collect()
}

/// Ends the ties of one voice on the next chord, on the notes of the same pitch.
fn ties(staff: &mut Staff, voice: &[(Ticks, At)]) {
    let sequence: Vec<At> = voice
        .iter()
        .map(|&(_, at)| at)
        .filter(|&at| event(staff, at).grace.is_none())
        .collect();
    let stops: Vec<(At, u8)> = sequence
        .windows(2)
        .flat_map(|pair| {
            let tied = match &event(staff, pair[0]).content {
                Content::Chord(notes) => notes
                    .iter()
                    .filter(|note| note.tie_start)
                    .map(|note| note.chrom2)
                    .collect(),
                _ => Vec::new(),
            };
            tied.into_iter().map(move |chrom2| (pair[1], chrom2))
        })
        .collect();
    stops.into_iter().for_each(|(at, chrom2)| {
        if let Content::Chord(notes) = &mut event_mut(staff, at).content {
            notes
                .iter_mut()
                .filter(|note| note.chrom2 == chrom2)
                .for_each(|note| note.tie_stop = true);
        }
    });
}

/// Slurs from the event they start on to the last event they reach, numbered
/// so that overlapping slurs differ.
fn slurs(staff: &mut Staff, spans: &[Span], voices: &BTreeMap<u8, Vec<(Ticks, At)>>, timeline: &Timeline) {
    let slurs: Vec<(At, At, Ticks, Ticks)> = spans
        .iter()
        .filter(|span| matches!(span.kind, SpanKind::Slur))
        .filter_map(|span| {
            let events = voices.get(&span.voice)?;
            let start = timeline.absolute(span.measure, span.at);
            let end = start + span.duration;
            let &(first_at, first) = events.iter().find(|(at, _)| at + SLACK >= start)?;
            let &(last_at, last) = events.iter().rev().find(|(at, _)| *at <= end + SLACK)?;
            (first_at < last_at).then_some((first, last, first_at, last_at))
        })
        .collect();
    let intervals: Vec<(Ticks, Ticks)> = slurs.iter().map(|&(.., start, end)| (start, end)).collect();
    slurs
        .iter()
        .zip(number_overlapping(&intervals))
        .for_each(|(&(first, last, ..), number)| {
            event_mut(staff, first).slurs.push(Slur { number, start: true });
            event_mut(staff, last).slurs.push(Slur { number, start: false });
        });
}

/// Numbers for intervals so that intervals that overlap get different ones,
/// the smallest free number first. An interval ending where another starts
/// may share its number.
fn number_overlapping(intervals: &[(Ticks, Ticks)]) -> Vec<u8> {
    let mut order: Vec<usize> = (0..intervals.len()).collect();
    order.sort_by_key(|&i| intervals[i]);
    let mut numbers = vec![1; intervals.len()];
    let mut open: Vec<(Ticks, u8)> = Vec::new();
    for index in order {
        let (start, end) = intervals[index];
        open.retain(|&(until, _)| until > start);
        let number = (1..)
            .find(|n| open.iter().all(|&(_, taken)| taken != *n))
            .expect("numbers are unbounded");
        open.push((end, number));
        numbers[index] = number;
    }
    numbers
}

/// Hairpins, octave lines and pedal lines as a start and a stop direction.
fn lines(staff: &mut Staff, spans: &[Span], timeline: &Timeline) {
    let directions: Vec<(usize, Direction)> = spans
        .iter()
        .filter_map(|span| {
            let (start, stop) = match span.kind {
                SpanKind::Wedge(kind) => (DirectionKind::Wedge(kind), DirectionKind::Wedge("stop")),
                SpanKind::Octave { kind, size } => (
                    DirectionKind::OctaveShift { kind, size },
                    DirectionKind::OctaveShift { kind: "stop", size },
                ),
                SpanKind::Pedal { line } => (
                    DirectionKind::Pedal { kind: "start", line },
                    DirectionKind::Pedal { kind: "stop", line },
                ),
                _ => return None,
            };
            let (end, end_at) = timeline.locate_end(timeline.absolute(span.measure, span.at) + span.duration)?;
            let direction = |at, kind| Direction {
                at,
                voice: span.voice,
                above: span.above,
                kinds: vec![kind],
                tempo: None,
            };
            Some([
                (span.measure, direction(span.at, start)),
                (end, direction(end_at, stop)),
            ])
        })
        .flatten()
        .collect();
    directions
        .into_iter()
        .for_each(|(measure, direction)| staff.measures[measure].directions.push(direction));
}

/// The chord a span starting at `span.at` is attached to.
fn attached(staff: &Staff, span: &Span) -> Option<At> {
    let measure = span.measure;
    staff.measures[measure]
        .events
        .iter()
        .position(|e| {
            e.voice == span.voice
                && e.grace.is_none()
                && matches!(e.content, Content::Chord(_))
                && e.at.abs_diff(span.at) <= SLACK
        })
        .map(|index| At { measure, index })
}

/// Ornaments and articulations that trill lines and symbols add to a chord.
fn marks(staff: &mut Staff, spans: &[Span]) {
    let marked: Vec<(At, &SpanKind)> = spans
        .iter()
        .filter(|span| matches!(span.kind, SpanKind::Ornament(_) | SpanKind::Articulation(_)))
        .filter_map(|span| Some((attached(staff, span)?, &span.kind)))
        .collect();
    marked.into_iter().for_each(|(at, kind)| {
        let event = event_mut(staff, at);
        match *kind {
            SpanKind::Ornament(name) if !event.ornaments.contains(&name) => event.ornaments.push(name),
            SpanKind::Articulation(bits) => event.articulations.0 |= bits,
            _ => {}
        }
    });
}

/// Attaches lyrics to their notes. Whether a syllable begins, continues or
/// ends a word follows from its own hyphen and the one before it in the verse.
fn lyrics(staff: &mut Staff, spans: &[Span]) {
    let mut syllables: Vec<(&Span, &Syllable)> = spans
        .iter()
        .filter_map(|span| match &span.kind {
            SpanKind::Lyric(syllable) => Some((span, syllable)),
            _ => None,
        })
        .collect();
    syllables.sort_by_key(|(span, _)| (span.measure, span.at));
    let mut hyphen_before: BTreeMap<u8, bool> = BTreeMap::new();
    let placed: Vec<(At, Lyric)> = syllables
        .into_iter()
        .filter_map(|(span, syllable)| {
            let continues = hyphen_before.insert(syllable.verse, syllable.hyphen).unwrap_or(false);
            let syllabic = match (continues, syllable.hyphen) {
                (false, false) => "single",
                (false, true) => "begin",
                (true, true) => "middle",
                (true, false) => "end",
            };
            let lyric = Lyric {
                verse: syllable.verse,
                syllabic,
                text: syllable.text.clone(),
                extend: syllable.melisma && !syllable.hyphen,
            };
            Some((attached(staff, span)?, lyric))
        })
        .collect();
    placed
        .into_iter()
        .for_each(|(at, lyric)| event_mut(staff, at).lyrics.push(lyric));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Note, NoteValue, Pitch};

    fn note(log: u8, beam: BeamMode) -> Event {
        let pitch = Pitch {
            step: 0,
            alter2: 0,
            octave: 4,
        };
        Event {
            value: NoteValue { log, dots: 0 },
            content: Content::Chord(vec![Note {
                pitch,
                chrom2: 120,
                tie_start: false,
                tie_stop: false,
            }]),
            beam,
            ..Event::default()
        }
    }

    #[test]
    fn beams_follow_sibelius_modes() {
        use BeamMode::{Continue, Start};
        let events = [
            note(7, Start),
            note(7, Continue),
            note(7, Continue),
            note(7, Start),
            note(8, Continue),
            note(7, Continue),
        ];
        let events: Vec<&Event> = events.iter().collect();
        assert_eq!(beam_groups(&events), vec![0..3]);
    }

    #[test]
    fn secondary_beams_and_hooks() {
        use BeamMode::{Continue, Start};
        let dotted = Event {
            value: NoteValue { log: 7, dots: 1 },
            ..note(7, Start)
        };
        let group = [dotted, note(6, Continue)];
        let group: Vec<&Event> = group.iter().collect();
        assert_eq!(
            beam_levels(&group),
            vec![vec![Beam::Begin], vec![Beam::End, Beam::BackwardHook]]
        );
    }

    #[test]
    fn missing_tuplets_are_implied_by_positions() {
        assert_eq!(ratio_for(64, 42).map(|t| (t.actual, t.normal)), Some((3, 2)));
        assert_eq!(ratio_for(64, 64), None);
        assert_eq!(ratio_for(64, 128), None);
    }

    #[test]
    fn overlapping_intervals_get_distinct_numbers() {
        assert_eq!(
            number_overlapping(&[(0, 10), (5, 20), (10, 30), (25, 40)]),
            vec![1, 2, 1, 2]
        );
    }
}
