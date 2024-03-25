#[deny(clippy::pedantic)]
mod app;
mod transcribe;
mod vendor;

use std::process;

use app::App;
use clap::Parser;

enum ProgramOutcome {
    Expected,
    Unexpected(String),
}

fn main() {
    let app = App::parse();

    let outcome = match app.command() {
        app::cmd::Command::Transform(t) => {
            let timings = t.read_data().expect("failed to read timings");
            match t.process_to_output(timings) {
                Ok(_) => ProgramOutcome::Expected,
                Err(app::cmd::Error::Io(e)) if e.kind() == std::io::ErrorKind::BrokenPipe => {
                    ProgramOutcome::Expected
                }
                Err(e) => ProgramOutcome::Unexpected(e.to_string()),
            }
        }
    };

    match outcome {
        ProgramOutcome::Expected => {}
        ProgramOutcome::Unexpected(msg) => {
            eprintln!("{}", msg);
            process::exit(1);
        }
    }
}
