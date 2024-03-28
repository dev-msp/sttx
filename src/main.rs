#[deny(clippy::pedantic)]
mod app;
mod transcribe;
mod vendor;

use std::{io, process};

use app::{
    cmd::{Command, Error as AppError},
    App,
};
use clap::Parser;

enum ProgramOutcome {
    Expected,
    Unexpected(String),
}

fn main() {
    let app = App::parse();

    let outcome = match app.command() {
        Command::Transform(t) => {
            let timings = t.read_data().expect("failed to read timings");
            match t.process_to_output(timings) {
                Ok(_) => ProgramOutcome::Expected,
                Err(AppError::Io(e)) if e.kind() == io::ErrorKind::BrokenPipe => {
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
