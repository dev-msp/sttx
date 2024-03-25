use std::{io, time::Duration};

use itertools::Itertools;

use super::{
    transcribe::{IterDyn, IteratorExt, Timing},
    vendor::BadCsvReader,
};

type TxResult = Result<Timing, csv::Error>;

#[derive(clap::Args)]
pub struct Input {
    #[arg(
        short = 'i',
        long = "input-format",
        name = "input-format",
        default_value = "csv-fix",
        value_enum
    )]
    format: Format,

    #[arg(value_parser = Source::parse)]
    source: Source,
}

impl Input {
    pub fn source(&self) -> Result<Box<dyn io::Read>, io::Error> {
        let reader: Box<dyn io::Read> = match self.source {
            Source::Stdin => Box::new(io::stdin()),
            Source::File(ref path) => Box::new(std::fs::File::open(path)?),
        };
        Ok(reader)
    }

    pub fn format(&self) -> &Format {
        &self.format
    }
}

#[derive(Debug, Clone)]
pub enum CsvHandling {
    WhisperCppFix,
}

#[derive(Debug, Clone)]
pub enum Format {
    Csv(Option<CsvHandling>),
    Json,
}

impl Default for Format {
    fn default() -> Self {
        Self::Csv(Some(CsvHandling::WhisperCppFix))
    }
}

impl clap::ValueEnum for Format {
    fn value_variants<'a>() -> &'a [Self] {
        &[
            Self::Csv(Some(CsvHandling::WhisperCppFix)),
            Self::Csv(None),
            Self::Json,
        ]
    }

    fn to_possible_value(&self) -> Option<clap::builder::PossibleValue> {
        use clap::builder::PossibleValue;
        match self {
            Format::Csv(Some(CsvHandling::WhisperCppFix)) => Some(
                PossibleValue::new("csv-fix").help("same as csv, plus whisper.cpp formatting fix"),
            ),
            Format::Csv(None) => Some(PossibleValue::new("csv")),
            Format::Json => Some(PossibleValue::new("json")),
        }
    }
}

impl Format {
    pub fn consume_reader<'a, R: io::Read + 'a>(&self, reader: R) -> IterDyn<'a> {
        match self {
            Self::Csv(handling) => {
                let mut csv_reader: csv::Reader<Box<dyn io::Read>> =
                    if let Some(CsvHandling::WhisperCppFix) = handling {
                        BadCsvReader::new(reader).into_csv_reader()
                    } else {
                        csv::Reader::from_reader(Box::new(reader))
                    };

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
pub enum Source {
    Stdin,
    File(String),
}

impl Source {
    #[allow(clippy::unnecessary_wraps)]
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
        use clap::error::{ContextKind, ContextValue, ErrorKind};
        let error = |kind: ErrorKind, msg: &str| -> clap::Error {
            let attribution = arg.map(|arg| format!(" for option '{}'", arg.get_id()));
            let mut e = clap::Error::new(kind);
            e.insert(
                ContextKind::Custom,
                ContextValue::String(
                    match attribution {
                        Some(attribution) => format!("{msg}{attribution}"),
                        None => msg.to_string(),
                    }
                    .clone(),
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
            .take_while(char::is_ascii_digit)
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
