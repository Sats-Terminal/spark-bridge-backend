//! Tests for null/Any error handling and top-level scalar/value type errors.

use std::collections::HashMap;

use lazy_static::lazy_static;
use prost_reflect::{DescriptorPool, DynamicMessage, MapKey, MessageDescriptor, Value};

use proto_hasher::ProtoHasher;

lazy_static! {
    static ref POOL: DescriptorPool = DescriptorPool::global();
}

fn lookup_msg(full_name: &str) -> MessageDescriptor {
    POOL.get_message_by_name(full_name)
        .unwrap_or_else(|| panic!("descriptor not found: {full_name}"))
}

fn mk_value_null() -> DynamicMessage {
    let d = lookup_msg("google.protobuf.Value");
    let mut m = DynamicMessage::new(d.clone());
    m.set_field(&d.get_field_by_name("null_value").unwrap(), Value::EnumNumber(0)); // NullValue = 0
    m
}

fn mk_value_empty() -> DynamicMessage {
    let d = lookup_msg("google.protobuf.Value");
    DynamicMessage::new(d)
}

fn mk_list_value_with(values: Vec<DynamicMessage>) -> DynamicMessage {
    let lvd = lookup_msg("google.protobuf.ListValue");
    let mut list_msg = DynamicMessage::new(lvd.clone());
    let values_fd = lvd.get_field_by_name("values").unwrap();
    {
        let list = list_msg
            .get_field_mut(&values_fd)
            .as_list_mut()
            .expect("values is list");
        for v in values {
            list.push(Value::Message(v));
        }
    }

    let vd = lookup_msg("google.protobuf.Value");
    let mut outer = DynamicMessage::new(vd.clone());
    outer.set_field(&vd.get_field_by_name("list_value").unwrap(), Value::Message(list_msg));
    outer
}

fn mk_struct_with_map(entries: &[(&str, DynamicMessage)]) -> DynamicMessage {
    let d = lookup_msg("google.protobuf.Struct");
    let mut m = DynamicMessage::new(d.clone());
    let f_fields = d.get_field_by_name("fields").unwrap();
    let mut map = HashMap::new();
    for (k, v) in entries {
        map.insert(MapKey::String((*k).to_owned()), Value::Message(v.clone()));
    }
    m.set_field(&f_fields, Value::Map(map));
    m
}

fn mk_struct_empty() -> DynamicMessage {
    let d = lookup_msg("google.protobuf.Struct");
    let mut m = DynamicMessage::new(d.clone());
    m.set_field(&d.get_field_by_name("fields").unwrap(), Value::Map(HashMap::new()));
    m
}

fn mk_any_with_type_url(url: &str) -> DynamicMessage {
    let d = lookup_msg("google.protobuf.Any");
    let mut m = DynamicMessage::new(d.clone());
    m.set_field(
        &d.get_field_by_name("type_url").unwrap(),
        Value::String(url.to_string()),
    );
    m
}

fn mk_wrapper_bool(v: bool) -> DynamicMessage {
    let d = lookup_msg("google.protobuf.BoolValue");
    let mut m = DynamicMessage::new(d.clone());
    m.set_field(&d.get_field_by_name("value").unwrap(), Value::Bool(v));
    m
}

fn mk_wrapper_int64(v: i64) -> DynamicMessage {
    let d = lookup_msg("google.protobuf.Int64Value");
    let mut m = DynamicMessage::new(d.clone());
    m.set_field(&d.get_field_by_name("value").unwrap(), Value::I64(v));
    m
}

fn mk_wrapper_double(v: f64) -> DynamicMessage {
    let d = lookup_msg("google.protobuf.DoubleValue");
    let mut m = DynamicMessage::new(d.clone());
    m.set_field(&d.get_field_by_name("value").unwrap(), Value::F64(v));
    m
}

