use crate::error::FlowProcessorError;
use crate::flow_router::FlowProcessorRouter;

use tracing::{info, instrument};

const LOG_PATH: &str = "flow_processor:routes:bridge_runes_flow";

#[instrument(skip(flow_processor), level = "trace", ret)]
pub async fn handle(flow_processor: &mut FlowProcessorRouter) -> Result<(), FlowProcessorError> {
    info!("[{LOG_PATH}] Handling btc addr bridge runes flow ...");
    Ok(())
}
