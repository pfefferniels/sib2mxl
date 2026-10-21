//! Staves grouped into instruments, with their names and brackets.

use libsib::tree::Node;

use super::{Styles, tag};
use crate::fields::Fields;
use crate::model::{Part, PartGroup, Staff};
use crate::text;

pub fn assemble(music: &Node, staves: &[&Node], staffs: Vec<Staff>, styles: &Styles) -> (Vec<Part>, Vec<PartGroup>) {
    let old_instruments = music.children("Instruments");
    let instruments = match old_instruments.is_empty() {
        true => instruments(
            music.children("InstrumentGroups").into_iter().map(grouped_staves),
            staves.len(),
        ),
        false => instruments(
            old_instruments.iter().map(|instrument| listed_staves(instrument)),
            staves.len(),
        ),
    };
    let mut part_of = vec![0; staves.len()];
    instruments
        .iter()
        .enumerate()
        .for_each(|(part, members)| members.iter().for_each(|&stave| part_of[stave] = part));
    let mut staffs: Vec<Option<Staff>> = staffs.into_iter().map(Some).collect();
    let parts = instruments
        .iter()
        .map(|members| {
            let old = old_instruments
                .iter()
                .find(|instrument| listed_staves(instrument).contains(&members[0]));
            let (name, abbreviation) = match old {
                Some(instrument) => (
                    styled(Some(instrument), "FullName"),
                    styled(Some(instrument), "ShortName"),
                ),
                None => instrument_name(staves[members[0]], styles),
            };
            Part {
                name: name.unwrap_or_default(),
                abbreviation: abbreviation.unwrap_or_default(),
                staves: members.iter().filter_map(|&stave| staffs[stave].take()).collect(),
            }
        })
        .collect();
    (parts, part_groups(music, &part_of))
}

fn grouped_staves(grouping: &Node) -> Vec<usize> {
    stave_numbers(grouping).collect()
}

/// The staves of an instrument in the instrument list of the first versions.
fn listed_staves(instrument: &Node) -> Vec<usize> {
    instrument.numbers("Staves").into_iter().map(|n| n as usize).collect()
}

/// The staves of each instrument in score order, given the staves of the
/// instruments that Sibelius lists; every other stave is an instrument.
fn instruments(listed: impl Iterator<Item = Vec<usize>>, count: usize) -> Vec<Vec<usize>> {
    let groups: Vec<Vec<usize>> = listed
        .map(|staves| staves.into_iter().filter(|&stave| stave < count).collect())
        .collect();
    let mut owner: Vec<Option<usize>> = vec![None; count];
    groups.iter().enumerate().for_each(|(group, staves)| {
        staves.iter().for_each(|&stave| {
            owner[stave].get_or_insert(group);
        })
    });
    let mut emitted = vec![false; groups.len()];
    (0..count)
        .filter_map(|stave| match owner[stave] {
            None => Some(vec![stave]),
            Some(group) if !emitted[group] => {
                emitted[group] = true;
                Some((stave..count).filter(|&s| owner[s] == Some(group)).collect())
            }
            Some(_) => None,
        })
        .collect()
}

fn stave_numbers(grouping: &Node) -> impl Iterator<Item = usize> {
    grouping
        .children("Staves")
        .into_iter()
        .filter_map(|stave| stave.number("value"))
        .map(|n| n as usize)
}

/// The names of the instrument a stave starts with: as edited in the score,
/// else as its instrument type defines them.
fn instrument_name(stave: &Node, styles: &Styles) -> (Option<String>, Option<String>) {
    let change = stave.children("Bars").first().and_then(|bar| {
        bar.children("Items")
            .into_iter()
            .find(|item| item.number("tag") == Some(tag::INSTRUMENT_CHANGE))
    });
    let instance = change.and_then(|change| change.child("Instance"));
    let instrument = change.and_then(|change| styles.instrument_type(change));
    let defined = |name: Option<&String>| nonempty(name?.clone());
    let name = styled(instance, "FullName")
        .or_else(|| defined(instrument.and_then(|t| t.full_name.as_ref())))
        .or_else(|| styled(change, "InstrumentText"));
    let abbreviation =
        styled(instance, "ShortName").or_else(|| defined(instrument.and_then(|t| t.short_name.as_ref())));
    (name, abbreviation)
}

/// The plain text of a styled-text field.
fn styled(owner: Option<&Node>, field: &str) -> Option<String> {
    let raw = owner?.child(field)?.string("Text")?;
    nonempty(text::plain(&text::runs(&raw, |_| None)))
}

fn nonempty(text: String) -> Option<String> {
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    (!text.is_empty()).then_some(text)
}

/// Brackets and braces that span more than one part.
fn part_groups(music: &Node, part_of: &[usize]) -> Vec<PartGroup> {
    let brackets = music
        .child("Brackets")
        .map(|list| list.children("items"))
        .unwrap_or_default();
    brackets
        .into_iter()
        .filter_map(|bracket| {
            let parts: Vec<usize> = stave_numbers(bracket)
                .filter_map(|stave| part_of.get(stave).copied())
                .collect();
            let (&first, &last) = (parts.iter().min()?, parts.iter().max()?);
            let symbol = match bracket.number("BracketType") {
                Some(1) => "brace",
                Some(2) => "square",
                _ => "bracket",
            };
            (first < last).then_some(PartGroup { first, last, symbol })
        })
        .collect()
}
