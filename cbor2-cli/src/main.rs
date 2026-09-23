//! `cbor` — the command line CBOR converter and inspector.
//!
//! Without a command, shows every CBOR item in the input as pretty diagnostic
//! notation (RFC 8949 §8), exactly as it appears on the wire.
//! `decode` shows CBOR items as pretty-printed diagnostic notation, or
//! converts them to pretty-printed JSON with `--json`; `--diag` and `--cdn`
//! are explicit spellings of the default diagnostic/CDN output. `encode`
//! converts JSON-compatible values or CDN text into CBOR items, optionally
//! restricted with `--json`, `--diag` or `--cdn`, and optionally as copyable
//! hex text with `--hex`;
//! `validate` checks one or more complete CBOR items. Data errors exit with
//! status 1, usage errors with status 2.
//!
//! Install with Homebrew or Cargo:
//!
//! ```text
//! brew install ldclabs/tap/cbor2-cli   # installs cbor
//! cargo install cbor2-cli              # installs cbor
//! ```

use std::env;
use std::fs::File;
use std::io::{self, BufReader, BufWriter, Cursor, Read, Write};
use std::path::Path;
use std::process;

use cbor2::{RawValue, Value};

const USAGE: &str = "\
Usage: cbor [COMMAND] [INPUT]

Shows, decodes and encodes CBOR (RFC 8949). Without a command, every
CBOR item in INPUT is shown as pretty diagnostic notation (\u{a7}8),
exactly as it appears on the wire.

Commands:
  decode  Show CBOR items as pretty-printed diagnostic notation,
          or convert them to pretty-printed JSON with --json
  encode  Convert JSON-compatible values or CDN text to CBOR items
  validate
          Validate one or more complete CBOR items

Input:
  INPUT is a file path, a hex string (optionally 0x-prefixed), a base64
  or base64url string, or `-` for stdin; stdin is the default. An
  argument containing a path separator is always a file path. `encode`
  reads JSON-compatible values or CDN text from a file or stdin only.
  Use --json or --diag/--cdn to restrict the accepted input syntax.
  CBOR-reading commands read raw CBOR bytes from files and stdin.
  Output goes to stdout.

Options:
  -d, --diag     With `decode`: print diagnostic notation (the default)
                 With `encode`: read only Concise Diagnostic Notation
      --cdn      With `decode`: print Concise Diagnostic Notation (the default)
                 With `encode`: read only Concise Diagnostic Notation
      --json     With `decode`: print pretty JSON instead of diagnostic notation
                 With `encode`: read only JSON text
      --hex      With `encode`: print lowercase hex text instead of raw bytes
      --         End options (e.g. cbor -- -Ds= for base64url input)
  -h, --help     Print this help
  -V, --version  Print the version

Examples:
  cbor a201020326                  # show hex CBOR
  cbor decode message.cbor         # CBOR file -> diagnostic notation
  cbor decode --json message.cbor  # CBOR file -> pretty JSON
  printf \"{1: h'dead'}\" | cbor encode --hex
  echo '{\"a\": 1}' | cbor encode --hex
  echo '{\"a\": 1}' | cbor encode    # JSON -> CBOR bytes";

enum Command {
    Show,
    Decode,
    Encode,
    Validate,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DecodeOutput {
    Diag,
    Json,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum EncodeOutput {
    Raw,
    Hex,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum EncodeInput {
    Json,
    Cdn,
}

struct Args {
    command: Command,
    decode_output: DecodeOutput,
    encode_output: EncodeOutput,
    encode_input: EncodeInput,
    input: Option<String>,
}

fn main() {
    let Args {
        command,
        decode_output,
        encode_output,
        encode_input,
        input,
    } = parse_args();

    let result = match command {
        Command::Show => decode(open_cbor_input(input.as_deref()), DecodeOutput::Diag),
        Command::Decode => decode(open_cbor_input(input.as_deref()), decode_output),
        Command::Encode => encode(
            open_text_input(input.as_deref()),
            encode_output,
            encode_input,
        ),
        Command::Validate => validate(open_cbor_input(input.as_deref())),
    };

    if let Err(err) = result {
        if is_broken_pipe(err.as_ref()) {
            return;
        }
        eprintln!("cbor: {err}");
        process::exit(1);
    }
}

// Parses the command line. `-h`/`--help` and `-V`/`--version` print and
// exit; anything malformed exits with 2.
fn parse_args() -> Args {
    let mut diag = false;
    let mut cdn = false;
    let mut json = false;
    let mut encode_output = EncodeOutput::Raw;
    let mut positional = Vec::new();
    let mut input_only = false;

    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--" => {
                input_only = positional.is_empty();
                positional.extend(args);
                break;
            }
            "-h" | "--help" => {
                println!("{USAGE}");
                process::exit(0);
            }
            "-V" | "--version" => {
                println!("cbor {}", env!("CARGO_PKG_VERSION"));
                process::exit(0);
            }
            "-d" | "--diag" => diag = true,
            "--cdn" => cdn = true,
            "--json" => json = true,
            "--hex" => encode_output = EncodeOutput::Hex,
            _ if arg.starts_with('-') && arg != "-" => {
                usage_error(format_args!("unrecognized option `{arg}`"));
            }
            _ => positional.push(arg),
        }
    }

