//! Scenario: **`std`** — streaming through `std::io` reader/writer traits.
//!
//! This is the path you take with files and sockets. Encoding writes into a
//! *reused* buffer through an `io::Write`, modeling a server that keeps one
//! scratch buffer per connection (so, unlike the `alloc` scenario, the
//! allocator is not on the hot path). Decoding goes through an `io::Read`,
//! whose copying source is the distinctive `std` cost versus a borrowed
//! slice.
//!
//! Ciborium uses its reader API in every scenario. Minicbor has no `io::Read`
//! decoder, so its decode measurements use its slice API here too.

use std::hint::black_box;

use cbor2_bench::*;
use criterion::{criterion_group, criterion_main, Criterion};
use serde::{de::DeserializeOwned, Serialize};

fn encode<T: Serialize, M: minicbor::Encode<()>>(c: &mut Criterion, payload: &Payload<T, M>) {
    let Payload {
        name, value, mini, ..
    } = payload;
    let mut g = c.benchmark_group(format!("std/encode/{name}"));
    g.bench_function("cbor2", |b| {
        let mut buf = Vec::new();
        b.iter(|| {
            buf.clear();
            cbor2::to_writer(black_box(value), &mut buf).unwrap();
            black_box(buf.as_slice());
        })
    });
    g.bench_function("ciborium", |b| {
        let mut buf = Vec::new();
        b.iter(|| {
            buf.clear();
            ciborium::into_writer(black_box(value), &mut buf).unwrap();
            black_box(buf.as_slice());
        })
    });
    g.bench_function("serde_cbor", |b| {
        let mut buf = Vec::new();
        b.iter(|| {
            buf.clear();
            serde_cbor::to_writer(&mut buf, black_box(value)).unwrap();
            black_box(buf.as_slice());
        })
    });
    g.bench_function("cbor4ii", |b| {
        let mut buf = Vec::new();
        b.iter(|| {
            buf.clear();
            cbor4ii::serde::to_writer(&mut buf, black_box(value)).unwrap();
            black_box(buf.as_slice());
        })
    });
    g.bench_function("minicbor", |b| {
        let mut buf = Vec::new();
        b.iter(|| {
            buf.clear();
            minicbor::encode(black_box(mini), &mut buf).unwrap();
            black_box(buf.as_slice());
        })
    });
    g.finish();
}

fn decode<T, M>(c: &mut Criterion, payload: &Payload<T, M>)
where
    T: DeserializeOwned,
    M: for<'b> minicbor::Decode<'b, ()>,
{
    let Payload { name, encoded, .. } = payload;
    let mut g = c.benchmark_group(format!("std/decode/{name}"));
    g.bench_function("cbor2", |x| {
        x.iter(|| cbor2::from_reader::<T, _>(black_box(&encoded.cbor2[..])).unwrap())
    });
    g.bench_function("ciborium", |x| {
        x.iter(|| ciborium::from_reader::<T, _>(black_box(&encoded.ciborium[..])).unwrap())
    });
    g.bench_function("serde_cbor", |x| {
        x.iter(|| serde_cbor::from_reader::<T, _>(black_box(&encoded.serde_cbor[..])).unwrap())
    });
    g.bench_function("cbor4ii", |x| {
        x.iter(|| cbor4ii::serde::from_reader::<T, _>(black_box(&encoded.cbor4ii[..])).unwrap())
    });
    g.bench_function("minicbor", |x| {
        x.iter(|| minicbor::decode::<M>(black_box(&encoded.minicbor)).unwrap())
    });
    g.finish();
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
