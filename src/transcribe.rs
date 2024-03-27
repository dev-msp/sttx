use std::{io, time::Duration};

use itertools::Itertools;

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct Timing {
    start: u32,
    end: u32,
    text: String,
}

impl std::fmt::Display for Timing {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} - {} ({})\n{}",
            format_clock_value(self.start, None),
            format_clock_value(self.end, None),
            format_clock_value(self.duration(), Some(ClockScale::Seconds)),
            self.content()
        )
    }
}

impl FromIterator<Timing> for Option<Timing> {
    fn from_iter<I: IntoIterator<Item = Timing>>(iter: I) -> Self {
        let mut iter = iter.into_iter();
        let first = iter.next()?;
        Some(iter.fold(first, |acc, t| acc.combine(&t)))
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ClockScale {
    Seconds,
    Minutes,
    Hours,
}

/// Formats a total number of milliseconds into a human-readable clock value.
///
/// ```
/// use crate::transcribe::format_clock_value;
/// use crate::transcribe::ClockScale::*;
///
/// // 10, 1000, 60000, 3600000
///
/// assert_eq!(format_clock_value(10,        None),           "0:00.01");
/// assert_eq!(format_clock_value(10,        Some(Seconds)),     "0.01");
/// assert_eq!(format_clock_value(10,        Some(Minutes)),  "0:00.01");
/// assert_eq!(format_clock_value(10,        Some(Hours)), "0:00:00.01");
///
/// assert_eq!(format_clock_value(1000,      None),           "0:01.00");
/// assert_eq!(format_clock_value(1000,      Some(Seconds)),     "1.00");
/// assert_eq!(format_clock_value(1000,      Some(Minutes)),  "0:01.00");
/// assert_eq!(format_clock_value(1000,      Some(Hours)), "0:00:01.00");
///
/// assert_eq!(format_clock_value(60e3,      None),           "1:00.00");
/// assert_eq!(format_clock_value(60e3,      Some(Seconds)),    "60.00");
/// assert_eq!(format_clock_value(60e3,      Some(Minutes)),  "1:00.00");
/// assert_eq!(format_clock_value(60e3,      Some(Hours)), "0:01:00.00");
///
/// assert_eq!(format_clock_value(60 * 60e3, None),        "1:00:00.00");
/// assert_eq!(format_clock_value(60 * 60e3, Some(Seconds)),  "3600.00");
/// assert_eq!(format_clock_value(60 * 60e3, Some(Minutes)), "60:00.00");
/// assert_eq!(format_clock_value(60 * 60e3, Some(Hours)), "1:00:00.00");
/// ```
fn format_clock_value(total_ms: u32, min_clock_scale: Option<ClockScale>) -> String {
    let min_clock_scale = min_clock_scale.unwrap_or(ClockScale::Minutes);
    let ms = total_ms % 1000;
    let s = total_ms / 1000;
    let m = s / 60;
    let h = m / 60;

    match min_clock_scale {
        ClockScale::Hours => format!("{}:{:02}:{:02}.{:02}", h, m % 60, s % 60, ms / 10),
        ClockScale::Minutes => format!("{}:{:02}.{:02}", m, s % 60, ms / 10),
        ClockScale::Seconds => format!("{}.{:02}", s, ms / 10),
    }
}

impl Timing {
    #[allow(dead_code)]
    pub fn start(&self) -> u32 {
        self.start
    }

    #[allow(dead_code)]
    pub fn end(&self) -> u32 {
        self.end
    }

    #[allow(dead_code)]
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

pub struct Iter<I>
where
    I: Iterator<Item = Timing>,
{
    inner: I,
}

impl<'a, I> Iterator for Iter<I>
where
    I: Iterator<Item = Timing> + 'a,
{
    type Item = Timing;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

pub type IterDyn<'a> = Iter<Box<dyn Iterator<Item = Timing> + 'a>>;

#[inline]
fn is_sentence(s: &str) -> bool {
    s.chars()
        .enumerate()
        .last()
        .map_or(false, |(i, c)| i > 0 && matches!(c, '.' | '!' | '?'))
}

#[allow(dead_code)]
impl<'a, I> Iter<I>
where
    I: Iterator<Item = Timing> + 'a,
{
    pub fn sentences(self) -> IterDyn<'a> {
        self.batching(move |it| it.take_while_inclusive(|t| !is_sentence(&t.text)).collect())
            .boxed()
    }

    pub fn max_silence(self, max_silence: Duration) -> IterDyn<'a> {
        self.peekable()
            .batching(move |it| {
                let mut acc = it.next()?;
                let mut total_silence = 0;

                while it.peek().map_or(false, |next| {
                    total_silence + next.start - acc.end < max_silence.as_millis() as u32
                }) {
                    let Some(next) = it.next() else {
                        return Some(acc);
                    };

                    total_silence += next.start - acc.end;

                    acc = acc.combine(&next);
                }
                Some(acc)
            })
            .boxed()
    }

    pub fn min_word_count(self, min_words: usize) -> IterDyn<'a> {
        self.batching(move |it| {
            it.take_while_inclusive(|t| t.text.split_whitespace().count() < min_words)
                .collect()
        })
        .boxed()
    }

    pub fn by_gap(self, gap_size: Duration) -> IterDyn<'a> {
        self.peekable()
            .batching(move |it| {
                let mut acc = it.next()?;
                while it.peek().map_or(false, |next| {
                    next.start - acc.end < gap_size.as_millis() as u32
                }) {
                    let Some(next) = it.next() else {
                        return Some(acc);
                    };

                    acc = acc.combine(&next);
                }
                Some(acc)
            })
            .boxed()
    }

