use std::time::Duration;

use clap::{
    error::{ContextKind, ContextValue, ErrorKind},
    Error, Parser,
};

#[derive(Parser)]
#[command(version, about, long_about = None)]
pub struct App {
    #[arg(short = 'f', long = "format", default_value = "pretty", value_parser = OutputKind::parse)]
    output_type: OutputKind,

    #[arg(short = 'o',  long = "output", default_value = "-", value_parser = OutputSink::parse)]
    output_sink: OutputSink,

    #[arg(value_parser = InputSource::parse)]
    input: InputSource,

    /// Concatenates speech events, stopping when the accumulated delay between events exceeds the
    /// given duration.
    #[arg(long, value_parser = ParseDuration)]
    max_silence: Option<Duration>,

    /// Concatenates speech events, stopping at the next sentence ending ('.', '!', or '?')
    #[arg(short, long, default_value = "false")]
    sentences: bool,

    /// Concatenates speech events, stopping when the total word count of the result exceeds the
    /// given value.
    #[arg(short = 'w', long)]
    min_word_count: Option<usize>,

    /// Concatenates speech events, stopping when the delay until the start of the next events
    #[arg(short = 'g', long, value_parser = ParseDuration)]
    by_gap: Option<Duration>,

    /// Concatenates speech events, stopping when the total duration of the result exceeds the
    /// given value.
    #[arg(short, long, value_parser = ParseDuration)]
    lasting: Option<Duration>,
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
pub enum OutputKind {
    Csv,
    Pretty,
}

impl OutputKind {
    fn parse(s: &str) -> Result<Self, String> {
        match s {
            "csv" => Ok(Self::Csv),
            "pretty" => Ok(Self::Pretty),
            _ => Err("invalid output kind".to_string()),
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

#[allow(dead_code)]
impl App {
    pub fn input(&self) -> &InputSource {
        &self.input
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

    pub fn sentences(&self) -> bool {
        self.sentences
    }

    pub fn output(&self) -> OutputKind {
        self.output_type.clone()
    }

    pub fn source(&self) -> std::io::Result<Box<dyn std::io::Read>> {
        let reader: Box<dyn std::io::Read> = match self.input {
            InputSource::Stdin => Box::new(std::io::stdin()),
            InputSource::File(ref path) => Box::new(std::fs::File::open(path)?),
        };
        Ok(reader)
    }

    pub fn sink(&self) -> std::io::Result<Box<dyn std::io::Write>> {
        let writer: Box<dyn std::io::Write> = match self.output_sink {
            OutputSink::Stdout => Box::new(std::io::stdout()),
            OutputSink::File(ref path) => Box::new(std::fs::File::create(path)?),
        };
        Ok(writer)
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
