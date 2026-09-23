#![no_std]

extern crate alloc;
use alloc::{borrow::Cow, string::String};

#[derive(cbor2::Cbor)]
pub struct Message<'a, T> {
    #[cbor(key = 1)]
    #[serde(default)]
    pub value: T,
    pub name: Cow<'a, str>,
}

pub fn roundtrip() -> Message<'static, u8> {
    let value = Message {
        value: 7,
        name: Cow::Owned(String::from("name")),
    };
    let bytes = cbor2::to_vec(&value).unwrap();
    cbor2::from_reader(bytes.as_slice()).unwrap()
}
