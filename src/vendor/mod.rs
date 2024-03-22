use std::collections::VecDeque;

use std::io::{BufRead, BufReader, Read};

pub struct BadCsvReader<R> {
    inner: BufReader<R>,
}

impl<R: Read> BadCsvReader<R> {
    pub fn new(inner: R) -> Self {
        Self {
            inner: BufReader::new(inner),
        }
    }
}

/// Removes quotes from lines that have exactly two commas (in other words, lines without commas
/// needing escaping)
///
/// Specifically intended to handle poorly-formatted CSV content generated by whisper.cpp.
impl<R: Read> Read for BadCsvReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let mut line = String::new();
        let n = self.inner.read_line(&mut line)?;
        if n == 0 {
            return Ok(0);
        }
        let comma_count = line.chars().filter(|&c| c == ',').count();
        let mut line_to_write = if comma_count == 2 {
            remove_bookending_quotes(&line)
        } else {
            line
        };
        line_to_write.push('\n');

        let len = line_to_write.len();
        buf[..len].copy_from_slice(line_to_write.as_bytes());
        Ok(len)
    }
}
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
