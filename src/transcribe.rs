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

type IterDyn<'a> = Iter<Box<dyn Iterator<Item = Timing> + 'a>>;

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

    pub fn write_csv<W: std::io::Write>(self, w: W) -> csv::Result<()> {
        let mut wtr = csv::Writer::from_writer(w);
        for t in self {
            wtr.serialize(t)?;
        }
        Ok(wtr.flush()?)
    }
}

pub trait IteratorExt<'a>: Sized + Iterator<Item = Timing>
where
    Self: 'a,
{
    fn join_continuations(self) -> IterDyn<'a> {
        self.peekable()
            .batching(|it| {
                let mut acc = it.next()?;
                if it.peek().is_some_and(Timing::is_continuation) {
                    let Some(next) = it.next() else {
                        return Some(acc);
                    };

                    acc = acc.absorb(&next);
                }
                Some(acc)
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
