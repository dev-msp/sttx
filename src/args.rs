use std::time::Duration;

use clap::{
    builder::PossibleValue,
    error::{ContextKind, ContextValue, ErrorKind},
    Args, Error, Parser, ValueEnum,
};
use itertools::Itertools;

use crate::{
    transcribe::{IterDyn, IteratorExt, Timing},
    vendor::BadCsvReader,
    TxResult,
};

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

#[derive(Args)]
pub struct Input {
    #[arg(
        short = 'i',
        long = "input-format",
        name = "input-format",
        default_value = "csv-fix",
        value_enum
    )]
    format: InputFormat,

    #[arg(value_parser = InputSource::parse)]
    source: InputSource,
}

impl Input {
    pub fn source(&self) -> Result<Box<dyn std::io::Read>, std::io::Error> {
        let reader: Box<dyn std::io::Read> = match self.source {
            InputSource::Stdin => Box::new(std::io::stdin()),
            InputSource::File(ref path) => Box::new(std::fs::File::open(path)?),
        };
        Ok(reader)
    }
}

#[derive(Args)]
pub struct Output {
    #[arg(short = 'f', long = "format", default_value = "pretty", value_enum)]
    format: OutputFormat,

    /// The path to which the program should write the output. Use `-` for stdout.
    #[arg(short = 'o',  long = "output", default_value = "-", value_parser = OutputSink::parse)]
    sink: OutputSink,
}

impl Output {
    pub fn sink(&self) -> Result<Box<dyn std::io::Write>, std::io::Error> {
        Ok(match self.sink {
            OutputSink::Stdout => Box::new(std::io::stdout()),
            OutputSink::File(ref path) => Box::new(std::fs::File::create(path)?),
        })
    }
}

pub mod cmd {
    use std::time::Duration;

    use clap::{Args, Subcommand};

    use super::{OutputFormat, ParseDuration};
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
        input: super::Input,

        #[command(flatten)]
        output: super::Output,

        #[command(flatten)]
        pipeline: TranscriptionPipeline,
    }

    impl Transform {
        pub fn read_data(&self) -> Result<IterDyn<'_>, std::io::Error> {
            use crate::transcribe::IteratorExt;

            let source = self.input.source()?;
            let raw_iter: IterDyn = self.input.format.consume_reader(source);
            let timings = raw_iter.join_continuations();

            Ok(self.pipeline.process_iter(timings))
        }

        pub fn process_to_output(&self, timings: IterDyn<'_>) -> Result<(), Error> {
            let mut s = self.output.sink()?;
            match self.output.format {
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

#[derive(Debug, Clone)]
pub enum CsvHandling {
    WhisperCppFix,
}

#[derive(Debug, Clone)]
pub enum InputFormat {
    Csv(Option<CsvHandling>),
    Json,
}

impl Default for InputFormat {
    fn default() -> Self {
        Self::Csv(Some(CsvHandling::WhisperCppFix))
    }
}

impl ValueEnum for InputFormat {
    fn value_variants<'a>() -> &'a [Self] {
        &[
            Self::Csv(Some(CsvHandling::WhisperCppFix)),
            Self::Csv(None),
            Self::Json,
        ]
    }

    fn to_possible_value(&self) -> Option<PossibleValue> {
        match self {
            InputFormat::Csv(Some(CsvHandling::WhisperCppFix)) => Some(
                PossibleValue::new("csv-fix").help("same as csv, plus whisper.cpp formatting fix"),
            ),
            InputFormat::Csv(None) => Some(PossibleValue::new("csv")),
            InputFormat::Json => Some(PossibleValue::new("json")),
        }
    }
}

impl InputFormat {
    fn consume_reader<'a, R: std::io::Read + 'a>(&self, reader: R) -> IterDyn<'a> {
        match self {
            Self::Csv(handling) => {
                let rdr: Box<dyn std::io::Read> = if let Some(CsvHandling::WhisperCppFix) = handling
                {
                    Box::new(BadCsvReader::new(reader))
                } else {
                    Box::new(reader)
                };
                let mut csv_reader = csv::Reader::from_reader(rdr);

                csv_reader
                    .deserialize()
                    .map(|r: TxResult| r.expect("no malformed CSV records"))
                    .collect_vec()
                    .into_iter()
                    .boxed()
            }
            Self::Json => {
                let rdr = serde_json::Deserializer::from_reader(reader).into_iter::<Timing>();
                rdr.map(|r| r.expect("no malformed JSON records")).boxed()
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum InputSource {
    Stdin,
    File(String),
}

impl InputSource {
    fn parse(s: &str) -> Result<Self, String> {
        if s == "-" {
            Ok(Self::Stdin)
        } else {
            Ok(Self::File(s.to_string()))
        }
    }
}

#[derive(Debug, Clone)]
pub enum OutputFormat {
    Csv,
    Json,
    Pretty,
}

impl ValueEnum for OutputFormat {
    fn value_variants<'a>() -> &'a [Self] {
        &[Self::Csv, Self::Json, Self::Pretty]
    }

    fn to_possible_value(&self) -> Option<PossibleValue> {
        match self {
            Self::Csv => Some(PossibleValue::new("csv")),
            Self::Json => Some(PossibleValue::new("json")),
            Self::Pretty => Some(PossibleValue::new("pretty")),
        }
    }
}

#[derive(Debug, Clone)]
pub enum OutputSink {
    Stdout,
    File(String),
}

impl OutputSink {
    fn parse(s: &str) -> Result<Self, String> {
        if s == "-" {
            Ok(Self::Stdout)
        } else {
            Ok(Self::File(s.to_string()))
        }
    }
}

#[derive(Debug, Clone)]
struct ParseDuration;

impl clap::builder::TypedValueParser for ParseDuration {
    type Value = Duration;

    fn parse_ref(
        &self,
        _cmd: &clap::Command,
        arg: Option<&clap::Arg>,
        value: &std::ffi::OsStr,
    ) -> Result<Self::Value, clap::Error> {
        let error = |kind: ErrorKind, msg: &str| -> clap::Error {
            let attribution = arg.map(|arg| format!(" for option '{}'", arg.get_id()));
            let mut e = Error::new(kind);
            e.insert(
                ContextKind::Custom,
                ContextValue::String(
                    match attribution {
                        Some(attribution) => format!("{}{}", msg, attribution),
                        None => msg.to_string(),
                    }
                    .to_owned(),
                ),
            );
            e
        };

        let Some(s) = value.to_str() else {
            return Err(error(
                ErrorKind::MissingRequiredArgument,
                "didn't receive a string",
            ));
        };

        let digits = s
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .collect::<String>();

        if digits.is_empty() {
            return Err(error(
                ErrorKind::ValueValidation,
                "no digits found in value",
            ));
        }

        let rest = s.chars().skip(digits.len()).collect::<String>();
        if rest.is_empty() {
            return Err(error(ErrorKind::ValueValidation, "no unit found in value"));
        }

        let Ok(num) = digits.parse::<usize>() else {
            return Err(error(ErrorKind::ValueValidation, "couldn't parse digits"));
        };

        let duration = match rest.as_str() {
            "s" => Duration::from_secs(num as u64),
            "ms" => Duration::from_millis(num as u64),
            _ => {
                return Err(error(
                    ErrorKind::ValueValidation,
                    "invalid duration unit; expected 's' or 'ms'",
                ))
            }
        };

        Ok(duration)
    }
}
