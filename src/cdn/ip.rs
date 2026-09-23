use alloc::vec::Vec;

use crate::de::Error;

use super::encode::{write_definite_bytes, write_uint};
use super::types::{Atom, Indicator};

pub(super) fn ip_atom(content: &str, tagged: bool, offset: usize) -> Result<Atom, Error> {
    let (addr, prefix) = match content.split_once('/') {
        Some((addr, prefix)) => {
            if prefix.is_empty()
                || (prefix.len() > 1 && prefix.starts_with('0'))
                || !prefix.bytes().all(|b| b.is_ascii_digit())
            {
                return Err(Error::Syntax(offset));
            }
            let prefix = prefix
                .parse::<u8>()
                .map_err(|_| Error::Syntax(offset + addr.len() + 1))?;
            (addr, Some(prefix))
        }
        None => (content, None),
    };

    let parsed = if addr.contains(':') {
        IpAddr::V6(parse_ipv6(addr, offset)?)
    } else {
        IpAddr::V4(parse_ipv4(addr, offset)?)
    };

    let (tag_number, max_prefix, bytes) = match parsed {
        IpAddr::V4(bytes) => (52, 32, bytes.to_vec()),
        IpAddr::V6(bytes) => (54, 128, bytes.to_vec()),
    };

    let mut out = Vec::new();
    if tagged {
        write_uint(&mut out, 6, tag_number, Indicator::None)
            .map_err(|msg| Error::semantic(offset, msg))?;
    }

    let Some(prefix) = prefix else {
        if !tagged {
            return Ok(Atom::Bytes(bytes));
        }
        write_definite_bytes(&mut out, &bytes, Indicator::None)?;
        return Ok(Atom::Raw(out));
    };

    if prefix > max_prefix {
        return Err(Error::semantic(offset, "IP prefix length is out of range"));
    }
    // RFC 9164 §4.2 requires the bits beyond the prefix length to be zero.
    // Rejecting nonzero host bits — instead of silently masking them off —
    // keeps the literal faithful to the encoded data.
    let mut prefix_bytes = mask_prefix(bytes.clone(), prefix);
    if bytes[..prefix_bytes.len()] != prefix_bytes[..]
        || bytes[prefix_bytes.len()..].iter().any(|&b| b != 0)
    {
        return Err(Error::semantic(
            offset,
            "IP prefix has nonzero bits beyond the prefix length",
        ));
    }
    while prefix_bytes.last() == Some(&0) {
        prefix_bytes.pop();
    }
    write_uint(&mut out, 4, 2, Indicator::None).map_err(|msg| Error::semantic(offset, msg))?;
    write_uint(&mut out, 0, u64::from(prefix), Indicator::None)
        .map_err(|msg| Error::semantic(offset, msg))?;
    write_definite_bytes(&mut out, &prefix_bytes, Indicator::None)?;
    Ok(Atom::Raw(out))
}

enum IpAddr {
    V4([u8; 4]),
    V6([u8; 16]),
}

pub(super) fn parse_ipv4(input: &str, offset: usize) -> Result<[u8; 4], Error> {
    input
        .parse::<core::net::Ipv4Addr>()
        .map(|addr| addr.octets())
        .map_err(|_| Error::Syntax(offset))
}

pub(super) fn parse_ipv6(input: &str, offset: usize) -> Result<[u8; 16], Error> {
    input
        .parse::<core::net::Ipv6Addr>()
        .map(|addr| addr.octets())
        .map_err(|_| Error::Syntax(offset))
}

fn mask_prefix(mut bytes: Vec<u8>, prefix: u8) -> Vec<u8> {
    let full = usize::from(prefix / 8);
    let rem = prefix % 8;
    if rem == 0 {
        bytes.truncate(full);
    } else {
        bytes.truncate(full + 1);
        let mask = 0xff << (8 - rem);
        if let Some(last) = bytes.last_mut() {
            *last &= mask;
        }
    }
    bytes
}
