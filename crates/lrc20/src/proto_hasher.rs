use proto_hasher::hash_message;
use crate::token_transaction::TokenTransaction;
use crate::marshal::marshal_token_transaction;
use thiserror::Error;
use spark_protos::SPARK_FILE_DESCRIPTOR_SET;
use prost_reflect::{DescriptorPool, DynamicMessage};
use prost::Message;
use bitcoin::hashes::sha256::Hash as Sha256Hash;

#[derive(Error, Debug)]
pub enum ProtoHasherError {
    #[error("Proto hasher error: {0}")]
    ProtoHasherError(String),
    #[error("Marshal error: {0}")]
    MarshalError(String),
    #[error("Descriptor not found: {0}")]
    DescriptorNotFound(String),
    #[error("Decode error: {0}")]
    DecodeError(String),
    #[error("Hash error: {0}")]
    HashError(String),
}

pub fn get_descriptor_pool() -> DescriptorPool {
    DescriptorPool::decode(SPARK_FILE_DESCRIPTOR_SET).unwrap()
}

pub fn hash_token_transaction(descriptor_pool: DescriptorPool, token_transaction: TokenTransaction, is_partial_hash: bool) -> Result<Sha256Hash, ProtoHasherError> {
    let proto_token_transaction = marshal_token_transaction(token_transaction, !is_partial_hash)
        .map_err(|e| ProtoHasherError::MarshalError(e.to_string()))?;
    
    let descriptor = descriptor_pool.get_message_by_name("spark.TokenTransaction")
        .ok_or(ProtoHasherError::DescriptorNotFound("spark.TokenTransaction".to_string()))?;

    let message = DynamicMessage::decode(descriptor, proto_token_transaction.encode_to_vec().as_slice())
        .map_err(|e| ProtoHasherError::DecodeError(e.to_string()))?;

    let hash = hash_message(message)
        .map_err(|e| ProtoHasherError::HashError(e.to_string()))?;

    match hash {
        Some(hash) => Ok(hash),
        None => Err(ProtoHasherError::HashError("Hash is None".to_string())),
    }
}
