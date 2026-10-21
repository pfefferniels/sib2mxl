# sib2mxl

Converts Sibelius `.sib` scores to MusicXML 4.0. It reads files with
[libsib](https://github.com/pfefferniels/libsib) and writes compressed
(`.mxl`) or uncompressed (`.musicxml`) MusicXML.

```
sib2mxl score.sib                    writes score.mxl
sib2mxl score.sib out.musicxml       uncompressed
sib2mxl score.sib -                  uncompressed, to standard output
```

As a library:

```rust
let doc = libsib::Document::open("score.sib")?;
let musicxml = sib2mxl::to_musicxml(&doc)?;
std::fs::write("score.mxl", sib2mxl::to_mxl(&musicxml)?)?;
```

## What is converted

- Parts and their staves (grand staves stay one part), names and
  abbreviations, brackets and braces between parts.
- Measures with their lengths, including pickups and irregular bars; time and
  key signatures; clefs and clef changes; barlines, repeats and repeat endings.
- Notes, chords and rests in up to four voices per staff; durations, dots,
  grace notes, cue size, hidden notes; tuplets, nested ones included; beams as
  Sibelius sets them; ties; slurs.
- Transposing instruments: written pitch with `<transpose>`, transposed key
  signatures, and none for instruments written without them.
- Percussion staves as unpitched notes; tablature as guitar notation in the
  treble clef an octave down.
- Articulations, fermatas, tremolos, trills, mordents and turns, from note
  articulations, trill lines and symbols.
- Dynamics, expression and technique text, tempo text and metronome marks
  (with a playback tempo), rehearsal marks, *rit.* and *accel.* lines,
  hairpins, octave lines, pedal lines; lyrics with syllables, hyphens and
  extenders; figured bass.
- Title, composer, lyricist, arranger and copyright.

## Not converted

- Chord symbols and guitar frames, glissandi, arpeggios, boxes and other
  lines, and most symbols beyond those listed above.
- Layout: positions, stem directions, system and page breaks, fonts and
  text formatting. Accidentals are not written out (the file declares this),
  so readers derive them from the pitches; cautionary accidentals are lost.
- Cross-staff notes. The field that seems to mark them (`CrossStaveType`)
  does not behave consistently in the test files, so notes stay on their own
  staff.
- Instrument changes within a measure take effect from its start; key changes
  within a measure from the next one.
- Figured bass characters other than digits, `#`, `b` and `n` are written as
  the characters of Sibelius' figured bass font.

## Testing

`cargo test` converts every `.sib` file found in the directories listed in
`SIB_CORPUS` (separated like `PATH`); without the variable the corpus test
passes vacuously. No test files are distributed.

On the 363 test files used during development, every file with at least one
stave converted (one file has no staves and is rejected), and every result
validates against the MusicXML 4.0 schema. For one score a MusicXML export by
Sibelius was available: 212 of its 216 notes agree, and the four that differ
lie in bars that seem to have been edited after the export. Renderings of a
handful of the Sibelius example scores in MuseScore 4 look as expected, as far
as I can judge without the originals at hand. MuseScore 4.7.4 crashes on one
of them, at an 11:7 tuplet with a 4:3 tuplet nested inside; the file itself
validates.

## Notes

Durations, beams and articulations are stored in compact fields whose meaning
was inferred from the test files: the duration byte holds the log2 of the
length in ticks (256 to a quarter) with dots and a grace-note flag, beam modes
sit in the low bits of the tremolo byte, and articulations form a bit set in
the order of ManuScript's articulation constants. Bar lengths come from the
spacing table, which records where each bar ends. Some files place notes as if
inside a tuplet that is missing; these get the tuplet ratio that fits the
time they have, without a bracket.

## Licence

Licensed under either of the [Apache License, Version 2.0](LICENSE-APACHE) or
the [MIT licence](LICENSE-MIT), at your option.