    let mut positional = positional.into_iter().peekable();
    let command = match positional
        .peek()
        .map(String::as_str)
        .filter(|_| !input_only)
    {
        Some("decode") => {
            positional.next();
            Command::Decode
        }
        Some("encode") => {
            positional.next();
            Command::Encode
        }
        Some("validate") => {
            positional.next();
            Command::Validate
        }
        _ => Command::Show,
    };

    let input = positional.next();
    if positional.next().is_some() {
        usage_error(format_args!("at most one INPUT argument"));
    }
    if json && !matches!(command, Command::Decode | Command::Encode) {
        usage_error(format_args!(
            "`--json` only applies to `decode` or `encode`"
        ));
    }
    if diag && !matches!(command, Command::Decode | Command::Encode) {
        usage_error(format_args!(
            "`--diag` only applies to `decode` or `encode`"
        ));
    }
    if cdn && !matches!(command, Command::Decode | Command::Encode) {
        usage_error(format_args!("`--cdn` only applies to `decode` or `encode`"));
    }
    if json && (diag || cdn) {
        usage_error(format_args!(
            "`--json` cannot be combined with `--diag` or `--cdn`"
        ));
    }
    if encode_output == EncodeOutput::Hex && !matches!(command, Command::Encode) {
        usage_error(format_args!("`--hex` only applies to `encode`"));
    }

    let decode_output = if json {
        DecodeOutput::Json
    } else {
        DecodeOutput::Diag
    };

    let encode_input = if json {
        EncodeInput::Json
    } else {
        EncodeInput::Cdn
    };

    Args {
        command,
        decode_output,
        encode_output,
        encode_input,
        input,
    }
}

fn usage_error(msg: core::fmt::Arguments<'_>) -> ! {
    eprintln!("cbor: {msg}");
    eprintln!("Try `cbor --help`.");
    process::exit(2);
}

// Opens the input of the CBOR-reading commands: stdin (absent or `-`),
// an existing file, a hex string or a base64/base64url string.
fn open_cbor_input(arg: Option<&str>) -> Box<dyn Read> {
    let arg = match arg {
        None | Some("-") => return Box::new(BufReader::new(io::stdin().lock())),
        Some(arg) => arg,
    };

    if Path::new(arg).exists() {
        match File::open(arg) {
            Ok(file) => return Box::new(BufReader::new(file)),
            Err(err) => usage_error(format_args!("{arg}: {err}")),
        }
    }

    // Anything with a path separator is always a path — `/` is also a
    // standard-base64 character, and a mistyped file name must not be
    // decoded as inline data. Use base64url or externally decode to raw CBOR
    // before piping to stdin when standard base64 contains `/`.
    if arg.contains('/') || arg.contains('\\') {
        usage_error(format_args!("{arg}: no such file"));
    }

    if let Some(bytes) = from_hex(arg) {
        return Box::new(Cursor::new(bytes));
    }
    if let Some(bytes) = from_base64(arg) {
        return Box::new(Cursor::new(bytes));
    }

    usage_error(format_args!(
        "`{arg}` is not a file, a hex string or a base64 string"
    ));
}

// Opens the input of `encode`: stdin (absent or `-`) or a text file.
fn open_text_input(arg: Option<&str>) -> Box<dyn Read> {
    match arg {
        None | Some("-") => Box::new(BufReader::new(io::stdin().lock())),
        Some(path) => match File::open(path) {
            Ok(file) => Box::new(BufReader::new(file)),
            Err(err) => usage_error(format_args!("{path}: {err}")),
        },
    }
}

type Error = Box<dyn std::error::Error>;

