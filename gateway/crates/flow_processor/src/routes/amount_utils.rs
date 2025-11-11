use crate::error::FlowProcessorError;
use crate::rune_metadata_client::RuneMetadataClient;
use std::sync::Arc;

pub async fn normalize_rune_amount(
    amount: u64,
    rune_id: &str,
    metadata_client: &Option<Arc<RuneMetadataClient>>,
) -> Result<u64, FlowProcessorError> {
    let Some(client) = metadata_client else {
        return Ok(amount);
    };

    let metadata = match client.get_metadata(rune_id).await {
        Ok(metadata) => metadata,
        Err(err) => {
            tracing::warn!(
                "Failed to fetch rune metadata for {} to normalize amount: {}. Using raw amount.",
                rune_id,
                err
            );
            return Ok(amount);
        }
    };

    let factor = 10u128.checked_pow(metadata.divisibility as u32).ok_or_else(|| {
        FlowProcessorError::InvalidDataError(format!(
            "Divisibility {} for rune {} is too large",
            metadata.divisibility, rune_id
        ))
    })?;
    let scaled = (amount as u128).checked_mul(factor).ok_or_else(|| {
        FlowProcessorError::InvalidDataError(format!(
            "Amount {} with divisibility {} exceeds supported range",
            amount, metadata.divisibility
        ))
    })?;

    if scaled > u64::MAX as u128 {
        return Err(FlowProcessorError::InvalidDataError(format!(
            "Normalized amount {} exceeds u64 limits",
            scaled
        )));
    }

    Ok(scaled as u64)
}
