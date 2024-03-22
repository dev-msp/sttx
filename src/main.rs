use std::{
    collections::{HashMap, VecDeque},
    io::{BufReader, BufWriter},
    time::Duration,
};

use itertools::Itertools;

fn char_offsets(s: &str) -> impl Iterator<Item = (usize, char)> + '_ {
    s.chars().scan(0, |n, c| {
        let old_n = *n;
        *n = old_n + c.len_utf8();
        Some((old_n, c))
    })
}

fn remove_bookending_quotes(line: &str) -> String {
    let offsets = {
        let mut bo = char_offsets(line)
            .filter_map(|(offset, c)| (c == '"').then_some(offset))
            .collect::<VecDeque<_>>();

        bo.pop_front().zip(bo.pop_back())
    };

    let Some((left, right)) = offsets else {
        return line.to_owned();
    };

    let mut new_line = String::new();
    new_line.push_str(&line[..left]);
    new_line.push_str(&line[left + 1..right]);
    new_line.push_str(&line[right + 1..]);

    new_line
}

fn handle_quotes<R: std::io::BufRead, W: std::io::Write>(
    reader: R,
    mut writer: W,
) -> Result<(), std::io::Error> {
    for line in reader.lines() {
        let line = line.unwrap();
        let char_counts = line.chars().fold(HashMap::new(), |mut map, c| {
            *map.entry(c).or_insert(0) += 1;
            map
        });

        let char_count = |c: char| -> usize { *char_counts.get(&c).unwrap_or(&0) };
        let line_to_write = if char_count(',') == 2 {
            remove_bookending_quotes(&line)
        } else {
            line
        };
        writer.write_all(format!("{line_to_write}\n").as_bytes())?;
    }
    Ok(())
}

#[derive(Debug, serde::Deserialize)]
struct TranscribeTiming {
    start: u32,
    end: u32,
    text: String,
}

impl TranscribeTiming {
    fn duration(&self) -> u32 {
        self.end - self.start
    }

    fn content(&self) -> &str {
        self.text.trim()
    }

    fn combine(&self, other: &Self) -> Self {
        Self {
            start: self.start,
            end: other.end,
            text: format!("{}{}", self.text, other.text),
        }
    }
}

fn is_continuation(t: &TranscribeTiming) -> bool {
    !t.text.chars().next().is_some_and(char::is_whitespace)
}

trait TxbIter: Sized + Iterator<Item = TranscribeTiming> {
    fn fold_punctuation(self) -> impl Iterator<Item = Self::Item> {
        self.peekable().batching(move |it| {
            let mut acc = it.next()?;
            if it.peek().is_some_and(is_continuation) {
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

impl<I: Iterator<Item = TranscribeTiming>> TxbIter for I {}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let file_path = &args[1];
    let file = std::fs::File::open(file_path).unwrap();
    let r = BufReader::new(file);
    let mut bytes = Vec::new();
    handle_quotes(r, BufWriter::new(&mut bytes)).unwrap();

    type TxResult = Result<TranscribeTiming, csv::Error>;
    let mut rdr = csv::Reader::from_reader(bytes.as_slice());
    let timings = rdr
        .deserialize()
        .filter_map(|r: TxResult| r.ok())
        .fold_punctuation()
        .duration_windows(Duration::from_secs(60));

    for t in timings {
        println!("\nstart: {}, content:\n{}", t.start, t.content());
    }
}