// These serializers wrap output errors instead of always exposing io::Error
// through Error::source. A consumer such as `head` may close stdout early.
fn is_broken_pipe(error: &(dyn std::error::Error + 'static)) -> bool {
    if let Some(error) = error.downcast_ref::<io::Error>() {
        return error.kind() == io::ErrorKind::BrokenPipe;
    }
    if let Some(error) = error.downcast_ref::<serde_json::Error>() {
        return error.io_error_kind() == Some(io::ErrorKind::BrokenPipe);
    }
    matches!(error.downcast_ref::<cbor2::ser::Error>(),
        Some(cbor2::ser::Error::Io(error)) if error.kind() == io::ErrorKind::BrokenPipe)
}

// Decodes each CBOR item and pretty-prints it as diagnostic notation or, with
// `--json`, as JSON. The diagnostic path works on wire bytes and preserves
// indefinite-length markers; the JSON path re-spells through `Value`.
fn decode(input: Box<dyn Read>, output: DecodeOutput) -> Result<(), Error> {
    let stdout = io::stdout();
    let mut stdout = stdout.lock();

    match output {
        DecodeOutput::Diag => {
            for item in cbor2::de::Deserializer::from_reader(input).into_iter::<RawValue>() {
                let text = cbor2::to_cdn_pretty(item?.as_ref())?;
                writeln!(stdout, "{text}")?;
            }
        }
        DecodeOutput::Json => {
            let mut stdout = BufWriter::new(stdout);
            for item in cbor2::de::Deserializer::from_reader(input).into_iter::<Value>() {
                let json = to_json(item?)?;
                serde_json::to_writer_pretty(&mut stdout, &json)?;
                stdout.write_all(b"\n")?;
                // Keep complete items visible even while the input stays open.
                stdout.flush()?;
            }
            return Ok(stdout.flush()?);
        }
    }

    Ok(stdout.flush()?)
}

// Validates one or more complete CBOR items. This is deliberately a sequence
// check because the rest of the CLI accepts CBOR sequences item by item.
fn validate(input: Box<dyn Read>) -> Result<(), Error> {
    let mut count = 0usize;
    for item in cbor2::de::Deserializer::from_reader(input).into_iter::<serde::de::IgnoredAny>() {
        item?;
        count += 1;
    }

    if count == 0 {
        return Err("expected at least one CBOR item".into());
    }

    let stdout = io::stdout();
    let mut stdout = stdout.lock();
    writeln!(stdout, "valid")?;
    Ok(stdout.flush()?)
}

// Reads a stream with the CDN parser by default, or the strict JSON parser
// with `--json`, and writes it to stdout as CBOR items. Raw output streams
// bytes; hex output streams one copyable lowercase hex string for the complete
// CBOR sequence.
fn encode(mut input: Box<dyn Read>, output: EncodeOutput, mode: EncodeInput) -> Result<(), Error> {
    let stdout = io::stdout();
    let mut stdout = stdout.lock();
    let mut wrote_hex = false;

    if mode != EncodeInput::Json {
        let mut text = String::new();
        input.read_to_string(&mut text)?;
        let bytes = cbor2::cdn_sequence_to_vec(&text)?;
        match output {
            EncodeOutput::Raw => stdout.write_all(&bytes)?,
            EncodeOutput::Hex => {
                write_hex(&mut stdout, &bytes)?;
                stdout.write_all(b"\n")?;
            }
        }
        return Ok(stdout.flush()?);
    }

    for value in serde_json::Deserializer::from_reader(input).into_iter::<serde_json::Value>() {
        match output {
            EncodeOutput::Raw => cbor2::to_writer(&value?, &mut stdout)?,
            EncodeOutput::Hex => {
                let item = cbor2::to_vec(&value?)?;
                write_hex(&mut stdout, &item)?;
                wrote_hex = true;
            }
        }
    }

    if wrote_hex {
        stdout.write_all(b"\n")?;
    }

    Ok(stdout.flush()?)
}

// Converts a CBOR value to the closest JSON value.
//
// CBOR constructs that have no JSON equivalent are converted as follows:
// byte strings become lowercase hex strings, non-string map keys are
// JSON-encoded into strings, non-finite floats become null, tags are
// dropped (the inner value is kept), generic simple values become
// `simple(N)` strings, integers beyond the 64-bit ranges become strings
// and the "undefined" simple value becomes null. Colliding object keys are
// rejected, including collisions in nested values or compound keys.
fn to_json(value: Value) -> Result<serde_json::Value, Error> {
    use serde_json::Value as Json;

    Ok(match value {
        Value::Null => Json::Null,
        Value::Bool(x) => Json::Bool(x),
        Value::Integer(x) => match (u64::try_from(x), i64::try_from(x)) {
            (Ok(x), _) => Json::from(x),
            (_, Ok(x)) => Json::from(x),
            // Outside both ranges (e.g. near -2^64): fall back to a string.
            _ => Json::String(i128::from(x).to_string()),
        },
        Value::Float(x) => serde_json::Number::from_f64(x).map_or(Json::Null, Json::Number),
        Value::Bytes(x) => Json::String(hex(&x)),
        Value::Text(x) => Json::String(x),
        Value::Simple(x) => Json::String(format!("simple({})", x.value())),
        Value::Tag(_, x) => to_json(*x)?,
        Value::Array(x) => Json::Array(x.into_iter().map(to_json).collect::<Result<_, _>>()?),
        Value::Map(x) => {
            let mut map = serde_json::Map::new();
            for (key, value) in x {
                let key = match key {
                    Value::Text(s) => s,
                    other => serde_json::to_string(&to_json(other)?)?,
                };
                match map.entry(key) {
                    serde_json::map::Entry::Vacant(entry) => {
                        entry.insert(to_json(value)?);
                    }
                    serde_json::map::Entry::Occupied(entry) => {
                        return Err(format!(
                            "duplicate JSON object key {:?} after CBOR key conversion",
                            entry.key()
                        )
                        .into());
                    }
                }
            }
            Json::Object(map)
        }
        _ => Json::Null,
    })
}

const HEX: &[u8; 16] = b"0123456789abcdef";

// Bound temporary memory independently of the encoded item size.
fn write_hex(output: &mut impl Write, bytes: &[u8]) -> io::Result<()> {
    let mut buffer = [0u8; 4096];
    for chunk in bytes.chunks(buffer.len() / 2) {
        for (byte, pair) in chunk.iter().zip(buffer.as_chunks_mut::<2>().0) {
            pair[0] = HEX[(byte >> 4) as usize];
            pair[1] = HEX[(byte & 15) as usize];
        }
        output.write_all(&buffer[..chunk.len() * 2])?;
    }
    Ok(())
}

fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(char::from(HEX[(b >> 4) as usize]));
        out.push(char::from(HEX[(b & 15) as usize]));
    }
    out
}

