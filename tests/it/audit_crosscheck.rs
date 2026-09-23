//! Differential checks: independent implementations of the same contract
//! must agree on every input.

use cbor2::Value;

use crate::util::block_on;

struct Reader<'a>(&'a [u8]);
impl cbor2::async_io::AsyncRead for Reader<'_> {
    async fn read_exact(&mut self, buf: &mut [u8]) -> Result<(), cbor2::io::Error> {
        std::io::Read::read_exact(&mut self.0, buf)
    }
}

#[test]
fn crosscheck_validators_raw_and_async_on_small_and_random_inputs() {
    let check = |input: &[u8]| {
        let slice = cbor2::validate_slice(input).is_ok();
        let reader = cbor2::validate(input).is_ok();
        assert_eq!(slice, reader, "validators differ: {input:x?}");
        let mut source = Reader(input);
        let async_result = block_on(cbor2::async_io::read_item_with_limit(&mut source, 256));
        let async_valid = async_result.is_ok() && source.0.is_empty();
        assert_eq!(slice, async_valid, "async differs: {input:x?}");
        if slice {
            let raw: cbor2::RawValue = cbor2::from_slice(input).unwrap();
            assert_eq!(raw.as_bytes(), input);
            assert_eq!(cbor2::to_vec(&raw).unwrap(), input);
        }
    };
    check(&[]);
    for a in 0..=255u8 {
        check(&[a]);
    }
    for a in 0..=255u8 {
        for b in 0..=255u8 {
            check(&[a, b]);
        }
    }
    let mut state = 0xDEADBEEF01234567u64;
    let mut next = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    for _ in 0..100_000 {
        let length = (next() % 64) as usize;
        let bytes: Vec<u8> = (0..length).map(|_| next() as u8).collect();
        check(&bytes);
    }
}

// Xorshift, so the generated corpus is the same on every run.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

// Appends a head, sometimes wider than the preferred encoding.
fn head(rng: &mut Rng, out: &mut Vec<u8>, major: u8, value: u64) {
    let major = major << 5;
    if value < 24 && rng.below(4) != 0 {
        out.push(major | value as u8);
    } else if value < 256 && rng.below(3) != 0 {
        out.extend([major | 24, value as u8]);
    } else if let Ok(value) = u16::try_from(value) {
        out.push(major | 25);
        out.extend(value.to_be_bytes());
    } else if let Ok(value) = u32::try_from(value) {
        out.push(major | 26);
        out.extend(value.to_be_bytes());
    } else {
        out.push(major | 27);
        out.extend(value.to_be_bytes());
    }
}

// Appends one well-formed item, preferred or not, definite or not.
fn item(rng: &mut Rng, depth: u32, out: &mut Vec<u8>) {
    let number = |rng: &mut Rng| match rng.below(4) {
        0 => rng.below(24),
        1 => rng.below(256),
        2 => rng.below(70_000),
        _ => rng.next(),
    };
    match rng.below(if depth > 3 { 5 } else { 11 }) {
        major @ (0 | 1) => {
            let value = number(rng);
            head(rng, out, major as u8, value);
        }
        2 => {
            let len = rng.below(5);
            head(rng, out, 2, len);
            out.extend((0..len).map(|_| rng.next() as u8));
        }
        3 => {
            let text = ["", "a", "水", "zz", "é"][rng.below(5) as usize];
            head(rng, out, 3, text.len() as u64);
            out.extend(text.as_bytes());
        }
        4 => match rng.below(8) {
            simple @ 0..4 => out.push(0xf4 + simple as u8),
            4 => out.extend([0xf9, rng.next() as u8, rng.next() as u8]),
            5 => {
                out.push(0xfa);
                out.extend((rng.next() as u32).to_be_bytes());
            }
            6 => {
                out.push(0xfb);
                out.extend(rng.next().to_be_bytes());
            }
            _ => out.extend([0xf8, 32 + rng.below(224) as u8]),
        },
        kind @ 5..9 => {
            let (major, per_entry) = if kind < 7 { (4, 1) } else { (5, 2) };
            let len = rng.below(4);
            let indefinite = rng.below(3) == 0;
            if indefinite {
                out.push(major << 5 | 31);
            } else {
                head(rng, out, major, len);
            }
            for _ in 0..len * per_entry {
                item(rng, depth + 1, out);
            }
            if indefinite {
                out.push(0xff);
            }
        }
        9 => {
            let tag = [0, 1, 2, 3, 24, 55799, 100][rng.below(7) as usize];
            head(rng, out, 6, tag);
            item(rng, depth + 1, out);
        }
        _ => {
            let (major, segment) = if rng.below(2) == 0 {
                (2, 0x41)
            } else {
                (3, 0x61)
            };
            out.push(major << 5 | 31);
            for _ in 0..rng.below(3) {
                out.extend([segment, b'q']);
            }
            out.push(0xff);
        }
    }
}

