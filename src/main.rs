mod transcribe;
mod vendor;

use std::{
    io::{BufReader, BufWriter},
    time::Duration,
};

use crate::transcribe::TxbIter;

type TxResult = Result<transcribe::Timing, csv::Error>;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let file_path = &args[1];
    let file = std::fs::File::open(file_path).unwrap();
    let r = BufReader::new(file);
    let mut bytes = Vec::new();
    vendor::handle_quotes(r, BufWriter::new(&mut bytes)).unwrap();

    let mut rdr = csv::Reader::from_reader(bytes.as_slice());
    let timings = rdr
        .deserialize()
        .filter_map(|r: TxResult| r.ok())
        .join_continuations()
        .duration_windows(Duration::from_secs(10));

    for t in timings {
        println!("\nstart: {}, content:\n{}", t.start(), t.content());
    }
}
