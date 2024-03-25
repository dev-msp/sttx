use clap::{builder::PossibleValue, Args, ValueEnum};

#[derive(Args)]
pub struct Output {
    #[arg(short = 'f', long = "format", default_value = "pretty", value_enum)]
    format: Format,

    /// The path to which the program should write the output. Use `-` for stdout.
    #[arg(short = 'o',  long = "output", default_value = "-", value_parser = Sink::parse)]
    sink: Sink,
}

impl Output {
    pub fn sink(&self) -> Result<Box<dyn std::io::Write>, std::io::Error> {
        Ok(match self.sink {
            Sink::Stdout => Box::new(std::io::stdout()),
            Sink::File(ref path) => Box::new(std::fs::File::create(path)?),
        })
    }

    pub fn format(&self) -> &Format {
        &self.format
    }
}

#[derive(Debug, Clone)]
pub enum Format {
    Csv,
    Json,
    Pretty,
}

impl ValueEnum for Format {
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
pub enum Sink {
    Stdout,
    File(String),
}

impl Sink {
    #[allow(clippy::unnecessary_wraps)]
    fn parse(s: &str) -> Result<Self, String> {
        if s == "-" {
            Ok(Self::Stdout)
        } else {
            Ok(Self::File(s.to_string()))
        }
    }
}