    pub fn lasting(self, window_size: Duration) -> IterDyn<'a> {
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
        .boxed()
    }

    pub fn chunks(self, chunk_count: usize) -> IterDyn<'a> {
        self.batching(move |it| it.take(chunk_count).collect())
            .boxed()
    }

    pub fn write_csv<W: io::Write>(self, w: W) -> csv::Result<()> {
        let mut wtr = csv::Writer::from_writer(w);
        for t in self {
            wtr.serialize(t)?;
        }
        Ok(wtr.flush()?)
    }

    pub fn write_json<W: io::Write>(self, w: W) -> serde_json::Result<()> {
        serde_json::to_writer(w, &self.collect::<Vec<_>>())
    }

    pub fn write_srt<W: io::Write>(self, mut w: W) -> io::Result<()> {
        fn format_srt_value(total_ms: u32) -> String {
            let ms = total_ms % 1000;
            let s = total_ms / 1000;
            let m = s / 60;
            let h = m / 60;

            format!("{:02}:{:02}:{:02},{:03}", h, m % 60, s % 60, ms)
        }

        let mut i = 1;
        for t in self {
            writeln!(w, "{}", i)?;
            writeln!(
                w,
                "{} --> {}",
                format_srt_value(t.start),
                format_srt_value(t.end)
            )?;
            writeln!(w, "{}\n", t.content())?;
            i += 1;
        }
        Ok(())
    }
}

const MAX_DURATION: Duration = Duration::from_millis(500);

pub trait IteratorExt<'a>: Sized + Iterator<Item = Timing>
where
    Self: 'a,
{
    fn join_continuations(self) -> IterDyn<'a> {
        self.peekable()
            .batching(|it| {
                // We're consuming at least one event
                let mut acc = it.next()?;

                while it.peek().is_some_and(Timing::is_continuation) {
                    let Some(next) = it.next() else {
                        return Some(acc);
                    };

                    acc = acc.absorb(&next);
                }
                Some(acc)
            })
            .map(move |mut t| {
                // limit duration of each "utterance" to something reasonable
                if t.duration() > MAX_DURATION.as_millis() as u32 {
                    t.end = t.start + MAX_DURATION.as_millis() as u32;
                }
                t
            })
            .boxed()
    }

    fn boxed(self) -> IterDyn<'a> {
        Iter {
            inner: Box::new(self),
        }
    }
}

impl<'a, I: Iterator<Item = Timing> + 'a> IteratorExt<'a> for I {}
