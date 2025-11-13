//! Comprehensive tests mirroring the Go suite in protohash_test.go

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

fn mk_duration(seconds: i64, nanos: i32) -> DynamicMessage {
    let d = lookup_msg("google.protobuf.Duration");
    let mut m = DynamicMessage::new(d.clone());
    m.set_field(&d.get_field_by_name("seconds").unwrap(), Value::I64(seconds));
    m.set_field(&d.get_field_by_name("nanos").unwrap(), Value::I32(nanos));
    m
}

fn mk_timestamp(seconds: i64, nanos: i32) -> DynamicMessage {
    let d = lookup_msg("google.protobuf.Timestamp");
    let mut m = DynamicMessage::new(d.clone());
    m.set_field(&d.get_field_by_name("seconds").unwrap(), Value::I64(seconds));
    m.set_field(&d.get_field_by_name("nanos").unwrap(), Value::I32(nanos));
    m
}

fn mk_value_null() -> DynamicMessage {
    let d = lookup_msg("google.protobuf.Value");
    let mut m = DynamicMessage::new(d.clone());
    m.set_field(&d.get_field_by_name("null_value").unwrap(), Value::EnumNumber(0));
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

fn mk_value_number(v: f64) -> DynamicMessage {
    let d = lookup_msg("google.protobuf.Value");
    let mut m = DynamicMessage::new(d.clone());
    m.set_field(&d.get_field_by_name("number_value").unwrap(), Value::F64(v));
    m
}

fn mk_value_bool(b: bool) -> DynamicMessage {
    let d = lookup_msg("google.protobuf.Value");
    let mut m = DynamicMessage::new(d.clone());
    m.set_field(&d.get_field_by_name("bool_value").unwrap(), Value::Bool(b));
    m
}

fn mk_value_list_of_strings(items: &[&str]) -> DynamicMessage {
    let vd = lookup_msg("google.protobuf.Value");
    let lvd = lookup_msg("google.protobuf.ListValue");

    let mut list_msg = DynamicMessage::new(lvd.clone());
    {
        let values_fd = lvd.get_field_by_name("values").unwrap();
        let mut list = list_msg
            .get_field_mut(&values_fd)
            .as_list_mut()
            .expect("values is list");
        for s in items {
            list.push(Value::Message(mk_value_string(s)));
        }
    }

    let mut value_msg = DynamicMessage::new(vd.clone());
    value_msg.set_field(&vd.get_field_by_name("list_value").unwrap(), Value::Message(list_msg));
    value_msg
}

fn mk_struct_empty() -> DynamicMessage {
    let d = lookup_msg("google.protobuf.Struct");
    let mut m = DynamicMessage::new(d.clone());
    m.set_field(&d.get_field_by_name("fields").unwrap(), Value::Map(HashMap::new()));
    m
}

fn mk_struct_with_values(name: &str, age: f64) -> DynamicMessage {
    let d = lookup_msg("google.protobuf.Struct");
    let mut m = DynamicMessage::new(d.clone());
    let f_fields = d.get_field_by_name("fields").unwrap();

    let mut map = HashMap::new();
    map.insert(MapKey::String("name".into()), Value::Message(mk_value_string(name)));
    map.insert(MapKey::String("age".into()), Value::Message(mk_value_number(age)));

    m.set_field(&f_fields, Value::Map(map));
    m
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

#[test]
fn hash_well_known_types_duration() {
    let hasher = ProtoHasher::new();

    let msg1 = mk_duration(60, 0);
    let msg2 = mk_duration(3600, 0);
    let msg3 = mk_duration(60, 0);

    let h1 = hasher.hash_proto(msg1).unwrap();
    let h2 = hasher.hash_proto(msg2).unwrap();
    let h3 = hasher.hash_proto(msg3).unwrap();

    assert_eq!(h1, h3, "Same Duration should have same hash");
    assert_ne!(h1, h2, "Different Duration should have different hash");

    eprintln!("Duration h1: {h1}  h2: {h2}  h3: {h3}");
}

#[test]
fn hash_well_known_types_timestamp() {
    let hasher = ProtoHasher::new();

    let msg1 = mk_timestamp(1_234_567_890, 0);
    let msg2 = mk_timestamp(1_234_567_891, 0);
    let msg3 = mk_timestamp(1_234_567_890, 0);

    let h1 = hasher.hash_proto(msg1).unwrap();
    let h2 = hasher.hash_proto(msg2).unwrap();
    let h3 = hasher.hash_proto(msg3).unwrap();

    assert_eq!(h1, h3, "Same Timestamp should have same hash");
    assert_ne!(h1, h2, "Different Timestamp should have different hash");

    eprintln!("Timestamp h1: {h1}  h2: {h2}  h3: {h3}");
}

#[test]
fn hash_struct_value_empty() {
    let hasher = ProtoHasher::new();

    let msg = mk_struct_empty();
    let h = hasher.hash_proto(msg).expect("hash empty Struct");

    assert!(!h.to_string().is_empty(), "Expected non-empty hash for empty struct");
    eprintln!("Empty Struct: {h}");
}

#[test]
fn hash_struct_value_with_values() {
    let hasher = ProtoHasher::new();

    let msg1 = mk_struct_with_values("test", 30.0);
    let msg2 = mk_struct_with_values("test", 30.0);
    let msg3 = mk_struct_with_values("different", 30.0);

    let h1 = hasher.hash_proto(msg1).unwrap();
    let h2 = hasher.hash_proto(msg2).unwrap();
    let h3 = hasher.hash_proto(msg3).unwrap();

    assert_eq!(h1, h2, "Same Struct should have same hash");
    assert_ne!(h1, h3, "Different Struct should have different hash");

    eprintln!("Struct h1: {h1}  h2: {h2}  h3: {h3}");
}

#[test]
fn non_wkt_default_skipping() {
    let empty = mk_struct_empty();

    let with_nulls = mk_struct_with_map(&[("null_field", mk_value_null())]);

    let hasher = ProtoHasher::new();
    let h_empty = hasher.hash_proto(empty).unwrap();
    let h_nulls = hasher.hash_proto(with_nulls).unwrap();

    assert_eq!(h_empty, h_nulls, "null values should be skipped; hashes differ");
}

#[test]
fn non_wkt_field_sorting_by_number_or_key() {
    let struct1 = mk_struct_with_map(&[
        ("name", mk_value_string("test")),
        ("age", mk_value_number(30.0)),
        ("active", mk_value_bool(true)),
    ]);

    let struct2 = mk_struct_with_map(&[
        ("active", mk_value_bool(true)),
        ("name", mk_value_string("test")),
        ("age", mk_value_number(30.0)),
    ]);

    let hasher = ProtoHasher::new();
    let h1 = hasher.hash_proto(struct1).unwrap();
    let h2 = hasher.hash_proto(struct2).unwrap();

    assert_eq!(h1, h2, "field order should not affect hash; got different hashes");
}

#[test]
fn non_wkt_nested_messages() {
    let nested1 = mk_struct_with_map(&[(
        "user",
        mk_struct_with_map(&[("name", mk_value_string("alice")), ("id", mk_value_number(123.0))]),
    )]);

    let nested2 = mk_struct_with_map(&[(
        "user",
        mk_struct_with_map(&[("name", mk_value_string("alice")), ("id", mk_value_number(123.0))]),
    )]);

    let nested3 = mk_struct_with_map(&[(
        "user",
        mk_struct_with_map(&[("name", mk_value_string("bob")), ("id", mk_value_number(123.0))]),
    )]);

    let hasher = ProtoHasher::new();
    let h1 = hasher.hash_proto(nested1).unwrap();
    let h2 = hasher.hash_proto(nested2).unwrap();
    let h3 = hasher.hash_proto(nested3).unwrap();

    assert_eq!(h1, h2, "same nested messages should have same hash");
    assert_ne!(h1, h3, "different nested messages should have different hashes");
}

#[test]
fn non_wkt_list_handling() {
    let list1 = mk_struct_with_map(&[("items", mk_value_list_of_strings(&["a", "b", "c"]))]);
    let list2 = mk_struct_with_map(&[("items", mk_value_list_of_strings(&["a", "b", "c"]))]);

    let list3 = mk_struct_with_map(&[("items", mk_value_list_of_strings(&["c", "b", "a"]))]);

    let empty_list = mk_struct_with_map(&[("items", mk_value_list_of_strings(&[]))]);
    let no_list = mk_struct_empty();

    let hasher = ProtoHasher::new();
    let h1 = hasher.hash_proto(list1).unwrap();
    let h2 = hasher.hash_proto(list2).unwrap();
    let h3 = hasher.hash_proto(list3).unwrap();
    let h_empty = hasher.hash_proto(empty_list).unwrap();
    let h_no = hasher.hash_proto(no_list).unwrap();

    assert_eq!(h1, h2, "same lists should have same hash");
    assert_ne!(h1, h3, "different list order should produce different hashes");

    assert_eq!(h_empty, h_no, "empty list and no list should have same hash");
}

#[test]
fn non_wkt_value_types() {
    let string_val = mk_struct_with_map(&[("field", mk_value_string("test"))]);
    let number_val = mk_struct_with_map(&[("field", mk_value_number(42.0))]);
    let bool_val = mk_struct_with_map(&[("field", mk_value_bool(true))]);

    let hasher = ProtoHasher::new();
    let h_string = hasher.hash_proto(string_val).unwrap().to_string();
    let h_number = hasher.hash_proto(number_val).unwrap().to_string();
    let h_bool = hasher.hash_proto(bool_val).unwrap().to_string();

    let hashes = [("string", h_string), ("number", h_number), ("bool", h_bool)];

    for (i, (n1, h1)) in hashes.iter().enumerate() {
        for (j, (n2, h2)) in hashes.iter().enumerate() {
            if i != j {
                assert_ne!(
                    h1, h2,
                    "different value types {n1} and {n2} should have different hashes"
                );
            }
        }
    }
}