// Error kinds and positions must agree; I/O error wrappers may differ.
fn outcome<T: std::fmt::Debug>(result: Result<T, cbor2::de::Error>) -> String {
    match result {
        Err(cbor2::de::Error::Io(error)) => format!("io {:?}", error.kind()),
        other => format!("{other:?}"),
    }
}

#[test]
fn crosscheck_decoders_and_encoders_on_generated_items() {
    use std::collections::BTreeMap;

    // Compares encodings so that NaN payloads compare equal.
    let same = |a: &Value, b: &Value| cbor2::to_vec(a).unwrap() == cbor2::to_vec(b).unwrap();
    let mut rng = Rng(0x1234_5678_9abc_def1);
    for n in 0..20_000 {
        let mut input = Vec::new();
        item(&mut rng, 0, &mut input);
        if n % 4 == 3 {
            let at = rng.below(input.len() as u64) as usize;
            input[at] ^= 1 << rng.below(8);
        }
        let valid = cbor2::validate_slice(&input).is_ok();
        assert_eq!(valid, cbor2::validate(&input[..]).is_ok(), "{input:x?}");

        macro_rules! typed {
            ($($ty:ty),*) => {$(
                assert_eq!(
                    outcome(cbor2::from_slice::<$ty>(&input)),
                    outcome(cbor2::from_reader::<$ty, _>(&input[..])),
                    "{}: {input:x?}",
                    stringify!($ty)
                );
            )*};
        }
        typed!(u64, i128, bool, f64, String, Vec<u8>, Option<u64>, BTreeMap<String, u8>);

        let diagnostic = cbor2::to_cdn(&input[..]);
        assert_eq!(valid, diagnostic.is_ok(), "{input:x?}");
        let value = cbor2::from_slice::<Value>(&input);
        let from_reader = cbor2::from_reader::<Value, _>(&input[..]);
        assert_eq!(value.is_ok(), from_reader.is_ok(), "{input:x?}");
        let Ok(value) = value else {
            assert!(!valid, "well-formed item rejected: {input:x?}");
            continue;
        };
        assert!(same(&value, &from_reader.unwrap()), "{input:x?}");

        // Diagnostic notation reads back as the same item.
        if let Ok(text) = diagnostic {
            let bytes = cbor2::cdn_to_vec(&text).unwrap();
            assert!(same(&value, &cbor2::from_slice(&bytes).unwrap()), "{text}");
        }

        // Both canonical encoders agree, and their output is a fixed point.
        let direct = cbor2::to_canonical_vec(&value);
        let mut sorted = value.clone();
        let transformed = sorted
            .canonicalize()
            .map(|()| cbor2::to_vec(&sorted).unwrap());
        assert_eq!(direct.is_ok(), transformed.is_ok(), "{input:x?}");
        if let (Ok(direct), Ok(transformed)) = (direct, transformed) {
            assert_eq!(direct, transformed, "{input:x?}");
            let again: Value = cbor2::from_slice(&direct).unwrap();
            assert_eq!(cbor2::to_canonical_vec(&again).unwrap(), direct);
        }

        let encoded = cbor2::to_vec(&value).unwrap();
        assert_eq!(
            cbor2::serialized_size(&value).unwrap(),
            encoded.len() as u64
        );
        assert!(cbor2::validate_slice(&encoded).is_ok(), "{input:x?}");
        if valid {
            let raw: cbor2::RawValue = cbor2::from_slice(&input).unwrap();
            assert_eq!(raw.as_bytes(), &input[..]);
        }
    }
}
