use std::future::Future;
use std::task::{Context, Poll, Waker};

struct Reader<'a>(&'a [u8]);
impl cbor2::async_io::AsyncRead for Reader<'_> {
    async fn read_exact(&mut self, buf: &mut [u8]) -> Result<(), cbor2::io::Error> {
        std::io::Read::read_exact(&mut self.0, buf)
    }
}
fn ready<T>(future: impl Future<Output = T>) -> T {
    let mut cx = Context::from_waker(Waker::noop());
    match std::pin::pin!(future).as_mut().poll(&mut cx) {
        Poll::Ready(result) => result,
        Poll::Pending => panic!("in-memory read must be ready"),
    }
}

#[test]
fn crosscheck_validators_raw_and_async_on_small_and_random_inputs() {
    let check = |input: &[u8]| {
        let slice = cbor2::validate_slice(input).is_ok();
        let reader = cbor2::validate(input).is_ok();
        assert_eq!(slice, reader, "validators differ: {input:x?}");
        let mut source = Reader(input);
        let async_result = ready(cbor2::async_io::read_item_with_limit(&mut source, 256));
        let async_valid = async_result.is_ok() && source.0.is_empty();
        assert_eq!(slice, async_valid, "async differs: {input:x?}");
        if slice {
            let raw: cbor2::RawValue = cbor2::from_slice(input).unwrap();
            assert_eq!(raw.as_bytes(), input);
            assert_eq!(cbor2::to_vec(&raw).unwrap(), input);
        }
    };
    check(&[]);
    for a in 0..=255u8 {
        check(&[a]);
    }
    for a in 0..=255u8 {
        for b in 0..=255u8 {
            check(&[a, b]);
        }
    }
    let mut state = 0xDEADBEEF01234567u64;
    let mut next = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    for _ in 0..100_000 {
        let length = (next() % 64) as usize;
        let bytes: Vec<u8> = (0..length).map(|_| next() as u8).collect();
        check(&bytes);
    }
}
