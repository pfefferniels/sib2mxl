//! Chord symbols: Sibelius names a chord type by an id such as `m9`,
//! `maj7#11` or `13b9b5`, a quality and extension followed by alterations and
//! added notes. MusicXML describes the same chord by a kind and degrees.

use crate::model::{Degree, Harmony, Spelled, Ticks};

/// A chord symbol on `root` and over `bass`, each a pitch class with the
/// diatonic step (0 is C) it is spelled on.
pub fn harmony(at: Ticks, root: (i64, i64), bass: Option<(i64, i64)>, id: &str) -> Harmony {
    let (kind, implied, rest) = kind(id);
    let chord = kind != "none";
    Harmony {
        at,
        root: chord.then(|| spelled(root)),
        kind,
        text: display(id),
        bass: bass.filter(|_| chord).map(spelled),
        degrees: implied
            .iter()
            .map(|&value| Degree {
                value,
                alter: 0,
                kind: "add",
            })
            .chain(degrees(rest, extension(kind)))
            .collect(),
        frame: None,
    }
}

fn spelled((class, step): (i64, i64)) -> Spelled {
    const NATURALS: [i64; 7] = [0, 2, 4, 5, 7, 9, 11];
    let step = step.rem_euclid(7);
    let alter = (class - NATURALS[step as usize] + 6).rem_euclid(12) - 6;
    Spelled {
        step: step as u8,
        alter: alter as i8,
    }
}

/// Chord qualities by the start of their id, each before the qualities whose
/// id is a prefix of its own.
const QUALITIES: [(&str, &str); 30] = [
    ("nc", "none"),
    ("halfdim", "half-diminished"),
    ("m7b5", "half-diminished"),
    ("dim7", "diminished-seventh"),
    ("dim", "diminished"),
    ("aug7", "augmented-seventh"),
    ("aug", "augmented"),
    ("mmaj7", "major-minor"),
    ("m(maj7)", "major-minor"),
    ("maj13", "major-13th"),
    ("maj11", "major-11th"),
    ("maj9", "major-ninth"),
    ("maj7", "major-seventh"),
    ("maj", "major"),
    ("min", "minor"),
    ("m13", "minor-13th"),
    ("m11", "minor-11th"),
    ("m9", "minor-ninth"),
    ("m7", "minor-seventh"),
    ("m6", "minor-sixth"),
    ("m", "minor"),
    ("sus2", "suspended-second"),
    ("sus4", "suspended-fourth"),
    ("13", "dominant-13th"),
    ("11", "dominant-11th"),
    ("9", "dominant-ninth"),
    ("7", "dominant"),
    ("6", "major-sixth"),
    ("5", "power"),
    ("", "major"),
];

/// The kind of a chord, the degrees it implies beyond its kind, and what
/// follows its quality in the id. A dominant chord with a suspension is a
/// suspended chord with the seventh (and ninth, …) added.
fn kind(id: &str) -> (&'static str, &'static [u8], &str) {
    let (quality, kind) = QUALITIES
        .iter()
        .find(|(quality, _)| id.starts_with(quality))
        .copied()
        .unwrap_or(("", "major"));
    let rest = &id[quality.len()..];
    let suspension = [("sus4", "suspended-fourth"), ("sus2", "suspended-second")]
        .into_iter()
        .find(|(sus, _)| rest.starts_with(sus));
    let added: &'static [u8] = match kind {
        "dominant" => &[7],
        "dominant-ninth" => &[7, 9],
        "dominant-11th" => &[7, 9, 11],
        "dominant-13th" => &[7, 9, 13],
        _ => &[],
    };
    match suspension {
        Some((sus, suspended)) if !added.is_empty() => (suspended, added, &rest[sus.len()..]),
        _ => (kind, &[], rest),
    }
}

/// The highest chord tone a kind includes.
fn extension(kind: &str) -> u8 {
    match kind {
        k if k.ends_with("13th") => 13,
        k if k.ends_with("11th") => 11,
        k if k.ends_with("ninth") => 9,
        "dominant" | "major-seventh" | "minor-seventh" | "diminished-seventh" | "augmented-seventh"
        | "half-diminished" | "major-minor" => 7,
        _ => 5,
    }
}

/// Alterations (`b9`, `#11`) and additions (`add9`, `/9`). An alteration of a
/// tone the chord has alters it; of any other, adds it altered.
fn degrees(text: &str, extension: u8) -> Vec<Degree> {
    let Some(first) = text.chars().next() else {
        return Vec::new();
    };
    let (alter, sign) = match first {
        '#' => (1, 1),
        'b' => (-1, 1),
        '/' => (0, 1),
        _ if text.starts_with("add") => (0, 3),
        _ => return degrees(&text[first.len_utf8()..], extension),
    };
    let digits = text[sign..].chars().take_while(char::is_ascii_digit).count();
    let degree = text[sign..sign + digits].parse::<u8>().ok().map(|value| Degree {
        value,
        alter,
        kind: if alter != 0 && (value == 5 || value <= extension) {
            "alter"
        } else {
            "add"
        },
    });
    degree
        .into_iter()
        .chain(degrees(&text[sign + digits..], extension))
        .collect()
}

/// The suffix shown after the root, as the id spells it.
fn display(id: &str) -> String {
    match id {
        "maj" => String::new(),
        "min" => "m".into(),
        "halfdim" => "m7b5".into(),
        "nc" => "N.C.".into(),
        other => other.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parts(id: &str) -> (&'static str, Vec<(u8, i8, &'static str)>) {
        let h = harmony(0, (0, 0), None, id);
        (h.kind, h.degrees.iter().map(|d| (d.value, d.alter, d.kind)).collect())
    }

    #[test]
    fn ids_become_kinds_and_degrees() {
        assert_eq!(parts("m9"), ("minor-ninth", vec![]));
        assert_eq!(parts("13#11"), ("dominant-13th", vec![(11, 1, "alter")]));
        assert_eq!(parts("maj9#11"), ("major-ninth", vec![(11, 1, "add")]));
        assert_eq!(
            parts("13b9b5"),
            ("dominant-13th", vec![(9, -1, "alter"), (5, -1, "alter")])
        );
        assert_eq!(parts("9sus4"), ("suspended-fourth", vec![(7, 0, "add"), (9, 0, "add")]));
        assert_eq!(parts("6/9#11"), ("major-sixth", vec![(9, 0, "add"), (11, 1, "add")]));
        assert_eq!(parts("add2"), ("major", vec![(2, 0, "add")]));
        assert_eq!(parts("7alt"), ("dominant", vec![]));
        assert_eq!(parts("nc").0, "none");
    }

    #[test]
    fn roots_are_spelled_on_their_step() {
        let h = harmony(0, (10, 6), Some((1, 1)), "m9");
        assert_eq!(h.root, Some(Spelled { step: 6, alter: -1 }));
        assert_eq!(h.bass, Some(Spelled { step: 1, alter: -1 }));
    }
}
