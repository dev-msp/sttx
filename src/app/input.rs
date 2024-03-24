use std::time::Duration;

use clap::{
    builder::PossibleValue,
    error::{ContextKind, ContextValue, ErrorKind},
    Args, Error, ValueEnum,
};
use itertools::Itertools;

use crate::{
    transcribe::{IterDyn, IteratorExt, Timing},
    vendor::BadCsvReader,
    TxResult,
};

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

    pub fn format(&self) -> &InputFormat {
        &self.format
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
    pub fn consume_reader<'a, R: std::io::Read + 'a>(&self, reader: R) -> IterDyn<'a> {
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
pub struct ParseDuration;

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
