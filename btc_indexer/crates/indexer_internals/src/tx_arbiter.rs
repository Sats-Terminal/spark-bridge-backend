use async_trait::async_trait;
use bitcoin::{OutPoint, Txid};
use btc_indexer_api::api::{Amount, BtcTxReview, TxRejectReason};
use local_db_store_indexer::schemas::tx_tracking_storage::TxToUpdateStatus;
use ordinals::RuneId;
use std::collections::BTreeMap;
use std::sync::Arc;
use thiserror::Error;
use titan_client::TitanApi;
use titan_types::{RuneAmount, Transaction};
use tracing::instrument;

const BTC_BLOCK_CONFIRMATION_HEIGHT: u64 = 6;

#[derive(Debug, Clone, Copy)]
pub struct TxArbiter {}

#[async_trait]
pub trait TxArbiterTrait: Clone + Send + Sync + 'static {
    async fn check_tx<C: TitanApi>(
        &self,
        titan_client: Arc<C>,
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
    #[error("Decode error: {0}")]
    DecodeError(String),
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

#[async_trait]
impl TxArbiterTrait for TxArbiter {
    #[instrument(skip(titan_client), level = "trace", ret)]
    async fn check_tx<C: TitanApi>(
        &self,
        titan_client: Arc<C>,
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

        if !tx_to_check.has_runes() {
            return Ok(TxArbiterResponse::ReviewFormed(
                BtcTxReview::Failure {
                    reason: TxRejectReason::NoRunesInOuts,
                },
                out_point,
            ));
        }

        let current_tip = titan_client.get_tip().await?;
        if tx_to_check.status.block_height.is_none() || !tx_to_check.status.confirmed {
            return Ok(TxArbiterResponse::Rejected(RejectReason::NotIncludedInBlock));
        }

        let obtained_block_height = tx_to_check
            .status
            .block_height
            .ok_or(TxArbiterError::DecodeError("Block height not found".to_string()))?;
        if current_tip.height.saturating_sub(obtained_block_height) < BTC_BLOCK_CONFIRMATION_HEIGHT {
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

        let v_out = tx_info.v_out as usize;
        if tx_to_check.output.len() <= v_out {
            return Ok(TxArbiterResponse::ReviewFormed(
                BtcTxReview::Failure {
                    reason: TxRejectReason::NoExpectedVOutInOutputs {
                        got: tx_to_check.output.len() as u64,
                        expected: v_out as u64,
                    },
                },
                out_point,
            ));
        }

        if let Some(x) = tx_to_check.output.get(v_out) {
            if !x.has_runes() {
                return Ok(TxArbiterResponse::ReviewFormed(
                    BtcTxReview::Failure {
                        reason: TxRejectReason::NoExpectedTOutWithRunes,
                    },
                    out_point,
                ));
            }
            if !Self::check_runes_validity(&x.runes, tx_info.amount, tx_info.rune_id) {
                return Ok(TxArbiterResponse::ReviewFormed(
                    BtcTxReview::Failure {
                        reason: TxRejectReason::NoExpectedTOutWithRunesAmount { amount: tx_info.amount },
                    },
                    out_point,
                ));
            }
        }

        Ok(TxArbiterResponse::ReviewFormed(BtcTxReview::Success, out_point))
    }
}

impl TxArbiter {
    /// One rune entry consist from one RuneId of runes and has equal value to amount
    fn check_runes_validity(tx_to_check: &[RuneAmount], amount: Amount, id: RuneId) -> bool {
        let counted_runes = Self::count_runes_btree(tx_to_check.iter());
        if counted_runes.len() == 1
            && let Some((k, v)) = counted_runes.first_key_value()
            && *v == amount as u128
            && *k == id
        {
            true
        } else {
            false
        }
    }

    fn count_runes_btree<'a>(runes: impl Iterator<Item = &'a RuneAmount>) -> BTreeMap<RuneId, u128> {
        let mut rune_counts = BTreeMap::new();
        for rune in runes {
            *rune_counts
                .entry(RuneId {
                    block: rune.rune_id.block,
                    tx: rune.rune_id.tx,
                })
                .or_insert(0) += rune.amount;
        }
        rune_counts
    }
}