// Decodes a hex string — optionally 0x-prefixed, ASCII whitespace
// ignored — or returns `None` if the text is not hex.
fn from_hex(text: &str) -> Option<Vec<u8>> {
    let digits: Vec<u8> = text.bytes().filter(|b| !b.is_ascii_whitespace()).collect();
    let digits = digits
        .strip_prefix(b"0x")
        .or_else(|| digits.strip_prefix(b"0X"))
        .unwrap_or(&digits);

    if digits.is_empty() || !digits.len().is_multiple_of(2) {
        return None;
    }

    digits
        .chunks(2)
        .map(|pair| {
            let hi = char::from(pair[0]).to_digit(16)?;
            let lo = char::from(pair[1]).to_digit(16)?;
            Some((hi << 4 | lo) as u8)
        })
        .collect()
}

// Decodes a base64 or base64url string — padded or not, ASCII whitespace
// ignored — or returns `None` if the text is not base64.
fn from_base64(text: &str) -> Option<Vec<u8>> {
    fn sextet(b: u8) -> Option<u32> {
        Some(match b {
            b'A'..=b'Z' => b - b'A',
            b'a'..=b'z' => b - b'a' + 26,
            b'0'..=b'9' => b - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            _ => return None,
        } as u32)
    }

    let data: Vec<u8> = text.bytes().filter(|b| !b.is_ascii_whitespace()).collect();
    let unpadded = data.iter().position(|&b| b == b'=').unwrap_or(data.len());
    let padding = data.len() - unpadded;
    if padding > 0
        && (padding > 2
            || !data.len().is_multiple_of(4)
            || data[unpadded..].iter().any(|&b| b != b'='))
    {
        return None;
    }
    let data = &data[..unpadded];
    if data.is_empty() || data.len() % 4 == 1 {
        return None;
    }

    let mut out = Vec::with_capacity(data.len() * 3 / 4);
    for chunk in data.chunks(4) {
        let mut acc = 0u32;
        for &b in chunk {
            acc = acc << 6 | sextet(b)?;
        }
        match chunk.len() {
            4 => out.extend_from_slice(&[(acc >> 16) as u8, (acc >> 8) as u8, acc as u8]),
            3 if acc & 3 == 0 => out.extend_from_slice(&[(acc >> 10) as u8, (acc >> 2) as u8]),
            2 if acc & 15 == 0 => out.push((acc >> 4) as u8),
            _ => return None,
        }
    }

    Some(out)
}
