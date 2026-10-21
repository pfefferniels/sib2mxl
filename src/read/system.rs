//! The system stave: bar lengths, time and key signatures, barlines, repeat
//! endings, and text for the whole system.

use libsib::tree::Node;

use super::{BAR_END, BAR_START, Styles, Summary, Timeline, position, tag, text_style};
use crate::fields::Fields;
use crate::model::{Direction, DirectionKind, Ending, Key, Measure, QUARTER, Repeat, Ticks, Time};
use crate::text;

pub fn measures(music: &Node, system: &Node, summary: &Summary, styles: &Styles) -> Vec<Measure> {
    let bars = system.children("Bars");
    let objects: Vec<(usize, &Node)> = bars
        .iter()
        .enumerate()
        .flat_map(|(bar, items)| items.children("Items").into_iter().map(move |item| (bar, item)))
        .collect();
    let mut measures = signatures_and_lengths(music, &bars);
    objects
        .iter()
        .filter(|(_, item)| item.number("tag") == Some(tag::KEY_SIGNATURE))
        .for_each(|&(bar, item)| place_key(&mut measures, bar, item));
    objects
        .iter()
        .filter(|(bar, item)| *bar > 0 && item.number("tag") == Some(tag::BARLINE))
        .for_each(|&(bar, item)| place_barline(&mut measures[bar - 1], item));
    let timeline = Timeline::new(&measures);
    objects
        .iter()
        .filter(|(bar, item)| *bar > 0 && item.number("tag") == Some(tag::ENDING))
        .for_each(|&(bar, item)| place_ending(&mut measures, &timeline, bar - 1, item, styles));
    directions(summary, styles, &objects, &measures)
        .into_iter()
        .for_each(|(measure, direction)| measures[measure].directions.push(direction));
    if let Some(last) = measures.last_mut().filter(|last| last.right.style.is_none()) {
        last.right.style = Some("light-heavy");
    }
    measures
}

/// Each measure's time signature change and length. The length comes from the
/// spacing table, which records where each bar ends, and else from the time
/// signature in force.
fn signatures_and_lengths(music: &Node, bars: &[&Node]) -> Vec<Measure> {
    let times: Vec<Option<Time>> = bars
        .iter()
        .map(|bar| items(bar, tag::TIME_SIGNATURE).next().map(time))
        .collect();
    let in_force: Vec<Option<Time>> = times
        .iter()
        .scan(None, |current, time| {
            if time.is_some() {
                current.clone_from(time);
            }
            Some(current.clone())
        })
        .collect();
    let spaced = spaced_lengths(music);
    (1..bars.len())
        .map(|bar| {
            let nominal = in_force[bar].as_ref().map_or(4 * QUARTER, |time| time.length);
            let length = spaced.get(bar).copied().flatten().filter(|&l| l > 0).unwrap_or(nominal);
            Measure {
                length,
                pickup: bar == 1 && length < nominal,
                time: times[bar]
                    .clone()
                    .or_else(|| (bar == 1).then(|| times[0].clone()).flatten()),
                ..Measure::default()
            }
        })
        .collect()
}

fn items(bar: &Node, tag: i64) -> impl Iterator<Item = &Node> {
    bar.children("Items")
        .into_iter()
        .filter(move |item| item.number("tag") == Some(tag))
}

/// The end of each bar as the spacing table records it: the last position
/// marked as a bar end before the barline.
fn spaced_lengths(music: &Node) -> Vec<Option<Ticks>> {
    const BAR_END_SPACING: i64 = 0x8000;
    let spacings = music.children("BarSpacings");
    let length = |spacing: &Node| {
        spacing
            .child("Spacings")?
            .children("items")
            .into_iter()
            .filter(|entry| entry.number("Count").is_some_and(|count| count & BAR_END_SPACING != 0))
            .filter_map(|entry| entry.number("Sr"))
            .filter(|&sr| sr < i64::from(BAR_END))
            .max()
            .map(|sr| sr as Ticks)
    };
    let mut lengths = vec![None; spacings.len()];
    spacings.iter().for_each(|spacing| {
        if let Some(slot) = spacing.number("BarNum").and_then(|n| lengths.get_mut(n as usize)) {
            *slot = length(spacing);
        }
    });
    lengths
}

fn time(item: &Node) -> Time {
    let top = item.number("Top").unwrap_or(4);
    let log = item.number("LogUnitLength").unwrap_or(8).clamp(0, 10);
    let display = item.string("TimeSignatureString").unwrap_or_default();
    let numeric = (top.to_string(), (1 << (10 - log)).to_string());
    let (symbol, (beats, beat_type)) = match display.as_str() {
        "c" => (Some("common"), numeric),
        "C" => (Some("cut"), numeric),
        _ => (
            None,
            display
                .split_once("\\n\\")
                .map_or(numeric, |(b, t)| (b.into(), t.into())),
        ),
    };
    Time {
        beats,
        beat_type,
        symbol,
        length: (top << log) as Ticks,
    }
}

/// A key signature in bar 0 holds for measure 1; elsewhere one at the start
/// of a bar changes it, and one at its end (or within it) changes the next.
fn place_key(measures: &mut [Measure], bar: usize, item: &Node) {
    let sr = item.number("Sr").unwrap_or(0) as Ticks;
    let measure = match bar {
        0 => 0,
        _ if sr == 0 || sr == BAR_START => bar - 1,
        _ => bar,
    };
    let key = Key {
        fifths: item.number("Sharps").unwrap_or(0) as i8,
        major: item.number("Major").is_none_or(|major| major != 0),
    };
    if let Some(measure) = measures.get_mut(measure) {
        measure.key = Some(key);
    }
}

