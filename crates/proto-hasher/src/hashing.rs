use crate::errors::ProtoHasherError;
use crate::google_protobuf::{GoogleValue, is_google_proto_value_null};
use bitcoin::hashes::sha256::Hash as Sha256Hash;
use bitcoin::hashes::{Hash, HashEngine};
use prost_reflect::{DynamicMessage, FieldDescriptor, Kind, MapKey, MessageDescriptor, ReflectMessage, Value};
use std::collections::HashMap;

const BOOL_IDENTIFIER: &str = "b";
pub(crate) const MAP_IDENTIFIER: &str = "d";
const FLOAT_IDENTIFIER: &str = "f";
const INT_IDENTIFIER: &str = "i";
pub(crate) const LIST_IDENTIFIER: &str = "l";
const BYTE_IDENTIFIER: &str = "r";
const UNICODE_IDENTIFIER: &str = "u";

const NAN_BITS: u64 = 0x7FF8000000000001;

pub(crate) fn hash_fields_by_names(message: &DynamicMessage, names: &[&str]) -> Result<Sha256Hash, ProtoHasherError> {
    let mut hash_engine = Sha256Hash::engine();

    hash_engine.input(&LIST_IDENTIFIER.as_bytes());

    for name in names {
        let descriptor = message.descriptor();
        let fd: FieldDescriptor = descriptor
            .get_field_by_name(name)
            .ok_or(ProtoHasherError::MissingField(name.to_string()))?;

        let value = message.get_field(&fd);
        let value_hash = hash_value(&fd.kind(), value.as_ref())?;

        if let Some(hash) = value_hash {
            hash_engine.input(hash.as_byte_array());
        }
    }

    Ok(Sha256Hash::from_engine(hash_engine))
}

pub(crate) fn hash_value(kind: &Kind, value: &Value) -> Result<Option<Sha256Hash>, ProtoHasherError> {
    let h = match kind {
        Kind::Bool => {
            let v = value.as_bool().ok_or(ProtoHasherError::ValueTypeMismatch {
                expected: "bool",
                found: value_type_label(value),
            })?;
            Some(hash_bool(v))
        }
        Kind::Double => {
            let v = value.as_f64().ok_or(ProtoHasherError::ValueTypeMismatch {
                expected: "double",
                found: value_type_label(value),
            })?;
            Some(hash_f64(v))
        }
        Kind::Float => {
            let v = value.as_f64().ok_or(ProtoHasherError::ValueTypeMismatch {
                expected: "float",
                found: value_type_label(value),
            })?;
            Some(hash_f64(v))
        }
        Kind::Int32 | Kind::Sint32 | Kind::Sfixed32 => {
            let v = value.as_i32().ok_or(ProtoHasherError::ValueTypeMismatch {
                expected: "int32",
                found: value_type_label(value),
            })?;
            Some(hash_i32(v))
        }
        Kind::Int64 | Kind::Sint64 | Kind::Sfixed64 => {
            let v = value.as_i64().ok_or(ProtoHasherError::ValueTypeMismatch {
                expected: "int64",
                found: value_type_label(value),
            })?;
            Some(hash_i64(v))
        }
        Kind::Uint32 | Kind::Fixed32 => {
            let v = value.as_u32().ok_or(ProtoHasherError::ValueTypeMismatch {
                expected: "uint32",
                found: value_type_label(value),
            })?;
            Some(hash_u32(v))
        }
        Kind::Uint64 | Kind::Fixed64 => {
            let v = value.as_u64().ok_or(ProtoHasherError::ValueTypeMismatch {
                expected: "uint64",
                found: value_type_label(value),
            })?;
            Some(hash_u64(v))
        }
        Kind::String => {
            let v = value.as_str().ok_or(ProtoHasherError::ValueTypeMismatch {
                expected: "string",
                found: value_type_label(value),
            })?;
            Some(hash_string(v))
        }
        Kind::Bytes => {
            let v = value.as_bytes().ok_or(ProtoHasherError::ValueTypeMismatch {
                expected: "bytes",
                found: value_type_label(value),
            })?;
            Some(hash_bytes(v))
        }
        Kind::Message(_) => {
            let dm = value.as_message().ok_or(ProtoHasherError::ValueTypeMismatch {
                expected: "message",
                found: value_type_label(value),
            })?;
            hash_message(dm.to_owned())?
        }
        Kind::Enum(_) => {
            let n = value.as_enum_number().ok_or(ProtoHasherError::ValueTypeMismatch {
                expected: "enum",
                found: value_type_label(value),
            })? as i64;
            Some(hash_i64(n))
        }
    };
    Ok(h)
}

