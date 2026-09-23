//! Scenario: fixed-buffer encoding and structural scans without hot-path allocation.
//!
//! ## Encoding (all five crates)
//!
//! Every crate can serialize into a caller-provided fixed buffer with zero
//! allocation, through a different door:
//!
//! | crate          | no-alloc encode entry point                |
//! |----------------|--------------------------------------------|
//! | cbor2          | [`cbor2::to_slice`]                         |
//! | ciborium       | `into_writer` over `&mut [u8]`             |
//! | serde_cbor     | `Serializer` over `ser::SliceWrite`        |
//! | cbor4ii        | `to_writer` over `&mut [u8]` (needs std)   |
//! | minicbor       | `encode` over `encode::write::Cursor`      |
//!
//! The output buffer is allocated once during setup and reused; nothing on
//! the measured path touches the allocator.
//!
//! ## Reading (cbor2 and minicbor measured here)
//!
//! These scan groups compare cbor2's validators with minicbor's `skip`.
//! Both check text UTF-8; validation additionally enforces CBOR structure and
//! complete input consumption. Serde_cbor also supports no-alloc typed reads
//! and `IgnoredAny` via caller-provided scratch or mutable input, but is not
//! measured in these scan groups. See the README capability matrix.
//!
//! This is a host build with alloc enabled. In particular, minicbor's `skip`
//! uses its alloc-capable implementation (which has a different no-alloc
//! implementation); these definite-length fixtures do not require its stack.

use std::hint::black_box;

use cbor2_bench::*;
use criterion::{criterion_group, criterion_main, Criterion};
use serde::Serialize;

/// Reused scratch buffer, sized once to fit the largest fixture.
const CAP: usize = FIXED_CAPACITY;

fn encode<T: Serialize, M: minicbor::Encode<()>>(c: &mut Criterion, payload: &Payload<T, M>) {
    let Payload {
        name, value, mini, ..
    } = payload;
    let mut g = c.benchmark_group(format!("no_alloc/encode/{name}"));
    g.bench_function("cbor2", |b| {
        let mut buf = vec![0u8; CAP];
        b.iter(|| {
            black_box(cbor2::to_slice(black_box(value), &mut buf).unwrap());
        })
    });
    g.bench_function("ciborium", |b| {
        let mut buf = vec![0u8; CAP];
        b.iter(|| {
            let mut slice: &mut [u8] = &mut buf[..];
            ciborium::into_writer(black_box(value), &mut slice).unwrap();
            let len = CAP - slice.len();
            black_box(&buf[..len]);
        })
    });
    g.bench_function("serde_cbor", |b| {
        let mut buf = vec![0u8; CAP];
        b.iter(|| {
            let mut ser = serde_cbor::Serializer::new(serde_cbor::ser::SliceWrite::new(&mut buf));
            black_box(value).serialize(&mut ser).unwrap();
            let len = ser.into_inner().bytes_written();
            black_box(&buf[..len]);
        })
    });
    g.bench_function("cbor4ii", |b| {
        let mut buf = vec![0u8; CAP];
        b.iter(|| {
            // cbor4ii has no public no_std slice serializer, but its
            // `to_writer` over a `&mut [u8]` (std::io::Write) encodes
            // into the fixed buffer without allocating.
            let mut slice: &mut [u8] = &mut buf[..];
            cbor4ii::serde::to_writer(&mut slice, black_box(value)).unwrap();
            let len = CAP - slice.len();
            black_box(&buf[..len]);
        })
    });
    g.bench_function("minicbor", |b| {
        let mut buf = vec![0u8; CAP];
        b.iter(|| {
            let mut cur = minicbor::encode::write::Cursor::new(&mut buf[..]);
            minicbor::encode(black_box(mini), &mut cur).unwrap();
            let len = cur.position();
            black_box(&buf[..len]);
        })
    });
    g.finish();
}

/// No-alloc structural reads: prove well-formedness / skip one item without
/// building a value. These groups cover the cbor2 and minicbor primitives.
fn scan<T, M>(c: &mut Criterion, payload: &Payload<T, M>) {
    let Payload { name, encoded, .. } = payload;
    let (bytes, bytes_mini) = (&encoded.cbor2, &encoded.minicbor);
    let mut g = c.benchmark_group(format!("no_alloc/scan/{name}"));
    g.bench_function("cbor2 (validate)", |x| {
        x.iter(|| cbor2::validate(black_box(&bytes[..])).unwrap())
    });
    g.bench_function("cbor2 (validate_slice)", |x| {
        x.iter(|| cbor2::validate_slice(black_box(&bytes[..])).unwrap())
    });
    g.bench_function("minicbor (skip)", |x| {
        x.iter(|| {
            let mut d = minicbor::Decoder::new(black_box(bytes_mini));
            d.skip().unwrap()
        })
    });
    g.finish();
}

/// `cbor2::serialized_size` computes the exact encoded length with no output
/// buffer and no allocation. Minicbor offers separate `CborLen` / `len` APIs;
/// this group measures cbor2 only, across the three payloads.
fn bench_serialized_size(c: &mut Criterion, fixtures: &Fixtures) {
    let Fixtures { ints, logs, blob } = fixtures;
    let mut g = c.benchmark_group("no_alloc/serialized_size (cbor2)");
    g.bench_function("int_array", |x| {
        x.iter(|| cbor2::serialized_size(black_box(&ints.value)).unwrap())
    });
    g.bench_function("log_batch", |x| {
        x.iter(|| cbor2::serialized_size(black_box(&logs.value)).unwrap())
    });
    g.bench_function("blob", |x| {
        x.iter(|| cbor2::serialized_size(black_box(&blob.value)).unwrap())
    });
    g.finish();
}

fn scenario(c: &mut Criterion) {
    let fixtures = Fixtures::prepare();
    let Fixtures { ints, logs, blob } = &fixtures;
    encode(c, ints);
    encode(c, logs);
    encode(c, blob);
    scan(c, ints);
    scan(c, logs);
    scan(c, blob);
    bench_serialized_size(c, &fixtures);
}

criterion_group!(benches, scenario);
criterion_main!(benches);
