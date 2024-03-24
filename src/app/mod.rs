pub mod cmd;
pub(crate) mod input;
pub(crate) mod output;

use clap::Parser;

#[derive(Parser)]
#[command(version, about, long_about = None)]
pub struct App {
    #[command(subcommand)]
    command: cmd::Command,
}

impl App {
    pub fn command(&self) -> &cmd::Command {
        &self.command
    }
}
