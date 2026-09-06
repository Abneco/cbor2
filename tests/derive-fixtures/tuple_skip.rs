#[derive(cbor2::Cbor)]
struct Tuple(#[serde(skip_serializing)] u8, u8);
fn main() {}
