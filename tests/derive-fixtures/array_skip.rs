#[derive(cbor2::Cbor)]
#[cbor(array)]
struct Positional {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    a: Option<u8>,
    b: u8,
}
fn main() {}
