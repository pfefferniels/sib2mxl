//! The music of a score, in the terms MusicXML needs. Positions are offsets
//! into a measure; everything that links objects across time (ties, slurs,
//! tuplets, beams, lines) is already resolved into start and stop marks.

/// Sibelius' unit of time, 256 per quarter note.
pub type Ticks = u32;

pub const QUARTER: Ticks = 256;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Score {
    pub work: Work,
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
    /// The instrument from each measure where it changes, the first from measure 0.
    pub instruments: Vec<(usize, Instrument)>,
    pub measures: Vec<StaffMeasure>,
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
    pub beam: BeamMode,
    /// Outermost first.
    pub tuplets: Vec<TupletLevel>,
    /// One entry per beam level, from the eighth-note beam inwards.
    pub beams: Vec<Beam>,
    pub articulations: Articulations,
    pub tremolo: u8,
    /// MusicXML ornament names, such as `trill-mark` or `mordent`.
    pub ornaments: Vec<&'static str>,
    pub slurs: Vec<Slur>,
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
pub struct Lyric {
    pub verse: u8,
    pub syllabic: &'static str,
    pub text: String,
    pub extend: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Note {
    pub pitch: Pitch,
    /// Sounding pitch in quarter tones, which identifies the note a tie ends on.
    pub chrom2: u8,
    pub tie_start: bool,
    pub tie_stop: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
    Words(String),
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
    Segno,
    Coda,
}

/// Figured bass under one position.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Figures {
    pub at: Ticks,
    pub voice: u8,
    pub figures: Vec<Figure>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Figure {
    pub prefix: Option<&'static str>,
    pub number: Option<String>,
}
