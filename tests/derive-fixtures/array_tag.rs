#[derive(cbor2::Cbor)]
#[cbor(array)]
#[serde(tag = "kind")]
struct Message {
    value: u8,
}
fn main() {}
