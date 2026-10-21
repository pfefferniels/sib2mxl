//! Converts every test file (see `common`).

mod common;

use common::corpus;
use sib2mxl::Error;

#[test]
fn every_score_with_staves_converts() {
    corpus().iter().for_each(|path| {
        let doc = libsib::Document::open(path).unwrap();
        match sib2mxl::to_musicxml(&doc) {
            Ok(musicxml) => {
                let parts = musicxml.matches("<score-part ").count();
                assert!(parts > 0, "{} has no parts", path.display());
                assert_eq!(musicxml.matches("<part id=").count(), parts, "{}", path.display());
            }
            Err(Error::Missing("staves")) => {}
            Err(error) => panic!("{}: {error}", path.display()),
        }
    });
}

#[test]
fn compressed_files_name_their_score() {
    let mxl = sib2mxl::to_mxl("<score-partwise/>").unwrap();
    assert!(mxl.starts_with(b"PK"));
    let text = String::from_utf8_lossy(&mxl);
    assert!(text.contains("application/vnd.recordare.musicxml"));
    assert!(text.contains("META-INF/container.xml"));
}
