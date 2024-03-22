mod transcribe;
mod vendor;

use std::time::Duration;

use crate::transcribe::TxbIter;

type TxResult = Result<transcribe::Timing, csv::Error>;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let file_path = &args[1];
    let file = std::fs::File::open(file_path).unwrap();

    let mut rdr = csv::Reader::from_reader(vendor::BadCsvReader::new(file));
    let timings = rdr
        .deserialize()
        .filter_map(|r: TxResult| r.ok())
        .join_continuations()
        .sentences()
        .duration_windows(Duration::from_secs(10));

    for t in timings {
        println!("{}", t);
    }
}
