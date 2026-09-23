#[derive(cbor2::Cbor)]
enum Message {
    A {
        #[cbor(key = 1)]
        value: u8,
    },
    B {
        value: u8,
    },
}
fn main() {}
