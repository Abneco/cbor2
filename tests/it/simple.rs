//! Tests for generic CBOR simple values (RFC 8949 §3.3).

use std::collections::BTreeMap;

use cbor2::core::{Encoder, Header};
use cbor2::{cbor, Simple, Value};

#[test]
fn simple_wrapper_preserves_wire_value() {
    let simple = Simple::new(59).unwrap();
    let bytes = cbor2::to_vec(&simple).unwrap();

    assert_eq!(hex::encode(&bytes), "f83b");
    assert_eq!(cbor2::from_slice::<Simple>(&bytes).unwrap(), simple);
    assert_eq!(
        cbor2::from_slice::<Value>(&bytes).unwrap(),
        Value::Simple(simple)
    );
    assert_eq!(cbor2::diagnostic(&bytes[..]).unwrap(), "simple(59)");
}

#[test]
fn one_byte_simple_values_roundtrip() {
    let simple = Simple::new(16).unwrap();
    let bytes = cbor2::to_vec(&simple).unwrap();

    assert_eq!(hex::encode(&bytes), "f0");
    assert_eq!(cbor2::from_slice::<Simple>(&bytes).unwrap(), simple);
    assert_eq!(
        cbor2::from_slice::<Value>(&bytes).unwrap(),
        Value::Simple(simple)
    );
}

#[test]
fn explicit_simple_can_capture_builtin_values() {
    assert_eq!(cbor2::from_slice::<Simple>(&[0xf4]).unwrap(), Simple::FALSE);
    assert_eq!(cbor2::from_slice::<Simple>(&[0xf5]).unwrap(), Simple::TRUE);
    assert_eq!(cbor2::from_slice::<Simple>(&[0xf6]).unwrap(), Simple::NULL);
    assert_eq!(
        cbor2::from_slice::<Simple>(&[0xf7]).unwrap(),
        Simple::UNDEFINED
    );

    // The default dynamic model keeps existing serde-compatible behavior for
    // the built-ins.
    assert_eq!(
        cbor2::from_slice::<Value>(&[0xf4]).unwrap(),
        Value::Bool(false)
    );
    assert_eq!(
        cbor2::from_slice::<Value>(&[0xf5]).unwrap(),
        Value::Bool(true)
    );
    assert_eq!(cbor2::from_slice::<Value>(&[0xf6]).unwrap(), Value::Null);
    assert_eq!(cbor2::from_slice::<Value>(&[0xf7]).unwrap(), Value::Null);
}

#[test]
fn simple_values_can_be_map_keys() {
    let redacted_claim_keys = Simple::new(59).unwrap();
    let value = Value::Map(vec![(
        Value::Simple(redacted_claim_keys),
        Value::Array(vec![Value::Bytes(vec![0xde, 0xad, 0xbe, 0xef])]),
    )]);

    let bytes = cbor2::to_vec(&value).unwrap();
    assert_eq!(hex::encode(&bytes), "a1f83b8144deadbeef");
    assert_eq!(cbor2::from_slice::<Value>(&bytes).unwrap(), value);

    let via_macro = cbor!({
        (Simple::new(59).unwrap()) => [Value::Bytes(vec![0xde, 0xad, 0xbe, 0xef])],
    })
    .unwrap();
    assert_eq!(via_macro, value);
}

#[test]
fn typed_maps_can_use_simple_keys() {
    let mut map = BTreeMap::new();
    map.insert(Simple::new(59).unwrap(), 1u8);

    let bytes = cbor2::to_vec(&map).unwrap();
    assert_eq!(hex::encode(&bytes), "a1f83b01");

    let back: BTreeMap<Simple, u8> = cbor2::from_slice(&bytes).unwrap();
    assert_eq!(back, map);
}

