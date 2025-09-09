use std::{env, fs, path::PathBuf};

use hex::ToHex;
use lazy_static::lazy_static;
use prost_reflect::{DescriptorPool, DynamicMessage, MessageDescriptor};
use proto_hasher::ProtoHasher;
use serde::Deserialize;
use serde_json::Value as JsonValue;
use spark_protos::SPARK_FILE_DESCRIPTOR_SET;

fn get_spark_descriptor_set() -> DescriptorPool {
    let mut pool = DescriptorPool::global();
    pool.decode_file_descriptor_set(SPARK_FILE_DESCRIPTOR_SET).unwrap();

    pool
}

lazy_static! {
    static ref POOL: DescriptorPool = get_spark_descriptor_set();
    static ref SPARK_INVOICE_FIELDS_DESC: MessageDescriptor = lookup_msg(FULL_NAME);
    static ref JSON_PATH: PathBuf = resolve_json_path();
}

const FULL_NAME: &str = "spark.SparkInvoiceFields";
const JSON_FILE: &str = "cross_language_hash_cases_proto.json";

#[derive(Deserialize)]
struct CrossLangFile {
    #[allow(dead_code)]
    description: Option<String>,
    #[serde(rename = "testCases")]
    test_cases: Vec<CrossLangTestCase>,
}

#[derive(Deserialize)]
struct CrossLangTestCase {
    name: String,
    #[allow(dead_code)]
    description: Option<String>,
    #[serde(rename = "expectedHash")]
    expected_hash_hex: Option<String>,
    #[serde(rename = "sparkInvoiceFields")]
    spark_invoice_fields: JsonValue,
}

fn lookup_msg(desc_full_name: &str) -> MessageDescriptor {
    POOL.get_message_by_name(desc_full_name)
        .unwrap_or_else(|| panic!("descriptor not found in POOL: {desc_full_name}"))
}

fn resolve_json_path() -> PathBuf {
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let p = base
        .join("crates/generic/proto-hasher/tests")
        .join("data")
        .join(JSON_FILE);
    if p.exists() {
        return p;
    }

    let alt = env::current_dir().unwrap().join("tests/data").join(JSON_FILE);
    if alt.exists() {
        return alt;
    }
    panic!(
        "could not locate test JSON.\nTried:\n  {}\n  {}",
        p.display(),
        alt.display()
    );
}

#[test]
fn test_spark_invoice_fields_json_cases() {
    assert_eq!(SPARK_INVOICE_FIELDS_DESC.full_name(), FULL_NAME);

    let data = fs::read(&*JSON_PATH).unwrap_or_else(|e| panic!("read json cases {}: {e}", JSON_PATH.display()));

    let file: CrossLangFile = serde_json::from_slice(&data).unwrap_or_else(|e| panic!("unmarshal json: {e}"));

    let hasher = ProtoHasher::new();

    for tc in file.test_cases {
        let dm = DynamicMessage::deserialize(SPARK_INVOICE_FIELDS_DESC.clone(), &tc.spark_invoice_fields)
            .expect("failed to deserialize proto");

        let got = hasher
            .hash_proto(dm)
            .unwrap_or_else(|e| panic!("hash SparkInvoiceFields ({}): {e}", tc.name));

        let got_hex = got.encode_hex::<String>();

        let expected = tc.expected_hash_hex.unwrap_or_default().trim().to_string();

        if expected.is_empty() || expected.eq_ignore_ascii_case("TBD") {
            println!("COMPUTED_HASH {}: {}", tc.name, got_hex);
            continue;
        }

        assert!(
            expected.eq_ignore_ascii_case(&got_hex),
            "hash mismatch ({}): expected={} got={}",
            tc.name,
            expected,
            got_hex
        );
    }
}
