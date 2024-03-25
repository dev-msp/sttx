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
            "{} - {} ({})\n{}",
            format_clock_value(self.start, None),
            format_clock_value(self.end, None),
            format_clock_value(self.duration(), Some(ClockScale::Seconds)),
            self.content()
        )
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ClockScale {
    Seconds,
    Minutes,
    Hours,
}

pub fn format_clock_value(total_ms: u32, min_clock_scale: Option<ClockScale>) -> String {
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

pub fn format_srt_value(total_ms: u32) -> String {
    let ms = total_ms % 1000;
    let s = total_ms / 1000;
    let m = s / 60;
    let h = m / 60;

    format!("{:02}:{:02}:{:02},{:03}", h, m % 60, s % 60, ms)
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
            let mut acc = it.next()?;
            while acc.text.split_whitespace().count() < min_words {
                let Some(next) = it.next() else {
                    return Some(acc);
                };

                acc = acc.combine(&next);
            }
            Some(acc)
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
        self.batching(move |it| {
            let mut acc = it.next()?;
            for _ in 1..chunk_count {
                let Some(next) = it.next() else {
                    return Some(acc);
                };

                acc = acc.combine(&next);
            }
            Some(acc)
        })
        .boxed()
    }

    pub fn write_csv<W: std::io::Write>(self, w: W) -> csv::Result<()> {
        let mut wtr = csv::Writer::from_writer(w);
        for t in self {
            wtr.serialize(t)?;
        }
        Ok(wtr.flush()?)
    }

    pub fn write_json<W: std::io::Write>(self, w: W) -> serde_json::Result<()> {
        serde_json::to_writer(w, &self.collect::<Vec<_>>())
    }

    pub fn write_srt<W: std::io::Write>(self, mut w: W) -> std::io::Result<()> {
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
