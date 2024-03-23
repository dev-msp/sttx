mod args;
mod transcribe;
mod vendor;

use args::App;
use clap::Parser;

use crate::transcribe::IteratorExt;

type TxResult = Result<transcribe::Timing, csv::Error>;

impl App {
    fn read_file(&self) -> std::fs::File {
        std::fs::File::open(self.path()).unwrap()
    }
}

fn main() {
    let app = App::parse();

    let file = app.read_file();

    let mut rdr = csv::Reader::from_reader(vendor::BadCsvReader::new(file));
    let mut timings = rdr
        .deserialize()
        .map(|r: TxResult| r.expect("no malformed CSV records"))
        .join_continuations();

    if let Some(silence) = app.max_silence() {
        timings = timings.max_silence(silence)
    }

    if let Some(gap) = app.by_gap() {
        timings = timings.by_gap(gap);
    }

    if app.sentences() {
        timings = timings.sentences();
    }

    if let Some(min_word_count) = app.min_word_count() {
        timings = timings.min_word_count(min_word_count);
    }

    if let Some(window) = app.lasting() {
        timings = timings.lasting(window);
    }

    let mut s = app.sink().unwrap();
    match app.output() {
        args::OutputKind::Csv => timings.write_csv(s).unwrap(),
        args::OutputKind::Pretty => {
            for t in timings {
                writeln!(s, "{}\n", t).unwrap();
            }
        }
    }
}
