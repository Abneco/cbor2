//! Tests for CBOR tag support.

use cbor2::tag::{AllowAny, AllowExact, RequireAny, RequireExact};
use cbor2::Value;

#[test]
fn require_exact() {
    // Tag 0: standard date/time string.
    let datetime = RequireExact::<String, 0>("2013-03-21T20:04:00Z".into());
    let bytes = cbor2::to_vec(&datetime).unwrap();
    assert_eq!(
        hex::encode(&bytes),
        "c074323031332d30332d32315432303a30343a30305a"
    );

    let back: RequireExact<String, 0> = cbor2::from_slice(&bytes).unwrap();
    assert_eq!(back, datetime);

    // The wrong tag is rejected...
    assert!(cbor2::from_slice::<RequireExact<String, 1>>(&bytes).is_err());
    // ...and so is a missing tag.
    let untagged = cbor2::to_vec(&"2013-03-21T20:04:00Z").unwrap();
    assert!(cbor2::from_slice::<RequireExact<String, 0>>(&untagged).is_err());
}

#[test]
fn allow_exact() {
    let bytes = cbor2::to_vec(&AllowExact::<u64, 1>(1363896240)).unwrap();
    assert_eq!(hex::encode(&bytes), "c11a514b67b0");

    // Tagged and untagged input both decode.
    let tagged: AllowExact<u64, 1> = cbor2::from_slice(&bytes).unwrap();
    assert_eq!(tagged.0, 1363896240);

    let untagged_bytes = cbor2::to_vec(&1363896240u64).unwrap();
    let untagged: AllowExact<u64, 1> = cbor2::from_slice(&untagged_bytes).unwrap();
    assert_eq!(untagged.0, 1363896240);

    // A different tag is rejected.
    assert!(cbor2::from_slice::<AllowExact<u64, 7>>(&bytes).is_err());
}

#[test]
fn require_any() {
    let bytes = cbor2::to_vec(&RequireAny(32, String::from("https://example.com"))).unwrap();
    let back: RequireAny<String> = cbor2::from_slice(&bytes).unwrap();
    assert_eq!(back.0, 32);
    assert_eq!(back.1, "https://example.com");

    let untagged = cbor2::to_vec(&"https://example.com").unwrap();
    assert!(cbor2::from_slice::<RequireAny<String>>(&untagged).is_err());
}

#[test]
fn allow_any() {
    let tagged = hex::decode("c11a514b67b0").unwrap();
    let back: AllowAny<u64> = cbor2::from_slice(&tagged).unwrap();
    assert_eq!(back, AllowAny(Some(1), 1363896240));

    let untagged = cbor2::to_vec(&1363896240u64).unwrap();
    let back: AllowAny<u64> = cbor2::from_slice(&untagged).unwrap();
    assert_eq!(back, AllowAny(None, 1363896240));

    // Round trip preserves the tag (or its absence).
    let bytes = cbor2::to_vec(&AllowAny(Some(1), 1363896240u64)).unwrap();
    assert_eq!(hex::encode(&bytes), "c11a514b67b0");
    let bytes = cbor2::to_vec(&AllowAny(None, 1363896240u64)).unwrap();
    assert_eq!(hex::encode(&bytes), "1a514b67b0");
}

#[test]
fn value_tags() {
    // Tags survive a round trip through Value.
    let value = Value::Tag(262, Box::new(Value::from("x")));
    let bytes = cbor2::to_vec(&value).unwrap();
    assert_eq!(hex::encode(&bytes), "d901066178");
    let back: Value = cbor2::from_slice(&bytes).unwrap();
    assert_eq!(back, value);

    // Nested tags survive too. (Note that a nested *bignum* tag would
    // collapse into an integer by design.)
    let nested = Value::Tag(1, Box::new(Value::Tag(0, Box::new(Value::from("x")))));
    let back: Value = cbor2::from_slice(&cbor2::to_vec(&nested).unwrap()).unwrap();
    assert_eq!(back, nested);

    // Value and the tag wrappers interconvert.
    let wrapped: RequireAny<String> = value.deserialized().unwrap();
    assert_eq!(wrapped, RequireAny(262, "x".into()));
    let again = Value::serialized(&wrapped).unwrap();
    assert_eq!(again, value);
}

#[test]
fn oversized_bignums_stay_tagged() {
    // A bignum wider than 128 bits cannot collapse into an integer, so it
    // survives as a tagged byte string.
    let mut payload = vec![1u8];
    payload.extend_from_slice(&[0u8; 16]);

    let value = Value::Tag(2, Box::new(Value::Bytes(payload)));
    let bytes = cbor2::to_vec(&value).unwrap();
    let back: Value = cbor2::from_slice(&bytes).unwrap();
    assert_eq!(back, value);

    // While a small one collapses.
    let small = Value::Tag(2, Box::new(Value::Bytes(vec![42])));
    let bytes = cbor2::to_vec(&small).unwrap();
    let back: Value = cbor2::from_slice(&bytes).unwrap();
    assert_eq!(back, Value::from(42));
}

