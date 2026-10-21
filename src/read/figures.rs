//! Figured bass as Sibelius' figured bass font (Opus Figured Bass) encodes
//! it: each character stands for a figure, an accidental, or a figure with an
//! accidental, a stroke or a plus sign. The table follows the glyphs of that
//! font.

use crate::model::Figure;

/// One line of figured bass, a figure.
pub fn figure(line: &str) -> Figure {
    line.chars().fold(Figure::default(), |mut figure, character| {
        match glyph(character) {
            Glyph::Accidental(accidental) if figure.number.is_none() => figure.prefix = Some(accidental),
            Glyph::Accidental(accidental) => figure.suffix = Some(accidental),
            Glyph::Figure { prefix, number, suffix } => {
                figure.prefix = prefix.or(figure.prefix);
                figure.number.get_or_insert_with(String::new).push_str(number);
                figure.suffix = suffix.or(figure.suffix);
            }
            Glyph::Extension => figure.extend = true,
            Glyph::Text(text) => figure.number.get_or_insert_with(String::new).push(text),
            Glyph::Nothing => {}
        }
        figure
    })
}

enum Glyph {
    Accidental(&'static str),
    Figure {
        prefix: Option<&'static str>,
        number: &'static str,
        suffix: Option<&'static str>,
    },
    /// A line that holds the figure before.
    Extension,
    Text(char),
    Nothing,
}

const SHARP: Option<&str> = Some("sharp");
const FLAT: Option<&str> = Some("flat");
const NATURAL: Option<&str> = Some("natural");
const PLUS: Option<&str> = Some("plus");
const SLASH: Option<&str> = Some("slash");
const BACK_SLASH: Option<&str> = Some("back-slash");

/// Figures with an accidental, stroke or plus: (character, prefix, number, suffix).
const FIGURES: [(char, Option<&str>, &str, Option<&str>); 70] = [
    ('b', None, "5", FLAT),
    ('n', None, "6", FLAT),
    ('m', None, "7", FLAT),
    ('h', None, "6", SHARP),
    ('f', None, "4", SHARP),
    ('j', None, "7", SHARP),
    ('g', None, "5", SHARP),
    ('y', None, "6", NATURAL),
    ('t', None, "5", NATURAL),
    ('u', None, "7", NATURAL),
    ('w', None, "2", NATURAL),
    ('c', None, "3", FLAT),
    ('d', None, "3", SHARP),
    ('e', None, "3", NATURAL),
    ('l', None, "9", SHARP),
    ('o', None, "9", NATURAL),
    ('r', None, "4", NATURAL),
    ('s', None, "2", SHARP),
    ('v', None, "4", FLAT),
    ('x', None, "2", FLAT),
    ('ë', None, "9", FLAT),
    ('B', FLAT, "5", None),
    ('C', FLAT, "3", None),
    ('D', SHARP, "3", None),
    ('E', NATURAL, "3", None),
    ('F', SHARP, "4", None),
    ('G', SHARP, "5", None),
    ('H', SHARP, "6", None),
    ('J', SHARP, "7", None),
    ('L', SHARP, "9", None),
    ('M', FLAT, "7", None),
    ('N', FLAT, "6", None),
    ('O', NATURAL, "9", None),
    ('R', NATURAL, "4", None),
    ('S', SHARP, "2", None),
    ('T', NATURAL, "5", None),
    ('U', NATURAL, "7", None),
    ('V', FLAT, "4", None),
    ('W', NATURAL, "2", None),
    ('X', FLAT, "2", None),
    ('Y', NATURAL, "6", None),
    ('´', None, "6", BACK_SLASH),
    ('&', None, "7", SLASH),
    ('Ï', None, "5", SLASH),
    ('ï', None, "5", SLASH),
    ('ö', None, "9", SLASH),
    ('ä', None, "7", PLUS),
    ('Ë', None, "5", PLUS),
    ('Ä', None, "4", PLUS),
    ('©', None, "2", PLUS),
    ('<', None, "7", PLUS),
    ('>', PLUS, "7", None),
    ('K', None, "4", PLUS),
    ('Z', None, "5", PLUS),
    ('k', None, "2", PLUS),
    ('z', None, "3", PLUS),
    ('a', None, "(2)", None),
    ('p', None, "(3)", None),
    ('A', None, "(4)", None),
    ('P', None, "(5)", None),
    ('$', None, "(6)", None),
    ('Â', None, "(7)", None),
    ('=', None, "(8)", None),
    ('À', None, "(9)", None),
    ('i', None, "[2]", None),
    ('q', None, "[3]", None),
    ('I', None, "[4]", None),
    ('Q', None, "[5]", None),
    ('"', None, "[6]", None),
    ('Ö', None, "[8]", None),
];

fn glyph(character: char) -> Glyph {
    let accidental = match character {
        '#' => Some("sharp"),
        '!' => Some("flat"),
        '§' => Some("natural"),
        '%' => Some("flat-flat"),
        '?' => Some("double-sharp"),
        '+' => Some("plus"),
        _ => None,
    };
    if let Some(accidental) = accidental {
        return Glyph::Accidental(accidental);
    }
    if let Some(&(_, prefix, number, suffix)) = FIGURES.iter().find(|(c, ..)| *c == character) {
        return Glyph::Figure { prefix, number, suffix };
    }
    match character {
        '-' | 'ü' => Glyph::Extension,
        '0'..='9' | '(' | ')' | '[' | ']' => Glyph::Text(character),
        _ => Glyph::Nothing,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parts(line: &str) -> (Option<&'static str>, Option<String>, Option<&'static str>, bool) {
        let f = figure(line);
        (f.prefix, f.number, f.suffix, f.extend)
    }

    #[test]
    fn glyphs_become_figures() {
        assert_eq!(parts("6"), (None, Some("6".into()), None, false));
        assert_eq!(parts("#"), (Some("sharp"), None, None, false));
        assert_eq!(parts("§"), (Some("natural"), None, None, false));
        assert_eq!(parts("h"), (None, Some("6".into()), Some("sharp"), false));
        assert_eq!(parts("´"), (None, Some("6".into()), Some("back-slash"), false));
        assert_eq!(parts("À"), (None, Some("(9)".into()), None, false));
        assert_eq!(parts("-"), (None, None, None, true));
    }
}