#[test]
fn value_bridge_preserves_simple_values() {
    let simple = Simple::new(59).unwrap();
    let value = Value::serialized(&simple).unwrap();

    assert_eq!(value, Value::Simple(simple));
    assert_eq!(value.deserialized::<Simple>().unwrap(), simple);
    assert_eq!(value.to_string(), "simple(59)");
    assert_eq!(format!("{value:?}"), "simple(59)");
}

#[test]
fn value_bridge_decodes_builtin_simple_values() {
    assert_eq!(
        Value::Bool(false).deserialized::<Simple>().unwrap(),
        Simple::FALSE
    );
    assert_eq!(
        Value::Bool(true).deserialized::<Simple>().unwrap(),
        Simple::TRUE
    );
    assert_eq!(Value::Null.deserialized::<Simple>().unwrap(), Simple::NULL);
    assert_eq!(
        Value::Simple(Simple::UNDEFINED)
            .deserialized::<Simple>()
            .unwrap(),
        Simple::UNDEFINED
    );
}

#[test]
fn builtin_simple_typed_conversions_match_wire() {
    fn check<T: serde::de::DeserializeOwned + PartialEq + std::fmt::Debug>(value: &Value) {
        let bytes = cbor2::to_vec(value).unwrap();
        match (value.deserialized::<T>(), cbor2::from_slice::<T>(&bytes)) {
            (Ok(value), Ok(wire)) => assert_eq!(value, wire),
            (Err(_), Err(_)) => {}
            (value, wire) => panic!("conversion mismatch: Value={value:?}, wire={wire:?}"),
        }
    }

    #[derive(Debug, PartialEq, serde::Deserialize)]
    struct Unit;
    #[derive(Debug, PartialEq, serde::Deserialize)]
    enum Variant {
        Unit,
    }

    for simple in [
        Simple::FALSE,
        Simple::TRUE,
        Simple::NULL,
        Simple::UNDEFINED,
        Simple::new(59).unwrap(),
    ] {
        let plain = Value::serialized(&simple).unwrap();
        for value in [plain.clone(), Value::Tag(7, Box::new(plain))] {
            check::<bool>(&value);
            check::<()>(&value);
            check::<Unit>(&value);
            check::<Option<bool>>(&value);
            check::<Option<()>>(&value);
            check::<Option<u64>>(&value);
            check::<Option<Simple>>(&value);
            check::<Simple>(&value);
            // The generic representation continues to preserve simple values.
            assert_eq!(value.deserialized::<Value>().unwrap(), value);
            check::<Variant>(&Value::Map(vec![("Unit".into(), value)]));
        }
    }
}

#[test]
fn canonical_encoding_sorts_simple_keys_by_encoded_bytes() {
    let mut value = Value::Map(vec![
        (Value::Simple(Simple::new(59).unwrap()), Value::Null),
        (Value::Simple(Simple::new(16).unwrap()), Value::Null),
        (Value::from(0), Value::Null),
    ]);

    value.canonicalize().unwrap();
    assert_eq!(
        value
            .as_map()
            .unwrap()
            .iter()
            .map(|(k, _)| k)
            .collect::<Vec<_>>(),
        vec![
            &Value::from(0),
            &Value::Simple(Simple::new(16).unwrap()),
            &Value::Simple(Simple::new(59).unwrap()),
        ]
    );
    assert_eq!(
        hex::encode(cbor2::to_canonical_vec(&value).unwrap()),
        "a300f6f0f6f83bf6"
    );
}

#[test]
fn reserved_simple_values_are_rejected() {
    assert!(Simple::new(24).is_none());
    assert_eq!(Simple::try_from(31).unwrap_err().value(), 31);

    let mut bytes = Vec::new();
    assert!(Encoder::from(&mut bytes).push(Header::Simple(24)).is_err());
    assert!(bytes.is_empty());

    assert!(cbor2::validate(&[0xf8, 0x18][..]).is_err());
    assert!(cbor2::from_slice::<Simple>(&[0xf8, 0x18]).is_err());
}
