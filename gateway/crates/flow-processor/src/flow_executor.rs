use tokio;
use tokio::sync::{mpsc, oneshot};
use tokio::time::Duration;
use crate::types::*;
use crate::errors::FlowProcessorError;
use persistent_storage::init::PostgresRepo;
use tracing;
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Clone)]
pub struct FlowProcessorState {
    storage: PostgresRepo,
}

pub struct FlowProcessor {
    pub tx_receiver: mpsc::Receiver<(FlowProcessorMessage, OneshotFlowProcessorSender)>,
    pub state: FlowProcessorState,
    flows_in_progress: HashMap<Uuid, (OneshotFlowProcessorReceiver, OneshotFlowProcessorSender)>,
}

impl FlowProcessor {
    pub fn new(
        tx_receiver: mpsc::Receiver<(FlowProcessorMessage, OneshotFlowProcessorSender)>,
        storage: PostgresRepo
    ) -> Self {
        Self { tx_receiver, state: FlowProcessorState { storage }, flows_in_progress: HashMap::new() }
    }

    pub async fn run(&mut self) {
        loop {
            let wrapper = self.tx_receiver.recv().await;
            
            match wrapper {
                None => {
                    panic!("Channel closed unexpectedly");
                }
                Some(wrapper) => {
                    tracing::info!("[main] Received message");

                    let (message, response_sender) = wrapper;

                    let (router_sender, router_receiver) = oneshot::channel::<Result<FlowProcessorResponse, FlowProcessorError>>();

                    let flow_id = uuid::Uuid::new_v4();
                    self.flows_in_progress.insert(flow_id, (router_receiver, response_sender));
                    
                    let mut router = FlowProcessorRouter::new(self.state.clone(), flow_id);
                    tokio::task::spawn(async move {
                        tracing::info!("[main] Running flow for id {}", flow_id);
                        router.run(message, router_sender).await;
                        tracing::info!("[main] Flow for id {} finished", flow_id);
                    });
                }
            }

            let ids = self.flows_in_progress.keys().cloned().collect::<Vec<_>>();
            for flow_id in ids {
                if let Some((mut receiver, sender)) = self.flows_in_progress.remove(&flow_id) {
                    if let Ok(message) = receiver.try_recv() {
                        tracing::info!("[main] Sending response for id {}", flow_id);
                        let _ = sender.send(message).map_err(|_| {
                            tracing::error!("[main] Failed to send response for id {}", flow_id);
                        });
                        self.flows_in_progress.remove(&flow_id);
                    }
                }
            }
        }
    }
}

struct FlowProcessorRouter {
    state: FlowProcessorState,
    flow_id: Uuid,
}

impl FlowProcessorRouter {
    fn new(
        state: FlowProcessorState,
        flow_id: Uuid
    ) -> Self {
        Self { state, flow_id }
    }

    async fn run(&mut self, message: FlowProcessorMessage, response_sender: oneshot::Sender<Result<FlowProcessorResponse, FlowProcessorError>>) {
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

        tracing::info!("[router] Attempting to send response for flow id {}", self.flow_id);
        let _ = response_sender.send(response).map_err(|_| {
            tracing::error!("[router] Failed to send response for flow id {}", self.flow_id);
        });
        tracing::info!("[router] Finished sending response for flow id {}", self.flow_id);
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
            message: format!("message for {}", self.flow_id),
        })
    }
}