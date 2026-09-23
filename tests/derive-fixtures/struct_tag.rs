#[derive(cbor2::Cbor)]
#[serde(tag = "kind")]
struct Message {
    #[cbor(key = 1)]
    value: u8,
}
fn main() {}
