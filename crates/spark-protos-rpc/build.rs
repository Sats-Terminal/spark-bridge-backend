/*!
 * This build script generates the Rust code for the Spark Operator RPC protocol buffers.
 * It uses the `tonic-build` crate to generate the Rust code from the protocol buffer definitions.
 * The generated code is placed in the `src/generated` directory.
 */

use std::{env, fs::read, path::PathBuf};

use prost_wkt_build::Message as _;
use tonic_build::FileDescriptorSet;

#[expect(clippy::expect_used, reason = "This is a build script")]
fn main() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    let spark_descriptor_file = out_dir.join("spark_descriptor.bin");
    tonic_build::configure()
        .build_server(false)
        .client_mod_attribute(".", "#[cfg(feature = \"client\")]")
        .type_attribute(".", "#[derive(serde::Serialize, serde::Deserialize)]")
        .type_attribute(".", "#[serde(rename_all = \"camelCase\")]")
        .extern_path(".google.protobuf.Timestamp", "::prost_wkt_types::Timestamp")
        .extern_path(".google.protobuf.Any", "::prost_wkt_types::Any")
        .extern_path(".google.protobuf.Value", "::prost_wkt_types::Value")
        .protoc_arg("--experimental_allow_proto3_optional")
        .file_descriptor_set_path(&spark_descriptor_file)
        .compile_protos(
            &[
                "spark-protos/spark/common.proto",
                "spark-protos/spark/spark.proto",
                "spark-protos/spark/spark_token.proto",
                "spark-protos/spark/spark_tree.proto",
                "spark-protos/spark/frost.proto",
                "spark-protos/spark/authn.proto",
            ],
            &["spark-protos"], // specify the root location to search proto dependencies
        )
        .expect("Failed to compile Spark Operator RPC protocol buffers");

    let spark_descriptor_bytes =
        read(&spark_descriptor_file).expect("Failed to read Spark Operator RPC protocol buffer descriptor");
    let spark_descriptor = FileDescriptorSet::decode(&*spark_descriptor_bytes)
        .expect("Failed to decode Spark Operator RPC protocol buffer descriptor");

    prost_wkt_build::add_serde(out_dir, spark_descriptor);

    println!("cargo:rerun-if-changed=spark-protos");
    println!("cargo:rerun-if-changed=build.rs");
}
