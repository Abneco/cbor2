//! Workloads for raw items, canonical maps, diagnostic input, flattening and
//! segmented text. Compare the same fixture and wire shape across revisions.
use cbor2::{Cbor, RawValue, Value};
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::{collections::BTreeMap, hint::black_box};

#[derive(Cbor)]
struct OpenRecord {
    #[cbor(key = 1)]
    name: String,
    body: RawValue,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

fn focused(c: &mut Criterion) {
    let payload = vec![0x55; 1024 * 1024];
    let raw = RawValue::serialized(serde_bytes::Bytes::new(&payload)).unwrap();
    let mut raw_group = c.benchmark_group("focused/raw");
    raw_group.throughput(Throughput::Bytes(raw.as_bytes().len() as u64));
    raw_group.bench_function("encode_slice_1MiB", |b| {
        let mut out = vec![0; raw.as_bytes().len()];
        b.iter(|| {
            let bytes = cbor2::to_slice(black_box(&raw), &mut out).unwrap();
            black_box(bytes);
        });
    });
    raw_group.bench_function("decode_1MiB", |b| {
        b.iter(|| cbor2::from_slice::<RawValue>(black_box(raw.as_bytes())).unwrap())
    });
    raw_group.bench_function("size_1MiB", |b| {
        b.iter(|| cbor2::serialized_size(black_box(&raw)).unwrap())
    });
    raw_group.finish();

    let mut canonical = c.benchmark_group("focused/canonical");
    for n in [16, 256, 1024] {
        let map = Value::Map(
            (0..n)
                .rev()
                .map(|i| (Value::from(i as u64), Value::from(i as u64)))
                .collect(),
        );
        canonical.bench_with_input(BenchmarkId::new("integer_keys", n), &map, |b, value| {
            b.iter(|| cbor2::to_canonical_vec(black_box(value)).unwrap())
        });
    }
    let composite = Value::Map(
        (0..128u64)
            .rev()
            .map(|i| {
                (
                    Value::Array(vec![i.into(), "long compound key".repeat(8).into()]),
                    i.into(),
                )
            })
            .collect(),
    );
    canonical.bench_function("compound_keys", |b| {
        b.iter(|| cbor2::to_canonical_vec(black_box(&composite)).unwrap())
    });
    canonical.finish();

    let mut cdn = c.benchmark_group("focused/cdn");
    for n in [2000, 8000, 16000] {
        for (kind, text) in [
            ("hex", format!("0x{}", "f".repeat(n))),
            ("decimal", "9".repeat(n)),
        ] {
            cdn.throughput(Throughput::Bytes(text.len() as u64));
            cdn.bench_with_input(BenchmarkId::new(kind, n), &text, |b, text| {
                b.iter(|| cbor2::cdn_to_vec(black_box(text)).unwrap())
            });
        }
    }
    cdn.finish();

    let record = OpenRecord {
        name: "record".into(),
        body: RawValue::serialized(serde_bytes::Bytes::new(&payload[..4096])).unwrap(),
        extra: [("count".into(), 7.into())].into(),
    };
    let bytes = cbor2::to_vec(&record).unwrap();
    let mut flatten = c.benchmark_group("focused/flatten");
    flatten.bench_function("encode", |b| {
        b.iter(|| cbor2::to_vec(black_box(&record)).unwrap())
    });
    flatten.bench_function("decode", |b| {
        b.iter(|| cbor2::from_slice::<OpenRecord>(black_box(&bytes)).unwrap())
    });
    flatten.finish();

    let text = format!("ilts<<{}>>", vec!["\"chunk\""; 1024].join(","));
    let bytes = cbor2::cdn_to_vec(&text).unwrap();
    let mut strings = c.benchmark_group("focused/text");
    strings.throughput(Throughput::Bytes(bytes.len() as u64));
    strings.bench_function("slice_1024_chunks", |b| {
        b.iter(|| cbor2::from_slice::<String>(black_box(&bytes)).unwrap())
    });
    strings.bench_function("reader_1024_chunks", |b| {
        b.iter(|| cbor2::from_reader::<String, _>(black_box(bytes.as_slice())).unwrap())
    });
    strings.finish();

    let integers: Vec<_> = (0..1024u64).map(|n| n * 65537).collect();
    let bytes = cbor2::to_vec(&integers).unwrap();
    c.bench_function("focused/integer_decode", |b| {
        b.iter(|| cbor2::from_slice::<Vec<u64>>(black_box(&bytes)).unwrap())
    });
}

criterion_group!(benches, focused);
criterion_main!(benches);
