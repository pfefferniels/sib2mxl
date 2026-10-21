//! The music of a score, in the terms MusicXML needs. Positions are offsets
//! into a measure; everything that links objects across time (ties, slurs,
//! tuplets, beams, lines) is already resolved into start and stop marks.

/// Sibelius' unit of time, 256 per quarter note.
pub type Ticks = u32;

pub const QUARTER: Ticks = 256;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Score {
    pub work: Work,
    pub layout: Layout,
    /// Text on the first page: title, composer and the like.
    pub credits: Vec<Credit>,
    pub measures: Vec<Measure>,
    pub parts: Vec<Part>,
    pub groups: Vec<PartGroup>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Work {
    pub title: Option<String>,
    pub composer: Option<String>,
    pub lyricist: Option<String>,
    pub arranger: Option<String>,
    pub rights: Option<String>,
}

/// Page and staff size, in millimetres.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Layout {
    pub staff_height: f64,
    pub page_width: f64,
    pub page_height: f64,
    pub left: f64,
    pub right: f64,
    pub top: f64,
    pub bottom: f64,
}

impl Default for Layout {
    /// A4 with a 7 mm staff, as Sibelius sets up a new score.
    fn default() -> Self {
        Self {
            staff_height: 7.0,
            page_width: 210.0,
            page_height: 297.0,
            left: 15.0,
            right: 15.0,
            top: 15.0,
            bottom: 15.0,
        }
    }
}

impl Layout {
    /// MusicXML tenths: ten to a staff space, forty to the staff height.
    pub fn tenths(&self, millimetres: f64) -> f64 {
        millimetres / self.staff_height * 40.0
    }

    /// Points for a text height in 32nds of a staff space, rounded to hundredths.
    pub fn points(&self, height: f64) -> f64 {
        const POINTS_PER_MILLIMETRE: f64 = 72.0 / 25.4;
        let points = height / 32.0 * self.staff_height / 4.0 * POINTS_PER_MILLIMETRE;
        (points * 100.0).round() / 100.0
    }
}

/// Text on the first page.
#[derive(Clone, Debug, PartialEq)]
pub struct Credit {
    /// A MusicXML credit type: `title`, `subtitle`, `composer`, `lyricist`, `rights`, …
    pub kind: &'static str,
    pub text: String,
    pub font: Font,
}

/// How text is set. Unset fields follow the reader's defaults.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Font {
    pub family: Option<String>,
    /// In points.
    pub size: Option<f64>,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
}

/// A stretch of text in one font.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Run {
    pub text: String,
    pub font: Font,
}

/// What a bar holds for all staves.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Measure {
    pub length: Ticks,
    /// A first bar shorter than its time signature.
    pub pickup: bool,
    pub time: Option<Time>,
    pub key: Option<Key>,
    pub left: Barline,
    pub right: Barline,
    /// The measure starts a new system or a new page.
    pub new_system: bool,
    pub new_page: bool,
    /// Tempo, rehearsal marks and other text for the whole system, shown
    /// above the top staff.
    pub directions: Vec<Direction>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Barline {
    pub style: Option<&'static str>,
    pub repeat: Option<Repeat>,
    pub ending: Option<Ending>,
}