// A synthetic deserializer that presents the internal tag-protocol enum in
// ways the CBOR deserializers never do: by variant index, by arbitrary name,
// with a non-identifier variant, or with tuple items of the wrong type. It
// exercises the protocol's own validation in the tag wrappers and in Value.
mod synthetic {
    use serde::de::value::{
        BoolDeserializer, Error as DeError, SeqDeserializer, StrDeserializer, U64Deserializer,
    };
    use serde::de::{self, Expected, IntoDeserializer};
    use serde::forward_to_deserialize_any;
    use serde::Deserialize;

    use cbor2::tag::AllowAny;
    use cbor2::Value;

    pub enum Variant {
        Index(u64),
        Name(&'static str),
        NotAnIdentifier,
    }

    // `I` is the type of the tuple items (the tag number, then the value).
    pub struct EnumDe<I> {
        pub variant: Variant,
        pub items: Vec<I>,
    }

    fn enum_de<I>(variant: Variant, items: Vec<I>) -> EnumDe<I> {
        EnumDe { variant, items }
    }

    impl<'de, I: IntoDeserializer<'de, DeError>> de::Deserializer<'de> for EnumDe<I> {
        type Error = DeError;

        // `Value::deserialize` calls `deserialize_any` and the tag wrappers
        // call `deserialize_enum`; both see the enum.
        fn deserialize_any<V: de::Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
            visitor.visit_enum(self)
        }

        forward_to_deserialize_any! {
            i8 i16 i32 i64 i128 u8 u16 u32 u64 u128
            bool f32 f64 char str string bytes byte_buf
            seq map struct tuple tuple_struct identifier ignored_any
            option unit unit_struct newtype_struct enum
        }
    }

    impl<'de, I: IntoDeserializer<'de, DeError>> de::EnumAccess<'de> for EnumDe<I> {
        type Error = DeError;
        type Variant = Self;

        fn variant_seed<V: de::DeserializeSeed<'de>>(
            self,
            seed: V,
        ) -> Result<(V::Value, Self::Variant), Self::Error> {
            let value = match self.variant {
                Variant::Index(x) => seed.deserialize(U64Deserializer::new(x))?,
                Variant::Name(x) => seed.deserialize(StrDeserializer::new(x))?,
                Variant::NotAnIdentifier => seed.deserialize(BoolDeserializer::new(true))?,
            };
            Ok((value, self))
        }
    }

    impl<'de, I: IntoDeserializer<'de, DeError>> de::VariantAccess<'de> for EnumDe<I> {
        type Error = DeError;

        fn unit_variant(self) -> Result<(), Self::Error> {
            Ok(())
        }

        fn newtype_variant_seed<U: de::DeserializeSeed<'de>>(
            self,
            seed: U,
        ) -> Result<U::Value, Self::Error> {
            let item = self.items.into_iter().next();
            seed.deserialize(
                item.ok_or_else(|| de::Error::custom("no payload"))?
                    .into_deserializer(),
            )
        }

        fn tuple_variant<V: de::Visitor<'de>>(
            self,
            _: usize,
            visitor: V,
        ) -> Result<V::Value, Self::Error> {
            // Exercise the visitor's self-description on the way through.
            assert!(!format!("{}", DisplayExpected(&visitor)).is_empty());
            visitor.visit_seq(SeqDeserializer::new(self.items.into_iter()))
        }

        fn struct_variant<V: de::Visitor<'de>>(
            self,
            _: &'static [&'static str],
            _: V,
        ) -> Result<V::Value, Self::Error> {
            Err(de::Error::custom("no structs here"))
        }
    }

    struct DisplayExpected<'a, T>(&'a T);

    impl<T: Expected> core::fmt::Display for DisplayExpected<'_, T> {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            self.0.fmt(f)
        }
    }

    fn allow_any<I: IntoDeserializer<'static, DeError>>(
        variant: Variant,
        items: Vec<I>,
    ) -> Result<AllowAny<u64>, DeError> {
        AllowAny::deserialize(enum_de(variant, items))
    }

    fn error<T>(result: Result<T, DeError>) -> String {
        result
            .err()
            .expect("synthetic input must be rejected")
            .to_string()
    }

    #[test]
    fn variant_indices_select_the_form() {
        assert_eq!(
            allow_any(Variant::Index(0), vec![5u64]).unwrap(),
            AllowAny(None, 5)
        );
        assert_eq!(
            allow_any(Variant::Index(1), vec![9u64, 5]).unwrap(),
            AllowAny(Some(9), 5)
        );

        let msg = error(allow_any(Variant::Index(2), Vec::<u64>::new()));
        assert!(msg.contains("variant index 0 or 1"), "{msg}");
    }

    #[test]
    fn variant_names_are_validated() {
        assert_eq!(
            allow_any(Variant::Name("@@UNTAGGED@@"), vec![5u64]).unwrap(),
            AllowAny(None, 5)
        );

        let msg = error(allow_any(Variant::Name("bogus"), Vec::<u64>::new()));
        assert!(msg.contains("unknown variant"), "{msg}");

        let msg = error(allow_any(Variant::NotAnIdentifier, Vec::<u64>::new()));
        assert!(msg.contains("a tag variant identifier"), "{msg}");
    }

    #[test]
    fn tagged_payload_lengths_are_validated() {
        let msg = error(allow_any(Variant::Index(1), Vec::<u64>::new()));
        assert!(msg.contains("invalid length 0"), "{msg}");
        assert!(msg.contains("a tag number and a value"), "{msg}");

        let msg = error(allow_any(Variant::Index(1), vec![9u64]));
        assert!(msg.contains("invalid length 1"), "{msg}");
    }

    #[test]
    fn tagged_payloads_with_wrong_types_are_rejected() {
        // The tag number arrives as a string: u64 parsing fails.
        let tagged = || enum_de(Variant::Name("@@TAGGED@@"), vec!["oops"]);
        assert!(AllowAny::<u64>::deserialize(tagged()).is_err());
        let msg = error(Value::deserialize(tagged()));
        assert!(msg.contains("invalid type"), "{msg}");
    }

    #[test]
    fn non_enum_input_is_rejected() {
        let err = AllowAny::<u64>::deserialize(BoolDeserializer::<DeError>::new(true));
        let msg = err.unwrap_err().to_string();
        assert!(msg.contains("a possibly tagged value"), "{msg}");
    }

    // Value's visitor also absorbs the tag protocol from a foreign
    // deserializer.
    #[test]
    fn value_accepts_the_tagged_variant() {
        let value = Value::deserialize(enum_de(Variant::Name("@@TAGGED@@"), vec![9u64, 5]));
        assert_eq!(value.unwrap(), Value::Tag(9, Box::new(Value::from(5))));
    }

    #[test]
    fn value_rejects_other_variants_and_short_payloads() {
        for (variant, items, expected) in [
            ("@@UNTAGGED@@", vec![], "expected tag"),
            ("@@TAGGED@@", vec![], "expected tag"),
            ("@@TAGGED@@", vec![9u64], "expected value"),
        ] {
            let msg = error(Value::deserialize(enum_de(Variant::Name(variant), items)));
            assert!(msg.contains(expected), "{variant}: {msg}");
        }

        // The variant name arrives as a bool: the String seed inside
        // Value's enum visitor fails.
        let bad_name = enum_de(Variant::NotAnIdentifier, Vec::<u64>::new());
        assert!(Value::deserialize(bad_name).is_err());
    }
}

