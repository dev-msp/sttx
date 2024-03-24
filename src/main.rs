mod args;
mod transcribe;
mod vendor;

use std::{io::ErrorKind, process};

use args::App;
use clap::Parser;
use itertools::Itertools;
use transcribe::IterDyn;

use crate::transcribe::IteratorExt;

type TxResult = Result<transcribe::Timing, csv::Error>;

#[derive(Debug)]
enum Error {
    Csv(csv::Error),
    Json(serde_json::Error),
    Io(std::io::Error),
}

impl From<csv::Error> for Error {
    fn from(e: csv::Error) -> Self {
        Self::Csv(e)
    }
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Self::Json(e)
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
            Self::Json(e) => write!(f, "JSON error: {}", e),
            Self::Io(e) => write!(f, "I/O error: {}", e),
        }
    }
}

impl App {
    fn transform_timings(&self) -> IterDyn<'_> {
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

        if let Some(chunk_count) = self.chunk_size() {
            timings = timings.chunks(chunk_count);
        }

        timings.collect_vec().into_iter().boxed()
    }

    fn process_to_output(&self, timings: IterDyn<'_>) -> Result<(), Error> {
        let mut s = self.sink().unwrap();
        match self.output() {
            args::OutputFormat::Csv => timings.write_csv(s)?,
            args::OutputFormat::Json => timings.write_json(s)?,
            args::OutputFormat::Pretty => {
                for t in timings {
                    writeln!(s, "{}\n", t)?;
                }
            }
        };
        Ok(())
    }
}

fn main() {
    let app = App::parse();

    let timings = app.transform_timings();

    match app.process_to_output(timings) {
        Err(Error::Io(e)) if e.kind() == ErrorKind::BrokenPipe => {
            process::exit(0);
        }
        _ => {}
    }
}