fn place_barline(measure: &mut Measure, item: &Node) {
    let sr = item.number("Sr").unwrap_or(0) as Ticks;
    let (style, repeat) = match item.number("BarlineType").unwrap_or(6) {
        0 => ("heavy-light", Some(Repeat::Forward)),
        1 => ("light-heavy", Some(Repeat::Backward)),
        2 => ("dashed", None),
        3 => ("light-light", None),
        4 => ("light-heavy", None),
        5 => ("none", None),
        8 => ("tick", None),
        9 => ("short", None),
        _ => ("regular", None),
    };
    let side = if sr == 0 || sr == BAR_START {
        &mut measure.left
    } else {
        &mut measure.right
    };
    side.style = Some(style);
    side.repeat = side.repeat.or(repeat);
}

/// A repeat ending (first-time bar, …) whose number comes from its line style,
/// such as `line.system.repeat.1st_n_2nd`.
fn place_ending(measures: &mut [Measure], timeline: &Timeline, measure: usize, item: &Node, styles: &Styles) {
    let style = styles.line(item).unwrap_or_default();
    let name = style.trim_start_matches("line.system.repeat.");
    let numbers: Vec<&str> = name.split(['_', '.']).filter_map(ordinal).collect();
    let number = if numbers.is_empty() {
        "1".to_string()
    } else {
        numbers.join(", ")
    };
    let open = ["2nd", "3rd", "open"].contains(&name);
    let start = timeline.absolute(measure, position(item, measures[measure].length));
    let end = timeline
        .locate_end(start + item.number("Duration").unwrap_or(0) as Ticks)
        .map_or(measure, |(end, _)| end);
    measures[measure].left.ending = Some(Ending {
        number: number.clone(),
        kind: "start",
    });
    measures[end].right.ending = Some(Ending {
        number,
        kind: if open { "discontinue" } else { "stop" },
    });
}

/// The number of an ordinal such as `1st` or `2nd`.
fn ordinal(word: &str) -> Option<&str> {
    let digits = word.trim_end_matches(char::is_alphabetic);
    let is_ordinal = !digits.is_empty() && digits.len() < word.len() && digits.chars().all(|c| c.is_ascii_digit());
    is_ordinal.then_some(digits)
}

/// System text, rehearsal marks and tempo lines, with the measure each is in.
fn directions(
    summary: &Summary,
    styles: &Styles,
    objects: &[(usize, &Node)],
    measures: &[Measure],
) -> Vec<(usize, Direction)> {
    let in_measures = || objects.iter().filter(|(bar, _)| *bar > 0 && *bar <= measures.len());
    let direction = |bar: usize, item: &Node, kinds: Vec<DirectionKind>, tempo: Option<f64>| {
        let at = position(item, measures[bar - 1].length);
        (
            bar - 1,
            Direction {
                at,
                voice: 1,
                above: true,
                kinds,
                tempo,
            },
        )
    };
    let texts = in_measures()
        .filter(|(_, item)| item.number("tag") == Some(tag::SYSTEM_TEXT))
        .filter_map(|&(bar, item)| {
            system_text(item, summary, styles).map(|(kinds, tempo)| direction(bar, item, kinds, tempo))
        });
    let rehearsal_marks = in_measures()
        .filter(|(_, item)| item.number("tag") == Some(tag::REHEARSAL_MARK))
        .scan(0, |next, &(bar, item)| {
            let index = match item.flag("Consecutive") {
                true => *next,
                false => item.number("Mark").unwrap_or(0) as u32,
            };
            *next = index + 1;
            Some(direction(
                bar,
                item,
                vec![DirectionKind::Rehearsal(rehearsal_label(item, index))],
                None,
            ))
        });
    let tempo_lines = in_measures()
        .filter(|(_, item)| item.number("tag") == Some(tag::RITARDANDO))
        .filter_map(|&(bar, item)| {
            let text = styles.line_text(&styles.line(item)?)?;
            let words = text::plain(&text::runs(text, |_| None)).trim().to_string();
            (!words.is_empty()).then(|| direction(bar, item, vec![DirectionKind::Words(words)], None))
        });
    texts.chain(rehearsal_marks).chain(tempo_lines).collect()
}

fn system_text(item: &Node, summary: &Summary, styles: &Styles) -> Option<(Vec<DirectionKind>, Option<f64>)> {
    const NOT_IN_MUSIC: [&str; 4] = ["page_aligned", "blankpage", "barnumber", "timesig"];
    let style = text_style(item, styles)?;
    let raw = item.child("StyledString")?.string("Text")?;
    if item.flag("HideInScore") || raw.starts_with('~') || NOT_IN_MUSIC.iter().any(|s| style.contains(s)) {
        return None;
    }
    let runs = text::runs(&raw, |name| summary.get(name));
    let (kinds, tempo) = match ["tempo", "metronome", "metricmodulation"]
        .iter()
        .any(|s| style.contains(s))
    {
        true => text::tempo(&runs),
        false => (text::words(&runs).into_iter().collect(), None),
    };
    (!kinds.is_empty()).then_some((kinds, tempo))
}

/// Letters as Sibelius counts rehearsal marks: A to Z, then AA, BB, ….
fn rehearsal_label(item: &Node, index: u32) -> String {
    let letter = char::from(b'A' + (index % 26) as u8);
    let label = letter.to_string().repeat(index as usize / 26 + 1);
    match item.flag("NewPrefixAndSuffix") {
        true => format!(
            "{}{label}{}",
            item.string("Prefix").unwrap_or_default(),
            item.string("Suffix").unwrap_or_default()
        ),
        false => label,
    }
}
