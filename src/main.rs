#[deny(clippy::pedantic)]
mod app;
mod transcribe;
mod vendor;

use std::process;

use app::App;
use clap::Parser;

fn main() {
    let app = App::parse();

    match app.command() {
        app::cmd::Command::Transform(t) => {
            let timings = t.read_data().expect("failed to read timings");
            match t.process_to_output(timings) {
                Err(app::cmd::Error::Io(e)) if e.kind() == std::io::ErrorKind::BrokenPipe => {
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
