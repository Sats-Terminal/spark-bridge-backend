use crate::error::FlowProcessorError;
use crate::flow_router::FlowProcessorRouter;
use crate::types::ExitSparkResponse;
use tracing::info;

const LOG_PATH: &str = "flow_processor:routes:exit_spark_flow";

pub async fn handle(x: &mut FlowProcessorRouter) -> Result<(), FlowProcessorError> {
    info!("[{LOG_PATH}] Handling exit spark flow ...");
    Ok(())
}
