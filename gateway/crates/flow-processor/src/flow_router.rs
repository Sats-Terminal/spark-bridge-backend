use crate::errors::FlowProcessorError;
use crate::types::*;
use persistent_storage::init::PostgresRepo;
use tokio::sync::mpsc;
use tracing;
use uuid::Uuid;

// This struct is used to route the message to the correct flow
// This struct instance is created for each message that is sent to the flow processor
pub struct FlowProcessorRouter {
    storage: PostgresRepo,
    flow_id: Uuid,
    response_sender: OneshotFlowProcessorSender,
    task_sender: mpsc::Sender<Uuid>,
}

impl FlowProcessorRouter {
    pub fn new(
        storage: PostgresRepo,
        flow_id: Uuid,
        response_sender: OneshotFlowProcessorSender,
        task_sender: mpsc::Sender<Uuid>,
    ) -> Self {
        Self {
            storage,
            flow_id,
            response_sender,
            task_sender,
        }
    }

    pub async fn run(mut self, message: FlowProcessorMessage) {
        let response = match message {
            FlowProcessorMessage::RunDkgFlow(request) => {
                let response = self.run_dkg_flow(request).await;
                let answer = response.map(|response| FlowProcessorResponse::RunDkgFlow(response));
                answer
            }
            FlowProcessorMessage::BridgeRunes(request) => {
                let response = self.run_bridge_runes_flow(request).await;
                let answer = response.map(|response| FlowProcessorResponse::BridgeRunes(response));
                answer
            }
            FlowProcessorMessage::ExitSpark(request) => {
                let response = self.run_exit_spark_flow(request).await;
                let answer = response.map(|response| FlowProcessorResponse::ExitSpark(response));
                answer
            }
        };

        let _ = self.response_sender.send(response).map_err(|_| {
            tracing::error!("[router] Failed to send response for flow id {}", self.flow_id);
        });

        let _ = self.task_sender.send(self.flow_id).await.map_err(|_| {
            tracing::error!("[router] Failed to send task for flow id {}", self.flow_id);
        });
    }

    async fn run_dkg_flow(&mut self, request: DkgFlowRequest) -> Result<DkgFlowResponse, FlowProcessorError> {
        Ok(DkgFlowResponse {
            public_key: format!("[router] public_key for {}", request.request_id),
        })
    }

    async fn run_bridge_runes_flow(
        &mut self,
        request: BridgeRunesRequest,
    ) -> Result<BridgeRunesResponse, FlowProcessorError> {
        Ok(BridgeRunesResponse {
            message: format!("message for {}", request.request_id),
        })
    }

    async fn run_exit_spark_flow(
        &mut self,
        request: ExitSparkRequest,
    ) -> Result<ExitSparkResponse, FlowProcessorError> {
        Ok(ExitSparkResponse {
            message: format!("message for {}", request.request_id),
        })
    }
}
