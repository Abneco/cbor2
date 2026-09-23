//! Prints checked encoded sizes. Run with `cargo run --release --bin sizes`.
//! Log records use text-keyed maps in the serde crates and arrays in minicbor.

use cbor2_bench::*;
use serde_bytes::ByteBuf;

fn main() {
    let ints = int_array(INT_ARRAY_LEN);
    let ints = Encoded::new(&ints, &ints);
    ints.assert_identical();
    let logs = log_batch(LOG_BATCH_LEN);
    let logs = Encoded::new(&logs, &log_batch_mini(&logs));
    let raw = blob(BLOB_LEN);
    let blob = Encoded::new(
        &ByteBuf::from(raw.clone()),
        &minicbor::bytes::ByteVec::from(raw),
    );
    blob.assert_identical();

    println!(
        "{:<14} {:>12} {:>12} {:>12}",
        "crate", "int_array", "log_batch", "blob"
    );
    println!("{}", "-".repeat(54));
    for (name, a, b, c) in [
        ("cbor2", &ints.cbor2, &logs.cbor2, &blob.cbor2),
        ("ciborium", &ints.ciborium, &logs.ciborium, &blob.ciborium),
        (
            "serde_cbor",
            &ints.serde_cbor,
            &logs.serde_cbor,
            &blob.serde_cbor,
        ),
        ("cbor4ii", &ints.cbor4ii, &logs.cbor4ii, &blob.cbor4ii),
        ("minicbor", &ints.minicbor, &logs.minicbor, &blob.minicbor),
    ] {
        println!("{name:<14} {:>12} {:>12} {:>12}", a.len(), b.len(), c.len());
    }
}
