use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const USAGE: &str = "usage: sib2mxl <score.sib> [<output.mxl> | <output.musicxml> | -]

Converts a Sibelius score to MusicXML. Without an output, writes <score.mxl>
next to the input. An output ending in .musicxml or .xml is written
uncompressed, and - writes uncompressed MusicXML to standard output.";

enum Output {
    Compressed(PathBuf),
    Uncompressed(PathBuf),
    Stdout,
}

impl Output {
    fn for_argument(argument: &str) -> Self {
        let path = PathBuf::from(argument);
        match path.extension().and_then(|e| e.to_str()) {
            _ if argument == "-" => Output::Stdout,
            Some("musicxml" | "xml") => Output::Uncompressed(path),
            _ => Output::Compressed(path),
        }
    }
}

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let (input, output) = match arguments.as_slice() {
        [input] if input != "-h" && input != "--help" => (
            PathBuf::from(input),
            Output::Compressed(Path::new(input).with_extension("mxl")),
        ),
        [input, output] => (PathBuf::from(input), Output::for_argument(output)),
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    match convert(&input, &output) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("sib2mxl: {}: {error}", input.display());
            ExitCode::FAILURE
        }
    }
}

fn convert(input: &Path, output: &Output) -> sib2mxl::Result<()> {
    let musicxml = sib2mxl::to_musicxml(&libsib::Document::open(input)?)?;
    match output {
        Output::Compressed(path) => std::fs::write(path, sib2mxl::to_mxl(&musicxml)?)?,
        Output::Uncompressed(path) => std::fs::write(path, musicxml)?,
        Output::Stdout => std::io::stdout().lock().write_all(musicxml.as_bytes())?,
    }
    Ok(())
}
