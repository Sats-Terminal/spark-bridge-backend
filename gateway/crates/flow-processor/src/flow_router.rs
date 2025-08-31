use tokio::sync::mpsc;
use uuid::Uuid;
use crate::types::*;
use crate::errors::FlowProcessorError;
use tracing;
use tokio::time::Duration;
use persistent_storage::init::PostgresRepo;


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
        task_sender: mpsc::Sender<Uuid>
    ) -> Self {
        Self { storage, flow_id, response_sender, task_sender }
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
            FlowProcessorMessage::Testing(request) => {
                let response = self.run_testing_flow(request).await;
                let answer = response.map(|response| FlowProcessorResponse::Testing(response));
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

    async fn run_bridge_runes_flow(&mut self, request: BridgeRunesRequest) -> Result<BridgeRunesResponse, FlowProcessorError> {
        Ok(BridgeRunesResponse {
            message: format!("message for {}", request.request_id),
        })
    }

    async fn run_exit_spark_flow(&mut self, request: ExitSparkRequest) -> Result<ExitSparkResponse, FlowProcessorError> {
        Ok(ExitSparkResponse {
            message: format!("message for {}", request.request_id),
        })
    }

    async fn run_testing_flow(&mut self, request: TestingRequest) -> Result<TestingResponse, FlowProcessorError> {
        let mut cur_time = 0;
        tracing::info!("[router] [{}] Testing flow running for {} seconds", self.flow_id, cur_time);
        for _ in 0..request.n_runs {
            tokio::time::sleep(Duration::from_secs(request.n_seconds)).await;
            cur_time += request.n_seconds;
            tracing::info!("[router] [{}] Testing flow running for {} seconds", self.flow_id, cur_time);
        }

        tracing::info!("[router] [{}] Testing flow finished", self.flow_id);
        Ok(TestingResponse {
            message: request.message,
        })
    }
}