//! Converts Sibelius scores, as decoded by [`libsib`], to MusicXML 4.0.

mod error;
mod fields;
mod model;
mod mxl;
mod read;
mod text;
mod write;
mod xml;

pub use error::{Error, Result};

/// The score of `doc` as an uncompressed (partwise) MusicXML document.
pub fn to_musicxml(doc: &libsib::Document) -> Result<String> {
    let parsed = doc.score()?;
    let score = read::score(&parsed.root)?;
    Ok(write::musicxml(&score))
}

/// A MusicXML document packed as a compressed MusicXML (`.mxl`) archive.
pub fn to_mxl(musicxml: &str) -> Result<Vec<u8>> {
    mxl::pack(musicxml)
}
