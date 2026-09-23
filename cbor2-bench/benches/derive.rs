//! Equal-wire serialization workloads for the derive marker and flatten paths.
use criterion::{criterion_group, criterion_main, Criterion};
use serde::{ser::SerializeMap, Serialize};
use std::{collections::BTreeMap, hint::black_box};

macro_rules! fixtures {
    ($keyed:ident, $flat:ident, $direct:ident; $($field:ident = $key:literal),+ $(,)?) => {
        #[derive(cbor2::Cbor)]
        struct $keyed { $(#[cbor(key = $key)] $field: u64,)+ }
        #[derive(cbor2::Cbor)]
        struct $flat {
            $(#[cbor(key = $key)] $field: u64,)+
            #[serde(flatten)] extra: BTreeMap<String, u64>,
        }
        struct $direct { $($field: u64,)+ }
        impl $keyed { fn sample() -> Self { Self { $($field: $key,)+ } } }
        impl $flat { fn sample() -> Self { Self { $($field: $key,)+ extra: BTreeMap::new() } } }
        impl $direct { fn sample() -> Self { Self { $($field: $key,)+ } } }
        impl Serialize for $direct {
            fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                let mut map = serializer.serialize_map(Some([$(stringify!($field)),+].len()))?;
                $(map.serialize_entry(&($key as u64), &self.$field)?;)+
                map.end()
            }
        }
    };
}

fixtures!(Keyed4, Flat4, Direct4; f0 = 0, f1 = 1, f2 = 2, f3 = 3);
fixtures!(Keyed16, Flat16, Direct16;
    f0 = 0, f1 = 1, f2 = 2, f3 = 3, f4 = 4, f5 = 5, f6 = 6, f7 = 7,
    f8 = 8, f9 = 9, f10 = 10, f11 = 11, f12 = 12, f13 = 13, f14 = 14, f15 = 15,
);

fn encode<T: Serialize>(c: &mut Criterion, name: &str, value: &T) {
    let mut output = Vec::with_capacity(1024);
    c.bench_function(name, |b| {
        b.iter(|| {
            output.clear();
            cbor2::to_writer(black_box(value), &mut output).unwrap();
            black_box(&output);
        });
    });
}

fn derive(c: &mut Criterion) {
    let keyed = Keyed4::sample();
    let flat = Flat4::sample();
    let direct = Direct4::sample();
    let expected = cbor2::to_vec(&direct).unwrap();
    assert_eq!(cbor2::to_vec(&keyed).unwrap(), expected);
    assert_eq!(cbor2::to_vec(&flat).unwrap(), expected);
    encode(c, "derive/keyed4", &keyed);
    encode(c, "derive/flatten4", &flat);
    encode(c, "derive/direct4", &direct);

    let keyed = Keyed16::sample();
    let flat = Flat16::sample();
    let direct = Direct16::sample();
    let expected = cbor2::to_vec(&direct).unwrap();
    assert_eq!(cbor2::to_vec(&keyed).unwrap(), expected);
    assert_eq!(cbor2::to_vec(&flat).unwrap(), expected);
    encode(c, "derive/keyed16", &keyed);
    encode(c, "derive/flatten16", &flat);
    encode(c, "derive/direct16", &direct);
}

criterion_group!(benches, derive);
criterion_main!(benches);
