//! Basic tests

use prost_reflect::{DescriptorPool, DynamicMessage, Value};
use proto_hasher::hash_message;

mod cross_language_test_cases;

const EXPECTED_HASH: &str = "5d707f14e44e14b64ea5890fea0968270f51f6562d631daad4131e03ced0a09b";

#[test]
fn test_basic() -> Result<(), Box<dyn std::error::Error>> {
    let pool = DescriptorPool::global();
    let value_md = pool
        .get_message_by_name("google.protobuf.Value")
        .ok_or("missing google.protobuf.Value in pool")?;

    let mut dm = DynamicMessage::new(value_md.clone());
    let f = value_md.get_field_by_name("string_value").unwrap();
    dm.set_field(&f, Value::String("hello world".into()));

    let hash_1 = hash_message(dm.clone())?;
    let hash_2 = hash_message(dm)?;

    assert_eq!(hash_1, hash_2);
    assert_eq!(hash_1.unwrap().to_string(), EXPECTED_HASH);

    Ok(())
}
