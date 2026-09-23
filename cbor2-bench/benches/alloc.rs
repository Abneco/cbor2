//! Scenario: **`no_std` + `alloc`** — the in-memory heap-buffer API path.
//!
//! Encoding produces a fresh `Vec<u8>`; decoding reads back from a `&[u8]`.
//! All five crates support this API shape. Log records still use different
//! wire formats; see the README. This is the path used in a `no_std + alloc`
//! target such as a wasm32 canister or an `alloc`-only embedded runtime.

use std::hint::black_box;

use cbor2_bench::*;
use criterion::{criterion_group, criterion_main, Criterion};
use serde_bytes::ByteBuf;

fn bench_encode(c: &mut Criterion) {
    macro_rules! encode_group {
        ($name:literal, $identical:literal, $serde:expr, $mini:expr) => {{
            let data = $serde;
            let mini = $mini;
            let encoded = Encoded::new(&data, &mini);
            if $identical {
                encoded.assert_identical();
            }
            let mut group = c.benchmark_group($name);
            group.bench_function("cbor2", |b| {
                b.iter(|| cbor2::to_vec(black_box(&data)).unwrap())
            });
            group.bench_function("ciborium", |b| b.iter(|| ciborium_to_vec(black_box(&data))));
            group.bench_function("serde_cbor", |b| {
                b.iter(|| serde_cbor::to_vec(black_box(&data)).unwrap())
            });
            group.bench_function("cbor4ii", |b| {
                b.iter(|| cbor4ii::serde::to_vec(Vec::new(), black_box(&data)).unwrap())
            });
            group.bench_function("minicbor", |b| {
                b.iter(|| minicbor::to_vec(black_box(&mini)).unwrap())
            });
            group.finish();
        }};
    }
    let logs = log_batch(LOG_BATCH_LEN);
    let mini = log_batch_mini(&logs);
    let raw = blob(BLOB_LEN);
    encode_group!(
        "alloc/encode/int_array",
        true,
        int_array(INT_ARRAY_LEN),
        int_array(INT_ARRAY_LEN)
    );
    encode_group!("alloc/encode/log_batch", false, logs, mini);
    encode_group!(
        "alloc/encode/blob",
        true,
        ByteBuf::from(raw.clone()),
        minicbor::bytes::ByteVec::from(raw)
    );
}

fn bench_decode(c: &mut Criterion) {
    macro_rules! decode_group {
        ($name:literal, $identical:literal, $ty:ty, $mty:ty, $serde:expr, $mini:expr) => {{
            let encoded = Encoded::new(&$serde, &$mini);
            if $identical {
                encoded.assert_identical();
            }
            let mut group = c.benchmark_group($name);
            group.bench_function("cbor2", |b| {
                b.iter(|| cbor2::from_slice::<$ty>(black_box(&encoded.cbor2)).unwrap())
            });
            group.bench_function("ciborium", |b| {
                b.iter(|| {
                    ciborium::from_reader::<$ty, _>(black_box(encoded.ciborium.as_slice())).unwrap()
                })
            });
            group.bench_function("serde_cbor", |b| {
                b.iter(|| serde_cbor::from_slice::<$ty>(black_box(&encoded.serde_cbor)).unwrap())
            });
            group.bench_function("cbor4ii", |b| {
                b.iter(|| cbor4ii::serde::from_slice::<$ty>(black_box(&encoded.cbor4ii)).unwrap())
            });
            group.bench_function("minicbor", |b| {
                b.iter(|| minicbor::decode::<$mty>(black_box(&encoded.minicbor)).unwrap())
            });
            group.finish();
        }};
    }
    let logs = log_batch(LOG_BATCH_LEN);
    let mini = log_batch_mini(&logs);
    let raw = blob(BLOB_LEN);
    decode_group!(
        "alloc/decode/int_array",
        true,
        Vec<u64>,
        Vec<u64>,
        int_array(INT_ARRAY_LEN),
        int_array(INT_ARRAY_LEN)
    );
    decode_group!(
        "alloc/decode/log_batch",
        false,
        Vec<LogEntry>,
        Vec<LogEntryMini>,
        logs,
        mini
    );
    decode_group!(
        "alloc/decode/blob",
        true,
        ByteBuf,
        minicbor::bytes::ByteVec,
        ByteBuf::from(raw.clone()),
        minicbor::bytes::ByteVec::from(raw)
    );
}

criterion_group!(benches, bench_encode, bench_decode);
criterion_main!(benches);
