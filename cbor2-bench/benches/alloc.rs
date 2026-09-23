//! Scenario: **`no_std` + `alloc`** — the in-memory heap-buffer API path.
//!
//! Encoding produces a fresh `Vec<u8>`; decoding reads back from a `&[u8]`.
//! All five crates support this API shape. Log records still use different
//! wire formats; see the README. This is the path used in a `no_std + alloc`
//! target such as a wasm32 canister or an `alloc`-only embedded runtime.

use std::hint::black_box;

use cbor2_bench::*;
use criterion::{criterion_group, criterion_main, Criterion};
use serde::{de::DeserializeOwned, Serialize};

fn encode<T: Serialize, M: minicbor::Encode<()>>(c: &mut Criterion, payload: &Payload<T, M>) {
    let Payload {
        name, value, mini, ..
    } = payload;
    let mut group = c.benchmark_group(format!("alloc/encode/{name}"));
    group.bench_function("cbor2", |b| {
        b.iter(|| cbor2::to_vec(black_box(value)).unwrap())
    });
    group.bench_function("ciborium", |b| b.iter(|| ciborium_to_vec(black_box(value))));
    group.bench_function("serde_cbor", |b| {
        b.iter(|| serde_cbor::to_vec(black_box(value)).unwrap())
    });
    group.bench_function("cbor4ii", |b| {
        b.iter(|| cbor4ii::serde::to_vec(Vec::new(), black_box(value)).unwrap())
    });
    group.bench_function("minicbor", |b| {
        b.iter(|| minicbor::to_vec(black_box(mini)).unwrap())
    });
    group.finish();
}

fn decode<T, M>(c: &mut Criterion, payload: &Payload<T, M>)
where
    T: DeserializeOwned,
    M: for<'b> minicbor::Decode<'b, ()>,
{
    let Payload { name, encoded, .. } = payload;
    let mut group = c.benchmark_group(format!("alloc/decode/{name}"));
    group.bench_function("cbor2", |b| {
        b.iter(|| cbor2::from_slice::<T>(black_box(&encoded.cbor2)).unwrap())
    });
    group.bench_function("ciborium", |b| {
        b.iter(|| ciborium::from_reader::<T, _>(black_box(encoded.ciborium.as_slice())).unwrap())
    });
    group.bench_function("serde_cbor", |b| {
        b.iter(|| serde_cbor::from_slice::<T>(black_box(&encoded.serde_cbor)).unwrap())
    });
    group.bench_function("cbor4ii", |b| {
        b.iter(|| cbor4ii::serde::from_slice::<T>(black_box(&encoded.cbor4ii)).unwrap())
    });
    group.bench_function("minicbor", |b| {
        b.iter(|| minicbor::decode::<M>(black_box(&encoded.minicbor)).unwrap())
    });
    group.finish();
}

fn scenario(c: &mut Criterion) {
    let Fixtures { ints, logs, blob } = Fixtures::prepare();
    encode(c, &ints);
    encode(c, &logs);
    encode(c, &blob);
    decode(c, &ints);
    decode(c, &logs);
    decode(c, &blob);
}

criterion_group!(benches, scenario);
criterion_main!(benches);
