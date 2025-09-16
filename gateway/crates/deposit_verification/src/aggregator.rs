use gateway_flow_processor::flow_sender::FlowSender;
use crate::traits::VerificationClient;
use std::sync::Arc;
use crate::types::*;
use crate::error::DepositVerificationError;

#[derive(Clone, Debug)]
pub struct DepositVerificationAggregator {
    flow_sender: FlowSender,
    verifiers: Vec<Arc<dyn VerificationClient>>,
}

impl DepositVerificationAggregator {
    pub fn new(flow_sender: FlowSender, verifiers: Vec<Arc<dyn VerificationClient>>) -> Self {
        Self { flow_sender, verifiers }
    }

    
}