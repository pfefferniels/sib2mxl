//! Compressed MusicXML: a zip archive with the score and a container file
//! that names it as the root file.

use std::io::{Cursor, Write};

use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

use crate::error::Result;

const SCORE: &str = "score.musicxml";

fn container() -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<container>
  <rootfiles>
    <rootfile full-path="{SCORE}" media-type="application/vnd.recordare.musicxml+xml"/>
  </rootfiles>
</container>
"#
    )
}

pub fn pack(musicxml: &str) -> Result<Vec<u8>> {
    let stored = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    let deflated = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    zip.start_file("mimetype", stored)?;
    zip.write_all(b"application/vnd.recordare.musicxml")?;
    zip.start_file("META-INF/container.xml", deflated)?;
    zip.write_all(container().as_bytes())?;
    zip.start_file(SCORE, deflated)?;
    zip.write_all(musicxml.as_bytes())?;
    Ok(zip.finish()?.into_inner())
}
