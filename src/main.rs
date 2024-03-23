mod args;
mod transcribe;
mod vendor;

use args::App;
use clap::Parser;

use crate::transcribe::IteratorExt;

type TxResult = Result<transcribe::Timing, csv::Error>;

#[derive(Debug)]
enum Error {
    Csv(csv::Error),
    Io(std::io::Error),
}

impl From<csv::Error> for Error {
    fn from(e: csv::Error) -> Self {
        Self::Csv(e)
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Csv(e) => write!(f, "CSV error: {}", e),
            Self::Io(e) => write!(f, "I/O error: {}", e),
        }
    }
}

impl App {
    fn transform_timings(&self) -> Result<(), Error> {
        let file = self.source().unwrap();

        let mut rdr = csv::Reader::from_reader(vendor::BadCsvReader::new(file));
        let mut timings = rdr
            .deserialize()
            .map(|r: TxResult| r.expect("no malformed CSV records"))
            .join_continuations();

        if let Some(silence) = self.max_silence() {
            timings = timings.max_silence(silence)
        }

        if let Some(gap) = self.by_gap() {
            timings = timings.by_gap(gap);
        }

        if self.sentences() {
            timings = timings.sentences();
        }

        if let Some(min_word_count) = self.min_word_count() {
            timings = timings.min_word_count(min_word_count);
        }

        if let Some(window) = self.lasting() {
            timings = timings.lasting(window);
        }

        let mut s = self.sink().unwrap();
        match self.output() {
            args::OutputKind::Csv => timings.write_csv(s)?,
            args::OutputKind::Pretty => {
                for t in timings {
                    writeln!(s, "{}\n", t)?;
                }
            }
        }

        Ok(())
    }
}

fn main() {
    let app = App::parse();

    app.transform_timings().unwrap();
}
