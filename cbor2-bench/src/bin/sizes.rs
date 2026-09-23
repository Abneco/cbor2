//! Prints checked encoded sizes. Run with `cargo run --release --bin sizes`.
//! Log records use text-keyed maps in the serde crates and arrays in minicbor.

use cbor2_bench::Fixtures;

fn main() {
    let Fixtures { ints, logs, blob } = Fixtures::prepare();
    let (ints, logs, blob) = (ints.encoded, logs.encoded, blob.encoded);

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
