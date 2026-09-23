//! Shared fixtures for the comparative CBOR benchmarks.
//!
//! Three payload shapes exercise different parts of a codec:
//!
//! * [`int_array`] — a flat `Vec<u64>`, the pure integer-header throughput
//!   case that every crate encodes natively.
//! * [`log_batch`] / [`log_batch_mini`] — a batch of structured telemetry
//!   records mixing text, integers, floats, booleans and nested lists: the
//!   "real document" case. The serde crates encode it as text-keyed maps;
//!   minicbor uses its idiomatic positional array form. Each crate is
//!   benchmarked on its *own* natural encoding, so the byte sizes differ
//!   slightly — that is part of what the comparison shows.
//! * [`blob`] — a single large byte string (major type 2), the COSE /
//!   crypto-payload case.
//!
//! Data is generated from a tiny deterministic PRNG so every run, and every
//! crate, sees byte-identical input without pulling in `rand`.

use serde::{Deserialize, Serialize};

/// SplitMix64 — a deterministic, dependency-free PRNG for fixtures.
struct SplitMix64(u64);

impl SplitMix64 {
    fn new(seed: u64) -> Self {
        Self(seed)
    }

    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
}

/// A structured telemetry record, serde flavor (text-keyed map on the wire).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LogEntry {
    pub timestamp: u64,
    pub level: u8,
    pub target: String,
    pub message: String,
    pub line: u32,
    pub success: bool,
    pub latency_ms: f64,
    pub labels: Vec<String>,
}

/// The same record for minicbor, encoded as a positional CBOR array
/// (`#[cbor(array)]`) — minicbor's idiomatic, compact form.
#[derive(Clone, Debug, PartialEq, minicbor::Encode, minicbor::Decode)]
#[cbor(array)]
pub struct LogEntryMini {
    #[n(0)]
    pub timestamp: u64,
    #[n(1)]
    pub level: u8,
    #[n(2)]
    pub target: String,
    #[n(3)]
    pub message: String,
    #[n(4)]
    pub line: u32,
    #[n(5)]
    pub success: bool,
    #[n(6)]
    pub latency_ms: f64,
    #[n(7)]
    pub labels: Vec<String>,
}

impl From<&LogEntry> for LogEntryMini {
    fn from(e: &LogEntry) -> Self {
        Self {
            timestamp: e.timestamp,
            level: e.level,
            target: e.target.clone(),
            message: e.message.clone(),
            line: e.line,
            success: e.success,
            latency_ms: e.latency_ms,
            labels: e.labels.clone(),
        }
    }
}

const TARGETS: [&str; 6] = [
    "net::http::server",
    "db::pool",
    "auth::token",
    "cbor2::de",
    "runtime::worker",
    "telemetry::export",
];

const WORDS: [&str; 12] = [
    "request",
    "completed",
    "timeout",
    "retry",
    "cache",
    "miss",
    "handshake",
    "expired",
    "queued",
    "flush",
    "deadline",
    "exceeded",
];

const LABELS: [&str; 8] = [
    "prod",
    "region=eu",
    "shard=3",
    "tls",
    "h2",
    "ipv6",
    "warm",
    "canary",
];

/// Builds a deterministic batch of `n` log records.
pub fn log_batch(n: usize) -> Vec<LogEntry> {
    let mut rng = SplitMix64::new(0xC0DE_CAFE);
    (0..n)
        .map(|i| {
            let r = rng.next_u64();
            let target = TARGETS[(r as usize) % TARGETS.len()].to_string();
            let word_count = 4 + (r >> 8) as usize % 6;
            let mut message = String::with_capacity(word_count * 8);
            for w in 0..word_count {
                if w > 0 {
                    message.push(' ');
                }
                message.push_str(WORDS[(rng.next_u64() as usize) % WORDS.len()]);
            }
            let label_count = (r >> 16) as usize % 4;
            let labels = (0..label_count)
                .map(|_| LABELS[(rng.next_u64() as usize) % LABELS.len()].to_string())
                .collect();
            LogEntry {
                timestamp: 1_700_000_000_000 + i as u64 * 37,
                level: (r >> 24) as u8 % 5,
                target,
                message,
                line: (r >> 32) as u32 % 4096,
                success: r & 1 == 0,
                latency_ms: (r >> 40) as f64 / 1024.0,
                labels,
            }
        })
        .collect()
}

/// The minicbor view of [`log_batch`].
pub fn log_batch_mini(batch: &[LogEntry]) -> Vec<LogEntryMini> {
    batch.iter().map(LogEntryMini::from).collect()
}

/// A flat array of `n` pseudo-random `u64`s spanning every header width.
pub fn int_array(n: usize) -> Vec<u64> {
    let mut rng = SplitMix64::new(0x1234_5678);
    (0..n)
        .map(|i| match i % 5 {
            // Spread across the CBOR integer header widths.
            0 => rng.next_u64() % 24,
            1 => rng.next_u64() % 256,
            2 => rng.next_u64() % 65_536,
            3 => rng.next_u64() % (1 << 32),
            _ => rng.next_u64(),
        })
        .collect()
}

/// A single byte string of `n` pseudo-random bytes (CBOR major type 2).
pub fn blob(n: usize) -> Vec<u8> {
    let mut rng = SplitMix64::new(0xDEAD_BEEF);
    let mut out = Vec::with_capacity(n);
    while out.len() < n {
        out.extend_from_slice(&rng.next_u64().to_le_bytes());
    }
    out.truncate(n);
    out
}

