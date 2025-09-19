use bitcoin::{OutPoint, Txid};
use btc_indexer_api::api::{BtcTxReview, TxRejectReason};
use local_db_store_indexer::schemas::tx_tracking_storage::TxToUpdateStatus;
use thiserror::Error;
use titan_client::TitanApi;
use titan_types::Transaction;
use tracing::instrument;

const BTC_BLOCK_CONFIRMATION_HEIGHT: u64 = 6;
const INPUT_V_BYTES_WEIGHT: u64 = 58;
const OUTPUT_V_BYTES_WEIGHT: u64 = 43;
const SATOSHI_PER_V_BYTE: u64 = 4;

pub struct TxArbiter {}

pub trait TxArbiterTrait {
    async fn check_tx<C: TitanApi>(
        titan_client: C,
        tx_to_check: &Transaction,
        tx_info: &TxToUpdateStatus,
    ) -> Result<TxArbiterResponse, TxArbiterError>;
}

#[derive(Debug, Error)]
pub enum TxArbiterError {
    #[error("Incorrect tc_id, got: {got}, expected: {expected}")]
    IncorrectTxId { got: Txid, expected: Txid },
    #[error("Titan client error: {0}")]
    TitanError(#[from] titan_client::Error),
}

#[derive(Debug)]
pub enum TxArbiterResponse {
    ReviewFormed(BtcTxReview, OutPoint),
    /// Has to be asked one more time later about tx status, not critical error
    Rejected(RejectReason),
}

#[derive(Debug)]
pub enum RejectReason {
    NotIncludedInBlock,
    NotEnoughConfirmations {
        current_block_height: u64,
        got: u64,
        needed_confirmations: u64,
    },
}

impl TxArbiterTrait for TxArbiter {
    #[instrument(skip(titan_client), level = "debug")]
    async fn check_tx<C: TitanApi>(
        titan_client: C,
        tx_to_check: &Transaction,
        tx_info: &TxToUpdateStatus,
    ) -> Result<TxArbiterResponse, TxArbiterError> {
        let out_point = OutPoint {
            txid: tx_info.tx_id.0,
            vout: tx_info.v_out,
        };

        if tx_to_check.txid != tx_info.tx_id.0 {
            return Err(TxArbiterError::IncorrectTxId {
                got: tx_to_check.txid,
                expected: tx_info.tx_id.0,
            });
        }

        if tx_to_check.has_runes() {
            return Ok(TxArbiterResponse::ReviewFormed(
                BtcTxReview::Failure {
                    reason: TxRejectReason::NoRunesInOuts,
                },
                out_point,
            ));
        }

        let current_tip = titan_client.get_tip().await?;
        if tx_to_check.status.block_height == None || !tx_to_check.status.confirmed {
            return Ok(TxArbiterResponse::Rejected(RejectReason::NotIncludedInBlock));
        }

        let obtained_block_height = tx_to_check.status.block_height.unwrap();
        if current_tip.height.saturating_sub(obtained_block_height) <= BTC_BLOCK_CONFIRMATION_HEIGHT {
            return Ok(TxArbiterResponse::Rejected(RejectReason::NotEnoughConfirmations {
                current_block_height: current_tip.height,
                got: obtained_block_height,
                needed_confirmations: BTC_BLOCK_CONFIRMATION_HEIGHT,
            }));
        }

        let fees_payed = tx_to_check.fee_paid_sat();
        if fees_payed.is_none() {
            return Ok(TxArbiterResponse::ReviewFormed(
                BtcTxReview::Failure {
                    reason: TxRejectReason::NoFeesPayed,
                },
                out_point,
            ));
        }

        let desired_satoshi_fee_amount = Self::calculate_desired_satoshi_fee_amount(tx_to_check);
        let fees_payed = fees_payed.unwrap();
        if desired_satoshi_fee_amount < fees_payed {
            return Ok(TxArbiterResponse::ReviewFormed(
                BtcTxReview::Failure {
                    reason: TxRejectReason::TooFewSatoshiPaidAsFee {
                        got: fees_payed,
                        at_least_expected: desired_satoshi_fee_amount,
                    },
                },
                out_point,
            ));
        }

        Ok(TxArbiterResponse::ReviewFormed(BtcTxReview::Success, out_point))
    }
}

impl TxArbiter {
    fn calculate_desired_satoshi_fee_amount(tx_to_check: &Transaction) -> u64 {
        let (inputs, outputs) = (tx_to_check.input.len() as u64, tx_to_check.output.len() as u64);
        INPUT_V_BYTES_WEIGHT * SATOSHI_PER_V_BYTE * inputs + OUTPUT_V_BYTES_WEIGHT * SATOSHI_PER_V_BYTE * outputs
    }
}
