fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut stream = Vec::new();

    for item in [
        cbor2::cbor!({ "type" => "start" })?,
        cbor2::cbor!({ "type" => "chunk", "n" => 1 })?,
        cbor2::cbor!({ "type" => "done" })?,
    ] {
        cbor2::to_writer(&item, &mut stream)?;
    }

    // An in-memory sequence decodes from the slice; use `from_reader` for
    // streams.
    let decoded: Vec<cbor2::Value> = cbor2::de::Deserializer::from_slice(&stream)
        .into_iter()
        .collect::<Result<_, _>>()?;

    assert_eq!(decoded.len(), 3);
    // The single-item validator rejects a sequence of three items.
    assert!(cbor2::validate_slice(&stream).is_err());

    for item in decoded {
        println!("{item}");
    }

    Ok(())
}
