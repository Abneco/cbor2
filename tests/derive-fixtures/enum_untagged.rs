#[derive(cbor2::Cbor)]
enum Message {
    Unit,
    #[serde(untagged)]
    Data {
        #[cbor(key = 1)]
        value: u8,
    },
}
fn main() {}
