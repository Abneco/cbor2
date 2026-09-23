use cbor2::__serde as serde;
use std::{borrow::Cow, marker::PhantomData};

#[derive(Default)]
struct NotSerde;

#[derive(cbor2::Cbor)]
struct FieldDefault<T> {
    #[serde(default)]
    value: T,
}

#[derive(cbor2::Cbor)]
struct Skipped<T> {
    #[serde(skip)]
    value: T,
}

#[derive(cbor2::Cbor)]
struct Marker<T> {
    value: u8,
    #[serde(skip)]
    marker: PhantomData<T>,
}

#[derive(cbor2::Cbor)]
struct Owned<'a> {
    name: Cow<'a, str>,
    #[serde(skip)]
    marker: PhantomData<&'a ()>,
}

#[derive(cbor2::Cbor)]
struct Borrowed<'a> {
    text: &'a str,
    optional: Option<&'a str>,
    #[serde(borrow)]
    cow: Cow<'a, str>,
}

trait Config {
    type Value;
}
impl Config for NotSerde {
    type Value = u8;
}

#[derive(cbor2::Cbor)]
struct Associated<T: Config> {
    value: T::Value,
}

#[derive(cbor2::Cbor)]
#[serde(bound = "")]
struct FieldBound<T: Config> {
    #[serde(bound(
        serialize = "T::Value: serde::Serialize",
        deserialize = "T::Value: serde::Deserialize<'de>"
    ))]
    value: T::Value,
}

#[derive(cbor2::Cbor)]
enum VariantBound<T: Config> {
    #[serde(bound(
        serialize = "T::Value: serde::Serialize",
        deserialize = "T::Value: serde::Deserialize<'de>"
    ))]
    Value(T::Value),
}

#[derive(cbor2::Cbor)]
struct Node<T> {
    value: T,
    next: Option<Box<Self>>,
}

#[derive(cbor2::Cbor)]
#[serde(default)]
struct Defaults<T> {
    #[serde(skip)]
    value: T,
}
impl<T: Default> Default for Defaults<T> {
    fn default() -> Self {
        Self {
            value: T::default(),
        }
    }
}

#[derive(cbor2::Cbor)]
struct AsText<T> {
    #[serde(
        serialize_with = "write_text",
        deserialize_with = "read_text",
        bound(
            serialize = "T: std::fmt::Display",
            deserialize = "T: std::str::FromStr, T::Err: std::fmt::Display"
        )
    )]
    value: T,
}
fn write_text<T: std::fmt::Display, S: serde::Serializer>(v: &T, s: S) -> Result<S::Ok, S::Error> {
    s.collect_str(v)
}
fn read_text<'de, T: std::str::FromStr, D: serde::Deserializer<'de>>(d: D) -> Result<T, D::Error>
where
    T::Err: std::fmt::Display,
{
    let s: String = serde::Deserialize::deserialize(d)?;
    s.parse().map_err(serde::de::Error::custom)
}
struct Number(u8);
impl std::fmt::Display for Number {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
impl std::str::FromStr for Number {
    type Err = std::num::ParseIntError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.parse().map(Self)
    }
}

fn roundtrip<T: serde::Serialize + serde::de::DeserializeOwned>(v: &T) -> T {
    let bytes = cbor2::to_vec(v).unwrap();
    cbor2::from_reader(bytes.as_slice()).unwrap()
}

fn main() {
    assert_eq!(
        cbor2::from_slice::<FieldDefault<u8>>(&[0xa0])
            .unwrap()
            .value,
        0
    );
    let _: NotSerde = roundtrip(&Skipped { value: NotSerde }).value;
    let marker = roundtrip(&Marker {
        value: 7,
        marker: PhantomData::<NotSerde>,
    });
    assert_eq!(marker.value, 7);
    let owned: Owned<'static> = roundtrip(&Owned {
        name: Cow::Owned("name".into()),
        marker: PhantomData,
    });
    assert!(matches!(owned.name, Cow::Owned(_)));
    let bytes = cbor2::to_vec(&Borrowed {
        text: "one",
        optional: Some("two"),
        cow: Cow::Borrowed("three"),
    })
    .unwrap();
    let borrowed: Borrowed<'_> = cbor2::from_slice(&bytes).unwrap();
    assert_eq!(borrowed.text, "one");
    assert_eq!(borrowed.optional, Some("two"));
    assert!(matches!(borrowed.cow, Cow::Borrowed("three")));
    assert_eq!(roundtrip(&Associated::<NotSerde> { value: 4 }).value, 4);
    assert_eq!(roundtrip(&FieldBound::<NotSerde> { value: 5 }).value, 5);
    let VariantBound::Value(value) = roundtrip(&VariantBound::<NotSerde>::Value(6));
    assert_eq!(value, 6);
    let node = Node {
        value: 1u8,
        next: Some(Box::new(Node {
            value: 2,
            next: None,
        })),
    };
    assert_eq!(roundtrip(&node).next.unwrap().value, 2);
    let _: NotSerde = roundtrip(&Defaults { value: NotSerde }).value;
    assert_eq!(roundtrip(&AsText { value: Number(42) }).value.0, 42);
}
