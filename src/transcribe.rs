use std::time::Duration;

use itertools::Itertools;

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct Timing {
    start: u32,
    end: u32,
    text: String,
}

impl std::fmt::Display for Timing {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} - {}\n{}",
            format_ms_timestamp(self.start),
            format_ms_timestamp(self.end),
            self.content()
        )
    }
}

fn format_ms_timestamp(total_ms: u32) -> String {
    let ms = total_ms % 1000;
    let s = total_ms / 1000;
    let m = s / 60;
    let h = m / 60;

    if h == 0 {
        return format!("{:02}:{:02}.{:02}", m, s % 60, ms / 10);
    }
    format!("{:02}:{:02}:{:02}.{:02}", h, m % 60, s % 60, ms / 10)
}

impl Timing {
    pub fn start(&self) -> u32 {
        self.start
    }

    pub fn duration(&self) -> u32 {
        self.end - self.start
    }

    pub fn content(&self) -> &str {
        self.text.trim()
    }

    pub fn combine(&self, other: &Self) -> Self {
        Self {
            start: self.start,
            end: other.end,
            text: format!("{}{}", self.text, other.text),
        }
    }

    /// Like combine, but does not change the timing
    pub fn absorb(&self, other: &Self) -> Self {
        Self {
            start: self.start,
            end: self.end,
            text: format!("{}{}", self.text, other.text),
        }
    }

    fn is_continuation(&self) -> bool {
        !self.text.chars().next().is_some_and(char::is_whitespace)
    }
}

pub trait TxbIter: Sized + Iterator<Item = Timing> {
    fn join_continuations(self) -> impl Iterator<Item = Self::Item> {
        self.peekable().batching(|it| {
            let mut acc = it.next()?;
            if it.peek().is_some_and(Timing::is_continuation) {
                let Some(next) = it.next() else {
                    return Some(acc);
                };

                acc = acc.absorb(&next);
            }
            Some(acc)
        })
    }

    fn sentences(self) -> impl Iterator<Item = Self::Item> {
        #[inline]
        fn is_sentence(s: &str) -> bool {
            s.chars()
                .enumerate()
                .last()
                .map_or(false, |(i, c)| i > 0 && matches!(c, '.' | '!' | '?'))
        }

        self.batching(move |it| {
            let mut acc = it.next()?;

            while !is_sentence(&acc.text) {
                let Some(next) = it.next() else {
                    return Some(acc);
                };

                acc = acc.combine(&next);
            }
            Some(acc)
        })
    }

    fn duration_windows(self, window_size: Duration) -> impl Iterator<Item = Self::Item> {
        self.batching(move |it| {
            let mut acc = it.next()?;
            while acc.duration() < window_size.as_millis() as u32 {
                let Some(next) = it.next() else {
                    return Some(acc);
                };

                acc = acc.combine(&next);
            }
            Some(acc)
        })
    }

    fn to_csv<W: std::io::Write>(self, w: W) -> csv::Result<()> {
        let mut wtr = csv::Writer::from_writer(w);
        for t in self {
            wtr.serialize(t)?;
        }
        Ok(wtr.flush()?)
    }
}

impl<I: Iterator<Item = Timing>> TxbIter for I {}
