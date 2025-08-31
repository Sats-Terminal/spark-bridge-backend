use thiserror::Error;

#[derive(Error, Debug)]
pub enum FlowProcessorError {
    #[error("Channel closed error: {0}")]
    ChannelClosedError(String),
    #[error("Invalid response type: {0}")]
    InvalidResponseType(String),
}