impl Barline {
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Repeat {
    Forward,
    Backward,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ending {
    pub number: String,
    /// `start`, `stop` or `discontinue` (a stop without a closing hook).
    pub kind: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Time {
    pub beats: String,
    pub beat_type: String,
    pub symbol: Option<&'static str>,
    pub length: Ticks,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Key {
    pub fifths: i8,
    pub major: bool,
}

impl Key {
    pub const C_MAJOR: Key = Key { fifths: 0, major: true };

    /// The key as written for an instrument: transposed, enharmonically within
    /// seven sharps or flats, or none for instruments written without key signatures.
    pub fn written_for(self, instrument: Instrument) -> Key {
        if !instrument.key_signature {
            return Key::C_MAJOR;
        }
        let fifths = match i32::from(self.fifths) + instrument.transposition.key_shift() {
            sharps if sharps > 7 => sharps - 12,
            flats if flats < -7 => flats + 12,
            fifths => fifths,
        };
        Key {
            fifths: fifths as i8,
            ..self
        }
    }

    /// The alteration, in quarter tones, the key gives a step (0 is C).
    pub fn alter2(self, step: u8) -> i8 {
        const SHARPS: [u8; 7] = [3, 0, 4, 1, 5, 2, 6];
        let count = usize::from(self.fifths.unsigned_abs()).min(7);
        match self.fifths.signum() {
            1 if SHARPS[..count].contains(&step) => 2,
            -1 if SHARPS[7 - count..].contains(&step) => -2,
            _ => 0,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Part {
    pub name: String,
    pub abbreviation: String,
    pub staves: Vec<Staff>,
}

/// A bracket or brace over consecutive parts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PartGroup {
    pub first: usize,
    pub last: usize,
    pub symbol: &'static str,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Staff {
    pub percussion: bool,
    /// The instrument from each change on, the first from the start.
    pub instruments: Vec<Switch>,
    pub measures: Vec<StaffMeasure>,
}

impl Staff {
    /// The instrument in force at a position.
    pub fn instrument_at(&self, measure: usize, at: Ticks) -> Instrument {
        self.instruments
            .iter()
            .rev()
            .find(|switch| (switch.measure, switch.at) <= (measure, at))
            .map_or_else(Instrument::default, |switch| switch.instrument)
    }
}

impl Staff {
    /// The key written on this staff in each measure: the score's key or the
    /// staff's own, as written for the instrument in force.
    pub fn written_keys(&self, measures: &[Measure]) -> Vec<Key> {
        measures
            .iter()
            .enumerate()
            .scan(Key::C_MAJOR, |concert, (index, measure)| {
                let own = self
                    .measures
                    .get(index)
                    .and_then(|m| m.keys.first())
                    .map(|&(_, key)| key);
                *concert = own.or(measure.key).unwrap_or(*concert);
                Some(concert.written_for(self.instrument_at(index, 0)))
            })
            .collect()
    }
}

/// A change of instrument on a staff.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Switch {
    pub measure: usize,
    pub at: Ticks,
    pub instrument: Instrument,
}

/// How an instrument's music is written.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Instrument {
    pub transposition: Interval,
    pub key_signature: bool,
}

impl Default for Instrument {
    fn default() -> Self {
        Self {
            transposition: Interval::default(),
            key_signature: true,
        }
    }
}

/// The interval from written to sounding pitch, in diatonic steps and quarter tones.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Interval {
    pub diatonic: i32,
    pub chrom2: i32,
}

impl Interval {
    /// How many fifths a key moves when written for this instrument.
    pub fn key_shift(self) -> i32 {
        12 * self.diatonic - 7 * (self.chrom2 / 2)
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct StaffMeasure {
    pub events: Vec<Event>,
    /// Clef changes; one at the measure's length sits before the barline.
    pub clefs: Vec<(Ticks, Clef)>,
    pub directions: Vec<Direction>,
    pub figures: Vec<Figures>,
    pub harmonies: Vec<Harmony>,
    /// Key signatures of this staff alone, which differ from the other staves'.
    pub keys: Vec<(Ticks, Key)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Clef {
    pub sign: &'static str,
    pub line: Option<u8>,
    pub octave_change: i8,
}

/// A chord, rest or bar rest in one voice.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Event {
    pub at: Ticks,
    /// 1 to 4, as in Sibelius.
    pub voice: u8,
    pub value: NoteValue,
    pub content: Content,
    pub grace: Option<Grace>,
    pub cue: bool,
    pub hidden: bool,
    /// Drawn on the staff above (-1) or below (1) its own.
    pub cross_staff: i8,
    /// The stem points the other way than it would by default.
    pub flip_stem: bool,
    /// `up` or `down`, where it differs from the default.
    pub stem: Option<&'static str>,
    pub beam: BeamMode,
    /// Outermost first.
    pub tuplets: Vec<TupletLevel>,
    /// One entry per beam level, from the eighth-note beam inwards.
    pub beams: Vec<Beam>,
    pub articulations: Articulations,
    /// Further MusicXML articulations, such as `breath-mark` or `caesura`.
    pub marks: Vec<&'static str>,
    /// Accidentals written above the note (editorial ones, for instance).
    pub accidental_marks: Vec<&'static str>,
    pub tremolo: u8,
    /// MusicXML ornament names, such as `trill-mark` or `mordent`.
    pub ornaments: Vec<&'static str>,
    pub slurs: Vec<Slur>,
    pub glissandi: Vec<Glissando>,
    /// The chord is spread, upwards, downwards or (None) without a direction.
    pub arpeggio: Option<Option<&'static str>>,
    pub lyrics: Vec<Lyric>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Content {
    Chord(Vec<Note>),
    #[default]
    Rest,
    /// A rest filling the whole measure.
    MeasureRest,
}

/// A written note value: a power-of-two length with dots.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NoteValue {
    /// log2 of the undotted length in ticks: 10 is a whole note, 8 a quarter.
    pub log: u8,
    pub dots: u8,
}

impl NoteValue {
    pub fn ticks(self) -> Ticks {
        let base: Ticks = 1 << self.log;
        2 * base - (base >> self.dots)
    }

    pub fn type_name(self) -> &'static str {
        const NAMES: [&str; 14] = [
            "1024th", "512th", "256th", "128th", "64th", "32nd", "16th", "eighth", "quarter", "half", "whole", "breve",
            "long", "maxima",
        ];
        NAMES[usize::from(self.log).min(NAMES.len() - 1)]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Grace {
    pub slash: bool,
}

/// How Sibelius beams a note to its neighbours.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BeamMode {
    #[default]
    None,
    Start,
    Continue,
    /// Continue with the primary beam only.
    Single,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Beam {
    Begin,
    Continue,
    End,
    ForwardHook,
    BackwardHook,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TupletLevel {
    pub actual: u8,
    pub normal: u8,
    pub start: bool,
    pub stop: bool,
}

/// Sibelius' articulation set, one bit each (the ManuScript `…Artic` constants).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Articulations(pub u16);

impl Articulations {
    pub const STACCATO: u16 = 1 << 1;
    pub const STACCATISSIMO: u16 = 1 << 2;
    pub const WEDGE: u16 = 1 << 3;
    pub const TENUTO: u16 = 1 << 4;
    pub const ACCENT: u16 = 1 << 5;
    pub const MARCATO: u16 = 1 << 6;
    pub const HARMONIC: u16 = 1 << 7;
    pub const PLUS: u16 = 1 << 8;
    pub const UP_BOW: u16 = 1 << 9;
    pub const DOWN_BOW: u16 = 1 << 10;
    pub const SQUARE_PAUSE: u16 = 1 << 12;
    pub const PAUSE: u16 = 1 << 13;
    pub const TRI_PAUSE: u16 = 1 << 14;

    pub fn has(self, bit: u16) -> bool {
        self.0 & bit != 0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Slur {
    pub number: u8,
    pub start: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Glissando {
    pub number: u8,
    pub start: bool,
    /// `solid` or `wavy`.
    pub line_type: &'static str,
    pub text: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Lyric {
    pub verse: u8,
    pub syllabic: &'static str,
    pub text: String,
    pub extend: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Note {
    pub pitch: Pitch,
    /// Sounding pitch in quarter tones, which identifies the note a tie ends on.
    pub chrom2: u8,
    pub tie_start: bool,
    pub tie_stop: bool,
    pub style: AccidentalStyle,
    /// The accidental shown, if any.
    pub accidental: Option<Accidental>,
}

/// How a note's accidental is shown, as set in Sibelius.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AccidentalStyle {
    /// Where the key and the bar require it.
    #[default]
    Normal,
    Hidden,
    Cautionary,
    Bracketed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Accidental {
    /// A MusicXML accidental, such as `sharp` or `flat-flat`.
    pub name: &'static str,
    pub cautionary: bool,
    pub parentheses: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Pitch {
    /// 0 is C, 6 is B.
    pub step: u8,
    /// In quarter tones.
    pub alter2: i8,
    pub octave: i8,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Direction {
    pub at: Ticks,
    pub voice: u8,
    pub above: bool,
    pub kinds: Vec<DirectionKind>,
    /// Quarter notes per minute.
    pub tempo: Option<f64>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum DirectionKind {
    Words(Vec<Run>),
    Dynamics(&'static str),
    Rehearsal(String),
    Metronome {
        unit: NoteValue,
        per_minute: String,
    },
    /// `crescendo`, `diminuendo` or `stop`.
    Wedge(&'static str),
    /// `up`, `down` or `stop`, as MusicXML counts the shift of the display.
    OctaveShift {
        kind: &'static str,
        size: u8,
    },
    Pedal {
        kind: &'static str,
        line: bool,
    },
    /// A general line over a stretch of music.
    Bracket {
        /// `start` or `stop`.
        kind: &'static str,
        number: u8,
        line_end: &'static str,
        line_type: &'static str,
    },
    Segno,
    Coda,
}

/// A chord symbol.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Harmony {
    pub at: Ticks,
    /// None for "no chord".
    pub root: Option<Spelled>,
    /// A MusicXML kind, such as `minor-seventh`.
    pub kind: &'static str,
    /// The suffix to show after the root.
    pub text: String,
    pub bass: Option<Spelled>,
    pub degrees: Vec<Degree>,
    /// The chord diagram shown with the symbol.
    pub frame: Option<Frame>,
}

/// A chord diagram of a fretted instrument.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Frame {
    pub strings: u8,
    pub frets: u8,
    pub first_fret: u32,
    pub notes: Vec<FrameNote>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameNote {
    /// Numbered from the highest string, 1.
    pub string: u8,
    pub fret: u8,
    /// `start` or `stop` of a barre.
    pub barre: Option<&'static str>,
}

/// A note name: a diatonic step (0 is C) and its alteration in semitones.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Spelled {
    pub step: u8,
    pub alter: i8,
}

/// A chord tone added to, altered in or taken from a chord's kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Degree {
    pub value: u8,
    pub alter: i8,
    /// `add`, `alter` or `subtract`.
    pub kind: &'static str,
}

/// Figured bass under one position.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Figures {
    pub at: Ticks,
    pub voice: u8,
    pub figures: Vec<Figure>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Figure {
    pub prefix: Option<&'static str>,
    pub number: Option<String>,
    pub suffix: Option<&'static str>,
    /// A line holding the figure before.
    pub extend: bool,
}