pub(crate) fn hash_bool(b: bool) -> Sha256Hash {
    let mut hash_engine = Sha256Hash::engine();

    hash_engine.input(&BOOL_IDENTIFIER.as_bytes());

    let bytes = if b { "1".as_bytes() } else { "0".as_bytes() };
    hash_engine.input(bytes);

    Sha256Hash::from_engine(hash_engine)
}

pub(crate) fn hash_i32(value: i32) -> Sha256Hash {
    hash_u64(value as u64)
}

pub(crate) fn hash_i64(value: i64) -> Sha256Hash {
    hash_u64(value as u64)
}

pub(crate) fn hash_u32(value: u32) -> Sha256Hash {
    hash_u64(value as u64)
}

pub(crate) fn hash_u64(value: u64) -> Sha256Hash {
    let mut hash_engine = Sha256Hash::engine();

    hash_engine.input(&INT_IDENTIFIER.as_bytes());
    hash_engine.input(&value.to_be_bytes());

    Sha256Hash::from_engine(hash_engine)
}

pub(crate) fn hash_f64(value: f64) -> Sha256Hash {
    let mut f = value;
    if f == 0.0 && f.is_sign_negative() {
        f = 0.0;
    }

    let bits: u64 = if f.is_nan() {
        NAN_BITS
    } else {
        // https://doc.rust-lang.org/std/primitive.f64.html#method.to_bits
        f.to_bits()
    };

    let mut hash_engine = Sha256Hash::engine();

    hash_engine.input(&FLOAT_IDENTIFIER.as_bytes());
    hash_engine.input(&bits.to_be_bytes());

    Sha256Hash::from_engine(hash_engine)
}

pub(crate) fn hash_string(value: &str) -> Sha256Hash {
    let mut hash_engine = Sha256Hash::engine();

    hash_engine.input(&UNICODE_IDENTIFIER.as_bytes());
    hash_engine.input(&value.as_bytes());

    Sha256Hash::from_engine(hash_engine)
}

pub(crate) fn hash_bytes(bytes: &[u8]) -> Sha256Hash {
    let mut hash_engine = Sha256Hash::engine();

    hash_engine.input(&BYTE_IDENTIFIER.as_bytes());
    hash_engine.input(&bytes);

    Sha256Hash::from_engine(hash_engine)
}

/// Hashes DynamicMessage
pub fn hash_message<M: Into<DynamicMessage>>(message: M) -> Result<Option<Sha256Hash>, ProtoHasherError> {
    let message = message.into();

    let descriptor = message.descriptor();

    if let Some(google_value) = GoogleValue::maybe_from_str(&descriptor.full_name()) {
        return google_value.hash(&message);
    };

    let mut field_hashes: Vec<FieldHashEntry> = hash_fields(message)?;

    field_hashes.sort_by(|a, b| a.number.cmp(&b.number));

    let mut hash_engine = Sha256Hash::engine();

    hash_engine.input(MAP_IDENTIFIER.as_bytes());
    for field_hash in field_hashes {
        hash_engine.input(field_hash.k_hash.as_byte_array());
        hash_engine.input(field_hash.v_hash.as_byte_array());
    }

    Ok(Some(Sha256Hash::from_engine(hash_engine)))
}

fn hash_fields(message: DynamicMessage) -> Result<Vec<FieldHashEntry>, ProtoHasherError> {
    let mut hashes = vec![];

    for (fd, value) in message.fields() {
        if !message.has_field(&fd) {
            continue;
        }

        if value.is_default(&fd.kind()) {
            continue;
        }

        let Some(hash_entry) = hash_field(&fd, value)? else {
            continue;
        };

        hashes.push(hash_entry);
    }

    Ok(hashes)
}

