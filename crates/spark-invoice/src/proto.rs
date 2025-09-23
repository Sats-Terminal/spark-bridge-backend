//! Utility functions for proto encoding

use bitcoin::secp256k1;
use chrono::{DateTime, TimeZone, Utc};
use token_identifier::TokenIdentifierParseError;

/// Error types that can occur during protobuf-like decoding.
#[derive(Debug)]
pub enum DecodeError {
    /// Input ended unexpectedly.
    Truncated,
    /// Varint overflowed maximum length.
    Overflow,
    /// Encountered unsupported or invalid wire type.
    InvalidWireType,
    /// Failed to decode UTF-8 string.
    Utf8(std::string::FromUtf8Error),
    /// Failed to parse timestamp.
    InvalidTimestamp,
    /// Failed to parse token identifier.
    InvalidTokenIdentifier(TokenIdentifierParseError),
    /// Failed to parse an uuid value.
    InvalidUuidBytesLength,
    /// Failed to parse an u128 value.
    InvalidU128BytesLength,
    /// Failed to parse a public key.
    InvalidPublicKey(secp256k1::Error),
    /// Required field is missing.
    MissingField(String),
}

impl From<std::string::FromUtf8Error> for DecodeError {
    fn from(e: std::string::FromUtf8Error) -> Self {
        DecodeError::Utf8(e)
    }
}

impl From<TokenIdentifierParseError> for DecodeError {
    fn from(e: TokenIdentifierParseError) -> Self {
        DecodeError::InvalidTokenIdentifier(e)
    }
}

/// Write a u32 as a varint into buffer.
#[inline]
pub fn write_varint_u32(out: &mut Vec<u8>, mut v: u32) {
    while v > 0x7F {
        out.push(((v as u8) & 0x7F) | 0x80);
        v >>= 7;
    }
    out.push(v as u8);
}

/// Write a u64 as a varint into buffer.
#[inline]
pub fn write_varint_u64(out: &mut Vec<u8>, mut v: u64) {
    while v > 0x7F {
        out.push(((v as u8) & 0x7F) | 0x80);
        v >>= 7;
    }
    out.push(v as u8);
}

/// Write a u128 as a BE bytes into buffer.
#[inline]
pub fn write_u128_be_bytes(out: &mut Vec<u8>, v: u128) {
    let b: [u8; 16] = v.to_be_bytes();
    let bytes = if let Some(first) = b.iter().position(|&c| c != 0) {
        b[first..].to_vec()
    } else {
        vec![0]
    };

    write_len_prefixed_bytes(out, &bytes);
}

/// Write a i64 as a varint into buffer.
#[inline]
pub fn write_int64(out: &mut Vec<u8>, v: i64) {
    // int64 uses two’s complement varint (NOT zigzag). Cast to u64 preserves bit pattern.
    write_varint_u64(out, v as u64);
}

/// Write a i32 as a varint into buffer.
#[inline]
pub fn write_int32(out: &mut Vec<u8>, v: i32) {
    write_varint_u32(out, v as u32);
}

const TIMESTAMP_SECONDS_TAG: u8 = 0x08;
const TIMESTAMP_NANOS_TAG: u8 = 0x10;

/// Encode a protobuf timestamp (seconds, nanos).
#[inline]
pub fn write_timestamp(out: &mut Vec<u8>, seconds: i64, nanos: i32) {
    if seconds != 0 {
        out.push(TIMESTAMP_SECONDS_TAG);
        write_int64(out, seconds);
    }

    if nanos != 0 {
        out.push(TIMESTAMP_NANOS_TAG);
        write_int32(out, nanos);
    }
}

/// Write length-prefixed bytes.
#[inline]
pub fn write_len_prefixed_bytes(out: &mut Vec<u8>, bytes: &[u8]) {
    write_varint_u32(out, bytes.len() as u32);
    out.extend_from_slice(bytes);
}

/// Write length-prefixed UTF-8 string.
#[inline]
pub fn write_len_prefixed_str(out: &mut Vec<u8>, s: &str) {
    write_len_prefixed_bytes(out, s.as_bytes());
}

/// Ensure buffer has enough remaining bytes.
#[inline]
fn ensure(buf: &[u8], pos: usize, need: usize) -> Result<(), DecodeError> {
    if pos + need > buf.len() {
        Err(DecodeError::Truncated)
    } else {
        Ok(())
    }
}

