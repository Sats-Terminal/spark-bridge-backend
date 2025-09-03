use gateway_flow_processor::flow_sender::FlowSender;

#[derive(Clone)]
pub struct AppState {
    pub flow_sender: FlowSender,
}
