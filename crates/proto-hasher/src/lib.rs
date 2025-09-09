//! Proto hasher

use bitcoin::hashes::sha256::Hash as Sha256Hash;

/// Hash functions
pub mod hashing;

/// Hash functions and types for Google Protobufs
pub(crate) mod google_protobuf;

/// Errors
pub mod errors;

use errors::ProtoHasherError;
use prost_reflect::{DynamicMessage, ReflectMessage};

/// Main function for hashing protobufs
pub use hashing::hash_message;

/// Proto hasher
pub struct ProtoHasher {}

impl ProtoHasher {
    /// Creates a new ProtoHasher
    pub fn new() -> Self {
        Self {}
    }

    /// Hashes proto message
    pub fn hash_proto<M: Into<DynamicMessage>>(&self, message: M) -> Result<Sha256Hash, ProtoHasherError> {
        let message = message.into();
        let descriptor = message.descriptor();

        match descriptor.full_name() {
            "google.protobuf.Value"
            | "google.protobuf.ListValue"
            | "google.protobuf.BoolValue"
            | "google.protobuf.Int32Value"
            | "google.protobuf.Int64Value"
            | "google.protobuf.UInt32Value"
            | "google.protobuf.UInt64Value"
            | "google.protobuf.FloatValue"
            | "google.protobuf.DoubleValue"
            | "google.protobuf.StringValue"
            | "google.protobuf.BytesValue" => {
                return Err(ProtoHasherError::TopLevelScalarNotHashable(
                    descriptor.full_name().to_owned(),
                ));
            }
            _ => {}
        };

        let hash = hash_message(message)?.ok_or(ProtoHasherError::NothingToHash)?;

        Ok(hash)
    }
}
