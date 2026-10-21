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
  key signatures, keys of single staves; clefs and clef changes; barlines,
  repeats and repeat endings; system and page breaks.
- Notes, chords and rests in up to four voices per staff; durations, dots,
  grace notes, cue size, hidden notes; tuplets, nested ones included; beams as
  Sibelius sets them; ties; slurs; cross-staff notes; flipped stems.
- Accidentals as Sibelius shows them: where key and bar require them, and
  cautionary or bracketed ones.
- Transposing instruments: written pitch with `<transpose>`, also for
  instrument changes within a bar, transposed key signatures, and none for
  instruments written without them.
- Percussion staves as unpitched notes; tablature as standard notation in the
  treble or bass clef.
- Articulations, fermatas, breath marks and caesuras, tremolos, trills,
  mordents and turns, accidentals above notes, arpeggios and glissandi.
- Chord symbols, with their chord diagrams.
- Dynamics and other text in the fonts of its text styles, with bold,
  italic, underline and size; tempo text and metronome marks (with a playback
  tempo), rehearsal marks, *rit.* and *accel.* lines, hairpins, octave lines,
  pedal lines, and other lines as brackets with their captions; lyrics with
  syllables, hyphens and extenders; figured bass.
- Page size, staff size and margins; title, subtitle, composer, lyricist,
  arranger and copyright, also as credits on the first page.

## Not converted

- Positions of objects other than their placement above or below the staff.
  The offsets Sibelius stores seem to be relative to each text style's default
  position rather than to the staff, so they are left out.
- Boxes and vertical lines, which MusicXML cannot express, and symbols beyond
  the common ones listed above.
- Hidden accidentals: MusicXML has no way to hide one, so readers may show it.
- Key changes within a bar (eleven in the test files) take effect from the
  next barline.

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

The meaning of most fields was inferred from the test files and is written up
in libsib's [`FORMAT.md`](https://github.com/pfefferniels/libsib/blob/main/FORMAT.md)
(*Meaning of the music fields*). Sibelius stores only whether a stem is
flipped; the default direction it flips is reconstructed from the usual rules,
which may differ from Sibelius in unusual cases. Some files place notes as if
inside a tuplet that is missing; these get the tuplet ratio that fits the time
they have, without a bracket. Figured bass is read through the glyphs of
Sibelius' figured bass font, so figures set in another font may come out
wrong.

## Licence

Licensed under either of the [Apache License, Version 2.0](LICENSE-APACHE) or
the [MIT licence](LICENSE-MIT), at your option.
