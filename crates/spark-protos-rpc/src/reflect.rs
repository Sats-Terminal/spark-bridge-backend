use prost_reflect::{DescriptorError, DescriptorPool, DynamicMessage, MessageDescriptor, prost::DecodeError};

use crate::{SPARK_FILE_DESCRIPTOR_SET, spark_token};

pub trait ToDynamicMessage {
    fn to_dynamic(&self) -> Result<DynamicMessage, SparkProtoReflectError>;
}

fn spark_lookup_descriptor(full_name: &str) -> Result<MessageDescriptor, SparkProtoReflectError> {
    DescriptorPool::decode(SPARK_FILE_DESCRIPTOR_SET)?
        .get_message_by_name(full_name)
        .ok_or(SparkProtoReflectError::MessageDescriptorNotFound(full_name.to_string()))
}

#[macro_export]
macro_rules! impl_to_dynamic {
    // multiple entries
    ( $( $ty:path => $full:expr ),+ $(,)? ) => {
        $(
            $crate::impl_to_dynamic!(@one $ty => $full);
        )+
    };
    // single entry
    (@one $ty:path => $full:expr) => {
        impl ToDynamicMessage for $ty {
            #[inline]
            fn to_dynamic(&self) -> Result<DynamicMessage, SparkProtoReflectError> {
                let desc = spark_lookup_descriptor($full)?;
                let bytes = <Self as prost::Message>::encode_to_vec(self);
                Ok(DynamicMessage::decode(desc, &bytes[..])?)
            }
        }
    };
}

impl_to_dynamic! {
    spark_token::TokenTransaction => "spark_token.TokenTransaction",
}

#[derive(Debug, thiserror::Error)]
pub enum SparkProtoReflectError {
    #[error("Descriptor error: {0}")]
    DescriptorError(#[from] DescriptorError),
    #[error("MessageDescriptor is missing with name: {0}")]
    MessageDescriptorNotFound(String),
    #[error("Message decoding error: {0}")]
    MessageDecodeError(#[from] DecodeError),
}