/// Read a single byte from buffer.
#[inline]
pub fn read_u8(buf: &[u8], pos: &mut usize) -> Result<u8, DecodeError> {
    ensure(buf, *pos, 1)?;
    let b = buf[*pos];
    *pos += 1;
    Ok(b)
}

/// Read a u32 varint from buffer.
#[inline]
pub fn read_varint_u32(buf: &[u8], pos: &mut usize) -> Result<u32, DecodeError> {
    let mut x: u32 = 0;
    let mut s = 0u32;
    for _ in 0..5 {
        let b = read_u8(buf, pos)?;
        x |= u32::from(b & 0x7F) << s;
        if (b & 0x80) == 0 {
            return Ok(x);
        }
        s += 7;
    }
    Err(DecodeError::Overflow)
}

/// Read a u64 varint from buffer.
#[inline]
pub fn read_varint_u64(buf: &[u8], pos: &mut usize) -> Result<u64, DecodeError> {
    let mut x: u64 = 0;
    let mut s = 0u32;
    for _ in 0..10 {
        let b = read_u8(buf, pos)?;
        x |= u64::from(b & 0x7F) << s;
        if (b & 0x80) == 0 {
            return Ok(x);
        }
        s += 7;
    }
    Err(DecodeError::Overflow)
}

/// Read length prefix as usize from buffer.
#[inline]
pub fn read_len(buf: &[u8], pos: &mut usize) -> Result<usize, DecodeError> {
    let l = read_varint_u32(buf, pos)? as usize;
    ensure(buf, *pos, l)?;
    Ok(l)
}

/// Read a slice of bytes of given length.
#[inline]
pub fn read_bytes<'a>(buf: &'a [u8], pos: &mut usize, len: usize) -> Result<&'a [u8], DecodeError> {
    ensure(buf, *pos, len)?;
    let s = &buf[*pos..*pos + len];
    *pos += len;
    Ok(s)
}

/// Read a UTF-8 string of given length.
#[inline]
pub fn read_string(buf: &[u8], pos: &mut usize, len: usize) -> Result<String, DecodeError> {
    let b = read_bytes(buf, pos, len)?.to_vec();
    Ok(String::from_utf8(b)?)
}

/// Skip field of given wire type in buffer.
pub fn skip_field(wire_type: u32, buf: &[u8], pos: &mut usize) -> Result<(), DecodeError> {
    match wire_type {
        0 => {
            read_varint_u64(buf, pos)?;
            Ok(())
        } // varint
        1 => {
            ensure(buf, *pos, 8)?;
            *pos += 8;
            Ok(())
        } // 64-bit
        2 => {
            let l = read_len(buf, pos)?;
            *pos += l;
            Ok(())
        } // length-delimited
        3 | 4 => Err(DecodeError::InvalidWireType), // groups unused
        5 => {
            ensure(buf, *pos, 4)?;
            *pos += 4;
            Ok(())
        } // 32-bit
        _ => Err(DecodeError::InvalidWireType),
    }
}

/// Protobuf timestamp with seconds and nanos.
#[derive(Debug, Clone, Copy, Default)]
pub struct Timestamp {
    seconds: i64,
    nanos: i32,
}

/// Decode a protobuf timestamp from bytes.
pub fn decode_timestamp(input: &[u8]) -> Result<Timestamp, DecodeError> {
    let mut pos = 0usize;
    let end = input.len();
    let mut ts = Timestamp::default();

    while pos < end {
        let tag = read_varint_u32(input, &mut pos)?;
        if tag == 0 {
            break;
        }
        let field = tag >> 3;
        let wt = tag & 7;

        match field {
            1 => {
                if wt != 0 {
                    return Err(DecodeError::InvalidWireType);
                }
                let v = read_varint_u64(input, &mut pos)?;
                ts.seconds = v as i64;
            }
            2 => {
                if wt != 0 {
                    return Err(DecodeError::InvalidWireType);
                }
                ts.nanos = i32::try_from(read_varint_u32(input, &mut pos)?).unwrap_or(i32::MAX);
            }
            _ => skip_field(wt, input, &mut pos)?,
        }
    }
    Ok(ts)
}

/// Convert a Timestamp into chrono::DateTime<Utc>.
pub fn timestamp_to_datetime(ts: Timestamp) -> Result<DateTime<Utc>, DecodeError> {
    Utc.timestamp_opt(ts.seconds, ts.nanos.clamp(0, 999_999_999) as u32)
        .single()
        .ok_or(DecodeError::InvalidTimestamp)
}