fn mk_value_bool(b: bool) -> DynamicMessage {
    let d = lookup_msg("google.protobuf.Value");
    let mut m = DynamicMessage::new(d.clone());
    m.set_field(&d.get_field_by_name("bool_value").unwrap(), Value::Bool(b));
    m
}

fn mk_value_number(v: f64) -> DynamicMessage {
    let d = lookup_msg("google.protobuf.Value");
    let mut m = DynamicMessage::new(d.clone());
    m.set_field(&d.get_field_by_name("number_value").unwrap(), Value::F64(v));
    m
}

fn mk_value_string(s: &str) -> DynamicMessage {
    let d = lookup_msg("google.protobuf.Value");
    let mut m = DynamicMessage::new(d.clone());
    m.set_field(
        &d.get_field_by_name("string_value").unwrap(),
        Value::String(s.to_owned()),
    );
    m
}

#[test]
fn null_and_any_error_cases() {
    let hasher = ProtoHasher::new();

    {
        let v = mk_value_null();
        let err = hasher.hash_proto(v).expect_err("expected error hashing null Value");
        let es = err.to_string();
        assert!(
            es.contains("cannot hash nil value") || es.contains("top-level scalar/value types are not hashable"),
            "unexpected error: {es}"
        );
    }

    {
        let v = mk_value_empty();
        let err = hasher.hash_proto(v).expect_err("expected error hashing empty Value");
        let es = err.to_string();
        assert!(
            es.contains("invalid struct value") || es.contains("top-level scalar/value types are not hashable"),
            "unexpected error: {es}"
        );
    }

    {
        let lv = mk_list_value_with(vec![mk_value_null()]);
        let err = hasher
            .hash_proto(lv)
            .expect_err("expected error hashing ListValue containing null");
        let es = err.to_string();

        assert!(
            es.contains("hashing list item 0") || es.contains("top-level scalar/value types are not hashable"),
            "unexpected error: {es}"
        );
    }

    {
        let st = mk_struct_with_map(&[("k", mk_value_null())]);
        let empty = mk_struct_empty();

        let h1 = hasher
            .hash_proto(st)
            .unwrap_or_else(|e| panic!("unexpected error hashing Struct with null field: {e}"));
        let h2 = hasher
            .hash_proto(empty)
            .unwrap_or_else(|e| panic!("unexpected error hashing empty Struct: {e}"));
        assert_eq!(h1, h2, "Struct with null field should hash equal to empty struct");
    }

    {
        let any = mk_any_with_type_url("type.googleapis.com/example.Message");
        let err = hasher.hash_proto(any).expect_err("expected error hashing Any");
        let es = err.to_string();
        assert!(
            es.contains("does not support hashing of type: \"type.googleapis.com/example.Message\""),
            "unexpected error: {es}"
        );
    }
}

#[test]
fn top_level_scalar_or_value_types_error() {
    let hasher = ProtoHasher::new();

    {
        let v = mk_value_bool(false);
        assert!(
            hasher.hash_proto(v).is_err(),
            "expected error hashing top-level structpb.Value(false)"
        );
    }

    {
        let v = mk_value_number(0.0);
        assert!(
            hasher.hash_proto(v).is_err(),
            "expected error hashing top-level structpb.Value(0)"
        );
    }

    {
        let v = mk_value_string("");
        assert!(
            hasher.hash_proto(v).is_err(),
            r#"expected error hashing top-level structpb.Value("")"#
        );
    }

    {
        let w = mk_wrapper_bool(false);
        assert!(
            hasher.hash_proto(w).is_err(),
            "expected error hashing top-level BoolValue(false)"
        );
    }

    {
        let w = mk_wrapper_int64(0);
        assert!(
            hasher.hash_proto(w).is_err(),
            "expected error hashing top-level Int64Value(0)"
        );
    }

    {
        let w = mk_wrapper_double(0.0);
        assert!(
            hasher.hash_proto(w).is_err(),
            "expected error hashing top-level DoubleValue(0)"
        );
    }
}
