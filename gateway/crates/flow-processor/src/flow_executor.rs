use tokio;
use tokio::sync::{mpsc, oneshot};
use tokio::time::Duration;
use tokio::task::JoinHandle;
use crate::types::*;
use crate::errors::FlowProcessorError;
use persistent_storage::init::PostgresRepo;
use tracing;
use std::collections::HashMap;
use uuid::Uuid;
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
pub struct FlowProcessorState {
    storage: PostgresRepo,
}

pub struct FlowProcessor {
    pub tx_receiver: mpsc::Receiver<(FlowProcessorMessage, OneshotFlowProcessorSender)>,
    pub flow_receiver: mpsc::Receiver<Uuid>,
    pub flow_sender: mpsc::Sender<Uuid>,
    pub state: FlowProcessorState,
    pub flows: HashMap<Uuid, JoinHandle<()>>,
    pub cancellation_token: CancellationToken,
}

impl FlowProcessor {
    pub fn new(
        tx_receiver: mpsc::Receiver<(FlowProcessorMessage, OneshotFlowProcessorSender)>,
        storage: PostgresRepo,
        cancellation_token: CancellationToken
    ) -> Self {
        let (flow_sender, flow_receiver) = mpsc::channel::<Uuid>(1000);
        Self { 
            tx_receiver, 
            flow_receiver, 
            flow_sender, 
            state: FlowProcessorState { storage }, 
            flows: HashMap::new(),
            cancellation_token
        }
    }

    pub async fn run(&mut self) {
        loop {
            tokio::select! {
                flow = self.flow_receiver.recv() => {
                    match flow {
                        None => {
                            tracing::error!("[main] Task channel closed unexpectedly");
                            break;
                        }
                        Some(flow_id) => {
                            tracing::info!("[main] Received task for id {}", flow_id);
                            let _ = self.flows.remove(&flow_id);
                        }
                    }
                }
                wrapper = self.tx_receiver.recv() => {
                    match wrapper {
                        None => {
                            tracing::error!("[main] Message channel closed unexpectedly");
                            break;
                        }
                        Some(wrapper) => {
                            tracing::info!("[main] Received message");
        
                            let (message, response_sender) = wrapper;
        
                            let flow_id = uuid::Uuid::new_v4();
                            
                            let router = FlowProcessorRouter::new(
                                self.state.clone(), 
                                flow_id,
                                response_sender,
                                self.flow_sender.clone()
                            );
        
                            let handle = tokio::task::spawn(async move {
                                tracing::info!("[main] Running flow for id {}", flow_id);
                                router.run(message).await;
                                tracing::info!("[main] Flow for id {} finished", flow_id);
                            });
        
                            self.flows.insert(flow_id, handle);
                        }
                    }
                }
                _ = self.cancellation_token.cancelled() => {
                    tracing::info!("[main] Shutting down flow processor");

                    for i in 0..10 {
                        if self.flows.is_empty() {
                            break;
                        }
                        while let Some(flow_id) = self.flow_receiver.recv().await {
                            let _ = self.flows.remove(&flow_id);
                        }
                        tokio::time::sleep(Duration::from_secs(1)).await;
                        tracing::info!("[main] Waiting flows to finish {}/10", i);
                    }
                    
                    for (flow_id, handle) in self.flows.iter() {
                        let _ = handle.abort();
                        tracing::info!("[main] Aborted flow for id {}", flow_id);
                    }

                    self.flows.clear();
                    break;
                }
            }
        }
    }
}

struct FlowProcessorRouter {
    state: FlowProcessorState,
    flow_id: Uuid,
    response_sender: OneshotFlowProcessorSender,
    task_sender: mpsc::Sender<Uuid>,
}

impl FlowProcessorRouter {
    fn new(
        state: FlowProcessorState,
        flow_id: Uuid,
        response_sender: OneshotFlowProcessorSender,
        task_sender: mpsc::Sender<Uuid>
    ) -> Self {
        Self { state, flow_id, response_sender, task_sender }
    }

    async fn run(mut self, message: FlowProcessorMessage) {
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
            message: format!("message for {}", self.flow_id),
        })
    }
}