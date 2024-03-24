mod input;
mod output;

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

pub mod cmd {
    use std::time::Duration;

    use clap::{Args, Subcommand};

    use super::{
        input::{Input, ParseDuration},
        output::{Output, OutputFormat},
    };
    use crate::transcribe::IterDyn;

    #[derive(Debug)]
    pub enum Error {
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

    #[derive(Subcommand)]
    pub enum Command {
        Transform(Transform),
    }

    #[derive(Args)]
    pub struct Transform {
        #[command(flatten)]
        input: Input,

        #[command(flatten)]
        output: Output,

        #[command(flatten)]
        pipeline: TranscriptionPipeline,
    }

    impl Transform {
        pub fn read_data(&self) -> Result<IterDyn<'_>, std::io::Error> {
            use crate::transcribe::IteratorExt;

            let source = self.input.source()?;
            let raw_iter: IterDyn = self.input.format().consume_reader(source);
            let timings = raw_iter.join_continuations();

            Ok(self.pipeline.process_iter(timings))
        }

        pub fn process_to_output(&self, timings: IterDyn<'_>) -> Result<(), Error> {
            let mut s = self.output.sink()?;
            match self.output.format() {
                OutputFormat::Csv => timings.write_csv(s)?,
                OutputFormat::Json => timings.write_json(s)?,
                OutputFormat::Pretty => {
                    for t in timings {
                        writeln!(s, "{}\n", t)?;
                    }
                }
            };
            Ok(())
        }
    }

    #[derive(Args)]
    pub struct TranscriptionPipeline {
        /// Concatenates until the accumulated delay between events exceeds the given duration.
        #[arg(long, value_parser = ParseDuration)]
        max_silence: Option<Duration>,

        /// Concatenates up to the next sentence ending ('.', '!', or '?')
        #[arg(short, long, default_value = "false")]
        sentences: bool,

        /// Concatenates until the total word count of the result exceeds the given value.
        #[arg(short = 'w', long)]
        min_word_count: Option<usize>,

        /// Concatenates until the delay until the start of the next event exceeds the given duration.
        #[arg(short = 'g', long, value_parser = ParseDuration)]
        by_gap: Option<Duration>,

        /// Concatenates until the total duration of the result exceeds the given value.
        #[arg(short, long, value_parser = ParseDuration)]
        lasting: Option<Duration>,

        /// Concatenates up to N events.
        #[arg(short, long)]
        chunk_size: Option<usize>,
    }

    #[allow(dead_code)]
    impl TranscriptionPipeline {
        pub fn process_iter<'a>(&self, mut it: IterDyn<'a>) -> IterDyn<'a> {
            if let Some(silence) = self.max_silence() {
                it = it.max_silence(silence)
            }

            if let Some(gap) = self.by_gap() {
                it = it.by_gap(gap);
            }

            if self.sentences() {
                it = it.sentences();
            }

            if let Some(min_word_count) = self.min_word_count() {
                it = it.min_word_count(min_word_count);
            }

            if let Some(window) = self.lasting() {
                it = it.lasting(window);
            }

            if let Some(chunk_count) = self.chunk_size() {
                it = it.chunks(chunk_count);
            }

            it
        }

        pub fn max_silence(&self) -> Option<Duration> {
            self.max_silence
        }

        pub fn min_word_count(&self) -> Option<usize> {
            self.min_word_count
        }

        pub fn by_gap(&self) -> Option<Duration> {
            self.by_gap
        }

        pub fn lasting(&self) -> Option<Duration> {
            self.lasting
        }

        pub fn chunk_size(&self) -> Option<usize> {
            self.chunk_size
        }

        pub fn sentences(&self) -> bool {
            self.sentences
        }
    }
}