/// Workload sizes shared by every scenario, so the three benchmark binaries
/// report comparable numbers.
pub const LOG_BATCH_LEN: usize = 128;
pub const INT_ARRAY_LEN: usize = 1024;
pub const BLOB_LEN: usize = 4096;

/// Reused fixed-buffer capacity for the comparison fixtures.
pub const FIXED_CAPACITY: usize = 64 * 1024;

/// Ciborium exposes a writer API rather than a `to_vec` convenience function.
pub fn ciborium_to_vec<T: Serialize>(value: &T) -> Vec<u8> {
    let mut bytes = Vec::new();
    ciborium::into_writer(value, &mut bytes).unwrap();
    bytes
}

/// Each decoder reads its own crate's encoding. Preparation and correctness
/// assertions run outside the timed loops; the benchmark calls stay explicit.
pub struct Encoded {
    pub cbor2: Vec<u8>,
    pub ciborium: Vec<u8>,
    pub serde_cbor: Vec<u8>,
    pub cbor4ii: Vec<u8>,
    pub minicbor: Vec<u8>,
}

impl Encoded {
    pub fn new<T, M>(value: &T, mini: &M) -> Self
    where
        T: Serialize + serde::de::DeserializeOwned + PartialEq + std::fmt::Debug,
        M: minicbor::Encode<()> + for<'b> minicbor::Decode<'b, ()> + PartialEq + std::fmt::Debug,
    {
        let encoded = Self {
            cbor2: cbor2::to_vec(value).unwrap(),
            ciborium: ciborium_to_vec(value),
            serde_cbor: serde_cbor::to_vec(value).unwrap(),
            cbor4ii: cbor4ii::serde::to_vec(Vec::new(), value).unwrap(),
            minicbor: minicbor::to_vec(mini).unwrap(),
        };
        for bytes in [
            &encoded.cbor2,
            &encoded.ciborium,
            &encoded.serde_cbor,
            &encoded.cbor4ii,
            &encoded.minicbor,
        ] {
            cbor2::validate_slice(bytes).unwrap();
        }
        assert_eq!(&cbor2::from_slice::<T>(&encoded.cbor2).unwrap(), value);
        assert_eq!(
            &cbor2::from_reader::<T, _>(encoded.cbor2.as_slice()).unwrap(),
            value
        );
        assert_eq!(
            &ciborium::from_reader::<T, _>(encoded.ciborium.as_slice()).unwrap(),
            value
        );
        assert_eq!(
            &serde_cbor::from_slice::<T>(&encoded.serde_cbor).unwrap(),
            value
        );
        assert_eq!(
            &serde_cbor::from_reader::<T, _>(encoded.serde_cbor.as_slice()).unwrap(),
            value
        );
        assert_eq!(
            &cbor4ii::serde::from_slice::<T>(&encoded.cbor4ii).unwrap(),
            value
        );
        assert_eq!(
            &cbor4ii::serde::from_reader::<T, _>(encoded.cbor4ii.as_slice()).unwrap(),
            value
        );
        assert_eq!(&minicbor::decode::<M>(&encoded.minicbor).unwrap(), mini);

        let mut buffer = vec![0; FIXED_CAPACITY];
        assert_eq!(cbor2::to_slice(value, &mut buffer).unwrap(), encoded.cbor2);
        let mut slice = buffer.as_mut_slice();
        ciborium::into_writer(value, &mut slice).unwrap();
        let len = FIXED_CAPACITY - slice.len();
        assert_eq!(buffer[..len], encoded.ciborium);
        let mut serializer =
            serde_cbor::Serializer::new(serde_cbor::ser::SliceWrite::new(&mut buffer));
        value.serialize(&mut serializer).unwrap();
        let len = serializer.into_inner().bytes_written();
        assert_eq!(buffer[..len], encoded.serde_cbor);
        let mut slice = buffer.as_mut_slice();
        cbor4ii::serde::to_writer(&mut slice, value).unwrap();
        let len = FIXED_CAPACITY - slice.len();
        assert_eq!(buffer[..len], encoded.cbor4ii);
        let mut cursor = minicbor::encode::write::Cursor::new(buffer.as_mut_slice());
        minicbor::encode(mini, &mut cursor).unwrap();
        let len = cursor.position();
        assert_eq!(buffer[..len], encoded.minicbor);
        assert_eq!(
            cbor2::serialized_size(value).unwrap(),
            encoded.cbor2.len() as u64
        );
        encoded
    }

    /// Integer-array and blob fixtures have the same wire format in every crate.
    pub fn assert_identical(&self) {
        for bytes in [
            &self.ciborium,
            &self.serde_cbor,
            &self.cbor4ii,
            &self.minicbor,
        ] {
            assert_eq!(bytes, &self.cbor2);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixtures_round_trip_and_fixed_buffers_match() {
        let ints = int_array(INT_ARRAY_LEN);
        Encoded::new(&ints, &ints).assert_identical();
        let logs = log_batch(LOG_BATCH_LEN);
        Encoded::new(&logs, &log_batch_mini(&logs));
        let raw = blob(BLOB_LEN);
        Encoded::new(
            &serde_bytes::ByteBuf::from(raw.clone()),
            &minicbor::bytes::ByteVec::from(raw),
        )
        .assert_identical();
    }
}
