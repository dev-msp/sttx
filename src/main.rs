mod args;
mod transcribe;
mod vendor;

use std::process;

use args::App;
use clap::Parser;

type TxResult = Result<transcribe::Timing, csv::Error>;

impl App {}

fn main() {
    let app = App::parse();

    match app.command() {
        args::cmd::Command::Transform(t) => {
            let timings = t.read_data().expect("failed to read timings");
            match t.process_to_output(timings) {
                Err(args::cmd::Error::Io(e)) if e.kind() == std::io::ErrorKind::BrokenPipe => {
                    process::exit(0);
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                    process::exit(1);
                }
                _ => {}
            }
        }
    }
}