#[test]
fn wrappers_reject_garbage_input() {
    use cbor2::tag::{AllowAny, AllowExact, RequireAny, RequireExact};

    // The Allow* wrappers accept untagged input by design; the Require*
    // wrappers do not.
    let untagged = [0x01u8];
    assert_eq!(
        cbor2::from_slice::<AllowAny<u8>>(&untagged).unwrap(),
        AllowAny(None, 1)
    );
    assert_eq!(
        cbor2::from_slice::<AllowExact<u8, 1>>(&untagged).unwrap(),
        AllowExact(1)
    );
    assert!(cbor2::from_slice::<RequireAny<u8>>(&untagged).is_err());
    assert!(cbor2::from_slice::<RequireExact<u8, 1>>(&untagged).is_err());

    // A tagged item whose payload fails to parse.
    let tagged_bool = [0xc1, 0xf5];
    assert!(cbor2::from_slice::<AllowAny<u8>>(&tagged_bool).is_err());
    assert!(cbor2::from_slice::<RequireExact<u8, 1>>(&tagged_bool).is_err());
}

#[test]
fn wrapper_payload_failures_propagate() {
    use cbor2::tag::{AllowAny, AllowExact, RequireAny, RequireExact};

    // The untagged payload fails to parse for each wrapper.
    let not_a_u8 = [0xf5u8]; // true
    assert!(cbor2::from_slice::<AllowAny<u8>>(&not_a_u8).is_err());
    assert!(cbor2::from_slice::<AllowExact<u8, 1>>(&not_a_u8).is_err());
    assert!(cbor2::from_slice::<RequireAny<u8>>(&not_a_u8).is_err());
    assert!(cbor2::from_slice::<RequireExact<u8, 1>>(&not_a_u8).is_err());
}

// A tag wrapper used as the tag-number field of another tag: the tag
// number serializer rejects it.
struct WrapperAsTagNumber;

impl serde::Serialize for WrapperAsTagNumber {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeTupleVariant;
        let mut acc = serializer.serialize_tuple_variant("@@TAG@@", 1, "@@TAGGED@@", 2)?;
        acc.serialize_field(&cbor2::tag::AllowAny(Some(1), 0u8))?;
        acc.serialize_field(&0u8)?;
        acc.end()
    }
}

#[test]
fn tag_protocol_rejects_wrapper_tag_numbers() {
    assert!(cbor2::to_vec(&WrapperAsTagNumber).is_err());
    assert!(Value::serialized(&WrapperAsTagNumber).is_err());
}
