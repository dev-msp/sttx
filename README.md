# sttx

Utility belt for transforming speech-to-text data. Pronounced "sticks".

## Install

`cargo install sttx`

## Usage

```sh
# Prepare original audio for transcription
ffmpeg -i memo.mp3 -ac 1 -ar 16000 output.wav

# Transcribe WAV audio with whisper.cpp
# -ml (--max-length): The maximum number of tokens in each output segment
# -sow (--split-on-word): Split the output on word boundaries
whisper.cpp --model "$MODEL" -ml 1 -sow -ocsv output.wav

sttx -s -g 500ms output.wav.csv
```

## Use cases

### Working with `whisper.cpp` output

[`whisper.cpp`](https://github.com/ggerganov/whisper.cpp) is a fantastic piece
of software offering state-of-the-art speech-to-text capability. It is a fairly
low-level program, and its output is not fully configurable. Given an audio
file as input, it can produce text in CSV, SRT, or plain text formats,
including timestamps.

The resolution of the data it gives you is controllable via the max length flag
(`-ml`). Note that the unit of length is tokens unless the split on word flag
(`-sow`) is enabled.

`whisper.cpp -i <input> -ml 1 -sow`

However, at best this only allows us to constrain the output by accumulating
chunks of N words.

`sttx` stakes its utility on the notion that even with no other additional
context, one can transform timestamped STT data into more useful
representations.

At its core, it offers stackable strategies for reducing a sequence of
timestamped speech events to a single event, taking advantage of the fact that
these events (start/end timestamps and text content) are a semigroup (e.g.
something that can be added to itself).

The strategies include:

- `--sentences`: Until the next sentence ending
- `--lasting`: Concatenating until a certain duration has been reached
- `--max-silence`: Until the summed total duration of the gaps in events exceeds the given amount
- `--by-gap`: Until the gap between this event and the next one exceeds the given amount
- `--min-word-count`: Until the total word count of the result exceeds the given figure
- `--chunk-size`: The next N events

For example, if you have a sequence of events like this:

```csv
start,end,text
0,1000, Hel
1000,1100,lo
1100,2000, world
2000,2000,!
2500,3000, How
3100,3500, are
4100,5000, you
5000,5000,?
6300,6700, I'm
6800,7200, fine
7200,7200,","
7300,7500, thanks
7500,7500,!
```

By default, `sttx` combines events without leading whitespace to the previous
event. So with no arguments, the expected output would be:

```csv
start,end,text
0,1100, Hello
1100,2000, world!
2500,3000, How
3100,3500, are
4100,5000, you?
6300,6700, I'm
6800,7200," fine,"
7300,7500, thanks!
```

With the `--sentences` flag, the output would be:

```csv
start,end,text
0,2000, Hello world!
2500,5000, How are you?
6300,7500," I'm fine, thanks!"
```

And `--sentences --chunk-size 2` gives you:

```csv
start,end,text
0,5000, Hello world! How are you?
6300,7500," I'm fine, thanks!"
```

Other output formats are supported:

`--format json`:

```json
[
  {
    "start": 0,
    "end": 5000,
    "text": " Hello world! How are you?"
  },
  {
    "start": 6300,
    "end": 7500,
    "text": " I'm fine, thanks!"
  }
]
```

`--format srt`:

```srt
1
00:00:00,000 --> 00:00:05,000
Hello world! How are you?

2
00:00:06,300 --> 00:00:07,500
I'm fine, thanks!
```

## Usage

```txt
Usage: sttx transform [OPTIONS] <SOURCE>

Arguments:
  <SOURCE>


Options:
  -i, --input-format <input-format>
          [default: csv-fix]

          Possible values:
          - csv-fix: same as csv, plus whisper.cpp formatting fix
          - csv
          - json

  -f, --format <FORMAT>
          [default: pretty]
          [possible values: csv, json, srt, pretty]

  -o, --output <SINK>
          The path to which the program should write the output. Use `-` for stdout

          [default: -]

      --max-silence <MAX_SILENCE>
          Concatenates until the accumulated delay between events exceeds the given duration

  -s, --sentences
          Concatenates up to the next sentence ending ('.', '!', or '?')

  -w, --min-word-count <MIN_WORD_COUNT>
          Concatenates until the total word count of the result exceeds the given value

  -g, --by-gap <BY_GAP>
          Concatenates until the delay until the start of the next event exceeds the given duration

  -l, --lasting <LASTING>
          Concatenates until the total duration of the result exceeds the given value

  -c, --chunk-size <CHUNK_SIZE>
          Concatenates up to N events

  -h, --help
          Print help (see a summary with '-h')
```

As of this writing (2024-03-25), `whisper.cpp`'s CSV output does not appear to
escape double quotes correctly. This finding may be my own error, but if not
I'll file an issue.
