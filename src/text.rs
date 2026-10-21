//! Sibelius text: characters interleaved with commands between backslashes,
//! such as `\B\` (bold), `\n\` (new line), `\fOpus Text\` (font),
//! `\ctext.character.musictext\` (character style) and `\$Title\` (wildcard).

use crate::model::{DirectionKind, NoteValue, QUARTER};

/// A stretch of text in one font, which is either a music font or not.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Run {
    pub text: String,
    pub music: bool,
}

pub fn runs(raw: &str, wildcard: impl Fn(&str) -> Option<String>) -> Vec<Run> {
    let mut music = false;
    raw.split('\\')
        .enumerate()
        .filter_map(|(i, part)| {
            if i % 2 == 0 {
                return (!part.is_empty()).then(|| Run {
                    text: part.to_string(),
                    music,
                });
            }
            match part.split_at_checked(1).unwrap_or(("", "")) {
                ("n" | "N", "") => Some(Run {
                    text: "\n".into(),
                    music: false,
                }),
                ("$", name) => wildcard(name).map(|text| Run { text, music: false }),
                ("f", font) => {
                    music = is_music_font(font);
                    None
                }
                ("c", style) => {
                    music = style.contains("musictext");
                    None
                }
                _ => None,
            }
        })
        .collect()
}

pub fn plain(runs: &[Run]) -> String {
    runs.iter().map(|run| run.text.as_str()).collect()
}

fn is_music_font(font: &str) -> bool {
    ["Opus", "Helsinki", "Reprise", "Inkpen2", "Bravura", "Petaluma"]
        .iter()
        .any(|prefix| font.starts_with(prefix))
}

/// Text written in Sibelius' text styles that hold dynamics.
const DYNAMICS: [&str; 27] = [
    "pppppp", "ppppp", "pppp", "ppp", "pp", "p", "mp", "mf", "f", "ff", "fff", "ffff", "fffff", "ffffff", "fp", "pf",
    "sf", "sfp", "sfpp", "sfz", "sffz", "sfzp", "fz", "rf", "rfz", "n", "sfzz",
];

pub fn dynamic(text: &str) -> Option<&'static str> {
    DYNAMICS.iter().find(|&&d| d == text.trim()).copied()
}

/// Expression text: a leading dynamic, if any, and the rest as words.
pub fn expression(runs: &[Run]) -> Vec<DirectionKind> {
    let text = plain(runs);
    let text = text.trim();
    let (head, rest) = text.split_once(char::is_whitespace).unwrap_or((text, ""));
    match (dynamic(text), dynamic(head)) {
        (Some(d), _) => vec![DirectionKind::Dynamics(d)],
        (None, Some(d)) => vec![
            DirectionKind::Dynamics(d),
            DirectionKind::Words(rest.trim().to_string()),
        ],
        _ => words(runs).into_iter().collect(),
    }
}

/// Plain text, with note glyphs of music fonts as Unicode note symbols.
pub fn words(runs: &[Run]) -> Option<DirectionKind> {
    let text: String = runs
        .iter()
        .map(|run| match run.music {
            true => run.text.chars().map(note_symbol).collect(),
            false => run.text.clone(),
        })
        .collect();
    let text = text.trim();
    (!text.is_empty()).then(|| DirectionKind::Words(text.to_string()))
}

fn note_symbol(glyph: char) -> String {
    match glyph {
        'w' => "𝅝".into(),
        'h' => "𝅗𝅥".into(),
        'q' => "♩".into(),
        'e' => "♪".into(),
        'x' => "𝅘𝅥𝅯".into(),
        other => other.to_string(),
    }
}

/// A metronome mark such as `♩ = 120`, possibly after words, with its tempo
/// in quarter notes per minute.
pub fn tempo(runs: &[Run]) -> (Vec<DirectionKind>, Option<f64>) {
    let glyph = runs
        .iter()
        .position(|run| run.music && note_value(run.text.trim()).is_some());
    let Some(glyph) = glyph else {
        return (words(runs).into_iter().collect(), None);
    };
    let unit = note_value(runs[glyph].text.trim()).expect("checked");
    let after = plain(&runs[glyph + 1..]);
    let Some(per_minute) = per_minute(&after) else {
        return (words(runs).into_iter().collect(), None);
    };
    let tempo = per_minute
        .parse::<f64>()
        .ok()
        .map(|n| n * f64::from(unit.ticks()) / f64::from(QUARTER));
    let mut kinds: Vec<_> = words(&runs[..glyph]).into_iter().collect();
    kinds.push(DirectionKind::Metronome { unit, per_minute });
    (kinds, tempo)
}

fn note_value(glyph: &str) -> Option<NoteValue> {
    let (symbol, dots) = match glyph.strip_suffix('.') {
        Some(symbol) => (symbol, 1),
        None => (glyph, 0),
    };
    let log = match symbol {
        "w" => 10,
        "h" => 9,
        "q" => 8,
        "e" => 7,
        "x" => 6,
        _ => return None,
    };
    Some(NoteValue { log, dots })
}

fn per_minute(text: &str) -> Option<String> {
    let number = text.trim_start().strip_prefix('=')?.trim_start();
    let end = number
        .find(|c: char| !c.is_ascii_digit() && c != '.')
        .unwrap_or(number.len());
    (end > 0).then(|| number[..end].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn none(_: &str) -> Option<String> {
        None
    }

    #[test]
    fn commands_are_dropped_and_lines_kept() {
        assert_eq!(plain(&runs("\\B\\\\I\\simile", none)), "simile");
        assert_eq!(plain(&runs("6\\n\\4", none)), "6\n4");
        assert_eq!(
            plain(&runs("\\$Title\\ (1)", |n| Some(format!("<{n}>")))),
            "<Title> (1)"
        );
    }

    #[test]
    fn dynamics_are_recognised_in_any_font() {
        let mf = runs("\\ctext.character.musictext\\\\i\\mf\\f_\\", none);
        assert_eq!(expression(&mf), vec![DirectionKind::Dynamics("mf")]);
        let mixed = runs("\\fReprise Text Std\\\\i\\p dolce", none);
        assert_eq!(
            expression(&mixed),
            vec![DirectionKind::Dynamics("p"), DirectionKind::Words("dolce".into())]
        );
    }

    #[test]
    fn metronome_marks_give_a_tempo() {
        let mark = runs("\\fOpus Text\\q\\fTimes New Roman\\ = 138", none);
        let (kinds, tempo) = self::tempo(&mark);
        assert_eq!(
            kinds,
            vec![DirectionKind::Metronome {
                unit: NoteValue { log: 8, dots: 0 },
                per_minute: "138".into()
            }]
        );
        assert_eq!(tempo, Some(138.0));
        let half = runs("\\ctext.character.musictext\\h\\c\\ = 88", none);
        assert_eq!(self::tempo(&half).1, Some(176.0));
    }
}
