use std::time::Duration;

use itertools::Itertools;

#[derive(Debug, serde::Deserialize)]
pub struct Timing {
    start: u32,
    end: u32,
    text: String,
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

    fn is_continuation(&self) -> bool {
        !self.text.chars().next().is_some_and(char::is_whitespace)
    }
}

pub trait TxbIter: Sized + Iterator<Item = Timing> {
    fn join_continuations(self) -> impl Iterator<Item = Self::Item> {
        self.peekable().batching(move |it| {
            let mut acc = it.next()?;
            if it.peek().is_some_and(Timing::is_continuation) {
                let Some(next) = it.next() else {
                    return Some(acc);
                };

                acc = acc.combine(&next);
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
}

impl<I: Iterator<Item = Timing>> TxbIter for I {}
