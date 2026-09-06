use std::collections::BTreeMap;

#[derive(Debug, cbor2::Cbor)]
struct Node {
    #[cbor(key = 1)]
    value: u8,
    next: Option<Box<Self>>,
}

#[derive(Debug, cbor2::Cbor)]
#[serde(default)]
struct Defaults<T> {
    value: T,
}
impl<T: From<u8>> Default for Defaults<T> {
    fn default() -> Self {
        Self { value: 37.into() }
    }
}

#[derive(Debug, cbor2::Cbor)]
#[serde(default = "Self::initial")]
struct Factory {
    value: u8,
}
impl Factory {
    fn initial() -> Self {
        Self { value: 42 }
    }
}

#[derive(Debug, cbor2::Cbor)]
struct Borrowed<'a> {
    #[cbor(key = 1)]
    #[serde(borrow)]
    name: &'a str,
    #[cbor(key = 2)]
    body: cbor2::RawValue,
    #[serde(flatten)]
    extra: BTreeMap<String, cbor2::Value>,
}

#[derive(Default, cbor2::Cbor)]
#[serde(default)]
struct LifetimePrefix<'__de_> {
    value: Option<&'__de_ str>,
}

fn main() {
    let node = Node {
        value: 1,
        next: Some(Box::new(Node {
            value: 2,
            next: None,
        })),
    };
    let bytes = cbor2::to_vec(&node).unwrap();
    assert_eq!(
        cbor2::from_slice::<Node>(&bytes)
            .unwrap()
            .next
            .unwrap()
            .value,
        2
    );
    assert_eq!(
        cbor2::from_slice::<Defaults<u8>>(&[0xa0]).unwrap().value,
        37
    );
    assert_eq!(cbor2::from_slice::<Factory>(&[0xa0]).unwrap().value, 42);
    let bytes = [0xa3, 1, 0x61, b'x', 2, 0x18, 1, 0x61, b'z', 3];
    let value: Borrowed<'_> = cbor2::from_slice(&bytes).unwrap();
    assert_eq!(value.name.as_ptr(), bytes[3..].as_ptr());
    assert_eq!(value.body.as_bytes(), [0x18, 1]);
    assert_eq!(cbor2::to_vec(&value).unwrap(), bytes);
    let value: LifetimePrefix<'_> = cbor2::from_slice(&[0xa0]).unwrap();
    assert_eq!(value.value, None);
}