fn hash_field(fd: &FieldDescriptor, value: &Value) -> Result<Option<FieldHashEntry>, ProtoHasherError> {
    let k_hash = hash_field_key(fd);
    let Some(v_hash) = hash_field_value(fd, value)? else {
        return Ok(None);
    };

    Ok(Some(FieldHashEntry {
        number: fd.number(),
        k_hash,
        v_hash,
    }))
}

pub(crate) fn hash_field_key(fd: &FieldDescriptor) -> Sha256Hash {
    hash_u32(fd.number())
}

pub(crate) fn hash_field_value(fd: &FieldDescriptor, value: &Value) -> Result<Option<Sha256Hash>, ProtoHasherError> {
    if fd.is_list() {
        let list = value.as_list().ok_or(ProtoHasherError::ValueTypeMismatch {
            expected: "list",
            found: value_type_label(value),
        })?;
        return hash_list(&fd.kind(), list);
    }
    if fd.is_map() {
        let kind = fd.kind();

        let entry_md: &MessageDescriptor = kind.as_message().ok_or(ProtoHasherError::MapKindNotMessage)?;

        let key_fd = entry_md.map_entry_key_field();
        let value_fd = entry_md.map_entry_value_field();

        let map_value = value.as_map().ok_or(ProtoHasherError::ValueTypeMismatch {
            expected: "map",
            found: value_type_label(value),
        })?;
        return hash_map(&key_fd, &value_fd, map_value);
    }

    hash_value(&fd.kind(), &value)
}

pub(crate) fn hash_list(kind: &Kind, list: &[Value]) -> Result<Option<Sha256Hash>, ProtoHasherError> {
    let mut hash_engine = Sha256Hash::engine();

    if list.is_empty() {
        return Ok(None);
    }

    hash_engine.input(LIST_IDENTIFIER.as_bytes());

    for value in list {
        let hash = hash_value(kind, value)?;
        if let Some(hash) = hash {
            hash_engine.input(hash.as_byte_array());
        }
    }

    Ok(Some(Sha256Hash::from_engine(hash_engine)))
}

pub(crate) fn hash_map(
    key_fd: &FieldDescriptor,
    value_fd: &FieldDescriptor,
    map: &HashMap<MapKey, Value>,
) -> Result<Option<Sha256Hash>, ProtoHasherError> {
    let mut hash_entries = vec![];

    for (key, value) in map {
        if is_google_proto_value_null(value) {
            continue;
        }

        let Some(k_hash) = hash_field_value(key_fd, &key.to_owned().into())? else {
            return Ok(None);
        };
        let Some(v_hash) = hash_field_value(value_fd, value)? else {
            return Ok(None);
        };

        hash_entries.push((k_hash, v_hash));
    }

    if hash_entries.is_empty() {
        return Ok(None);
    }

    hash_entries.sort_by(|a, b| a.0.cmp(&b.0));

    let mut hash_engine = Sha256Hash::engine();

    hash_engine.input(&MAP_IDENTIFIER.as_bytes());

    for (k_hash, v_hash) in hash_entries {
        hash_engine.input(k_hash.as_byte_array());
        hash_engine.input(v_hash.as_byte_array());
    }

    Ok(Some(Sha256Hash::from_engine(hash_engine)))
}

struct FieldHashEntry {
    number: u32,
    k_hash: Sha256Hash,
    v_hash: Sha256Hash,
}

pub(crate) fn value_type_label(v: &Value) -> &'static str {
    match v {
        Value::Bool(_) => "bool",
        Value::I32(_) => "int32",
        Value::I64(_) => "int64",
        Value::U32(_) => "uint32",
        Value::U64(_) => "uint64",
        Value::F32(_) => "float",
        Value::F64(_) => "double",
        Value::String(_) => "string",
        Value::Bytes(_) => "bytes",
        Value::EnumNumber(_) => "enum",
        Value::Message(_) => "message",
        Value::List(_) => "list",
        Value::Map(_) => "map",
    }
}

pub(crate) fn key_type_label(k: &prost_reflect::MapKey) -> &'static str {
    use prost_reflect::MapKey::*;
    match k {
        Bool(_) => "bool",
        I32(_) => "int32",
        I64(_) => "int64",
        U32(_) => "uint32",
        U64(_) => "uint64",
        String(_) => "string",
    }
}
