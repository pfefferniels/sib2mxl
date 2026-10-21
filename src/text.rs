//! Sibelius text: characters interleaved with commands between backslashes,
//! such as `\B\` and `\b\` (bold on and off), `\n\` (new line), `\s80\`
//! (size), `\fOpus Text\` (font), `\ctext.character.musictext\` (character
//! style) and `\$Title\` (wildcard).

use crate::model::{self, DirectionKind, Font, Layout, NoteValue, QUARTER};

/// A stretch of text in one format.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Run {
    pub text: String,
    /// Set in a music font, whose letters stand for symbols.
    pub music: bool,
    pub format: Format,
}

/// What commands change from the text style's font; unset means as the style.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Format {
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    pub underline: Option<bool>,
    /// In 32nds of a staff space, as text style heights.
    pub height: Option<u32>,
    pub family: Option<String>,
}

pub fn runs(raw: &str, wildcard: impl Fn(&str) -> Option<String>) -> Vec<Run> {
    let mut music = false;
    let mut format = Format::default();
    raw.split('\\')
        .enumerate()
        .filter_map(|(i, part)| {
            let run = |text: String, music| Run {
                text,
                music,
                format: format.clone(),
            };
            if i % 2 == 0 {
                return (!part.is_empty()).then(|| run(part.to_string(), music));
            }
            match part.split_at_checked(1).unwrap_or(("", "")) {
                ("n" | "N", "") => Some(run("\n".into(), false)),
                ("$", name) => wildcard(name).map(|text| run(text, false)),
                ("B", "") => set(&mut format.bold, true),
                ("b", "") => set(&mut format.bold, false),
                ("I", "") => set(&mut format.italic, true),
                ("i", "") => set(&mut format.italic, false),
                ("U", "") => set(&mut format.underline, true),
                ("u", "") => set(&mut format.underline, false),
                ("s", height) => set(&mut format.height, height.parse().ok()?),
                ("f", "_") => {
                    music = false;
                    format.family = None;
                    None
                }
                ("f", font) => {
                    music = is_music_font(font);
                    format.family = Some(font.to_string());
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

/// Sets a format field from a command, which adds no text.
fn set<T>(field: &mut Option<T>, value: T) -> Option<Run> {
    *field = Some(value);
    None
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
pub fn expression(runs: &[Run], style: &Font, layout: &Layout) -> Vec<DirectionKind> {
    let text = plain(runs);
    let text = text.trim();
    let (head, rest) = text.split_once(char::is_whitespace).unwrap_or((text, ""));
    match (dynamic(text), dynamic(head)) {
        (Some(d), _) => vec![DirectionKind::Dynamics(d)],
        (None, Some(d)) => vec![
            DirectionKind::Dynamics(d),
            DirectionKind::Words(vec![model::Run {
                text: rest.trim().to_string(),
                font: style.clone(),
            }]),
        ],
        _ => words(runs, style, layout).into_iter().collect(),
    }
}

/// Text as words in the fonts it is set in, with note glyphs of music fonts
/// as Unicode note symbols in the reader's font.
pub fn words(runs: &[Run], style: &Font, layout: &Layout) -> Option<DirectionKind> {
    let styled: Vec<model::Run> = runs
        .iter()
        .map(|run| match run.music {
            true => model::Run {
                text: run.text.chars().map(note_symbol).collect(),
                font: Font {
                    family: None,
                    ..font(&run.format, style, layout)
                },
            },
            false => model::Run {
                text: run.text.clone(),
                font: font(&run.format, style, layout),
            },
        })
        .fold(Vec::new(), |mut merged: Vec<model::Run>, run| {
            match merged.last_mut() {
                Some(last) if last.font == run.font => last.text.push_str(&run.text),
                _ => merged.push(run),
            }
            merged
        });
    let trimmed = trim(styled);
    (!trimmed.is_empty()).then_some(DirectionKind::Words(trimmed))
}

/// Plain words in a text style's font.
pub fn plain_words(text: &str, style: &Font) -> DirectionKind {
    DirectionKind::Words(vec![model::Run {
        text: text.trim().to_string(),
        font: style.clone(),
    }])
}

fn font(format: &Format, style: &Font, layout: &Layout) -> Font {
    Font {
        family: format.family.clone().or_else(|| style.family.clone()),
        size: format
            .height
            .map(|height| layout.points(f64::from(height)))
            .or(style.size),
        bold: format.bold.unwrap_or(style.bold),
        italic: format.italic.unwrap_or(style.italic),
        underline: format.underline.unwrap_or(style.underline),
    }
}

/// Runs without the white space around the whole text, and without empty runs.
fn trim(mut runs: Vec<model::Run>) -> Vec<model::Run> {
    if let Some(first) = runs.first_mut() {
        first.text = first.text.trim_start().to_string();
    }
    if let Some(last) = runs.last_mut() {
        last.text = last.text.trim_end().to_string();
    }
    runs.retain(|run| !run.text.is_empty());
    runs
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
pub fn tempo(runs: &[Run], style: &Font, layout: &Layout) -> (Vec<DirectionKind>, Option<f64>) {
    let glyph = runs
        .iter()
        .position(|run| run.music && note_value(run.text.trim()).is_some());
    let as_words = || (words(runs, style, layout).into_iter().collect(), None);
    let Some(glyph) = glyph else {
        return as_words();
    };
    let unit = note_value(runs[glyph].text.trim()).expect("checked");
    let Some(per_minute) = per_minute(&plain(&runs[glyph + 1..])) else {
        return as_words();
    };
    let tempo = per_minute
        .parse::<f64>()
        .ok()
        .map(|n| n * f64::from(unit.ticks()) / f64::from(QUARTER));
    let mut kinds: Vec<_> = words(&runs[..glyph], style, layout).into_iter().collect();
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
    fn formatting_commands_change_the_style_font() {
        let style = Font {
            family: Some("Plantin".into()),
            size: Some(10.0),
            ..Font::default()
        };
        let words = words(&runs("\\B\\loud\\b\\ \\I\\soft", none), &style, &Layout::default());
        let Some(DirectionKind::Words(runs)) = words else {
            panic!("no words");
        };
        let formats: Vec<(&str, bool, bool)> = runs
            .iter()
            .map(|r| (r.text.as_str(), r.font.bold, r.font.italic))
            .collect();
        assert_eq!(
            formats,
            vec![("loud", true, false), (" ", false, false), ("soft", false, true)]
        );
    }

    #[test]
    fn dynamics_are_recognised_in_any_font() {
        let style = Font::default();
        let layout = Layout::default();
        let mf = runs("\\ctext.character.musictext\\\\i\\mf\\f_\\", none);
        assert_eq!(expression(&mf, &style, &layout), vec![DirectionKind::Dynamics("mf")]);
        let mixed = runs("\\fReprise Text Std\\\\i\\p dolce", none);
        assert_eq!(
            expression(&mixed, &style, &layout),
            vec![DirectionKind::Dynamics("p"), plain_words("dolce", &style)]
        );
    }

    #[test]
    fn metronome_marks_give_a_tempo() {
        let (style, layout) = (Font::default(), Layout::default());
        let mark = runs("\\fOpus Text\\q\\fTimes New Roman\\ = 138", none);
        let (kinds, tempo) = self::tempo(&mark, &style, &layout);
        assert_eq!(
            kinds,
            vec![DirectionKind::Metronome {
                unit: NoteValue { log: 8, dots: 0 },
                per_minute: "138".into()
            }]
        );
        assert_eq!(tempo, Some(138.0));
        let half = runs("\\ctext.character.musictext\\h\\c\\ = 88", none);
        assert_eq!(self::tempo(&half, &style, &layout).1, Some(176.0));
    }
}
