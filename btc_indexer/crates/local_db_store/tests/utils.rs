use std::sync::LazyLock;

use global_utils::logger::{init_logger, LoggerGuard};

pub static TEST_LOGGER: LazyLock<LoggerGuard> = LazyLock::new(|| init_logger());

fn btc_tx_review_eq(a: &btc_indexer_api::api::BtcTxReview, b: &btc_indexer_api::api::BtcTxReview) -> bool {
    use btc_indexer_api::api::{BtcTxReview, TxRejectReason};
    match (a, b) {
        (BtcTxReview::Success, BtcTxReview::Success) => true,
        (BtcTxReview::Failure { reason: r1 }, BtcTxReview::Failure { reason: r2 }) => match (r1, r2) {
            (TxRejectReason::NoRunesInOuts, TxRejectReason::NoRunesInOuts) => true,
            (TxRejectReason::NoFeesPayed, TxRejectReason::NoFeesPayed) => true,
            (
                TxRejectReason::TooFewSatoshiPaidAsFee {
                    got: g1,
                    at_least_expected: e1,
                },
                TxRejectReason::TooFewSatoshiPaidAsFee {
                    got: g2,
                    at_least_expected: e2,
                },
            ) => g1 == g2 && e1 == e2,
            (
                TxRejectReason::NoExpectedVOutInOutputs { got: g1, expected: e1 },
                TxRejectReason::NoExpectedVOutInOutputs { got: g2, expected: e2 },
            ) => g1 == g2 && e1 == e2,
            (TxRejectReason::NoExpectedTOutWithRunes, TxRejectReason::NoExpectedTOutWithRunes) => true,
            (
                TxRejectReason::NoExpectedTOutWithRunesAmount { amount: a1 },
                TxRejectReason::NoExpectedTOutWithRunesAmount { amount: a2 },
            ) => a1 == a2,
            _ => false,
        },
        _ => false,
    }
}

fn tx_tracking_requests_to_send_response_eq(
    a: &local_db_store_indexer::schemas::track_tx_requests_storage::TxTrackingRequestsToSendResponse,
    b: &local_db_store_indexer::schemas::track_tx_requests_storage::TxTrackingRequestsToSendResponse,
) -> bool {
    a.uuid == b.uuid
        && a.out_point == b.out_point
        && a.callback_url == b.callback_url
        && btc_tx_review_eq(&a.review, &b.review)
}

pub fn tx_tracking_requests_vec_eq(
    a: &[local_db_store_indexer::schemas::track_tx_requests_storage::TxTrackingRequestsToSendResponse],
    b: &[local_db_store_indexer::schemas::track_tx_requests_storage::TxTrackingRequestsToSendResponse],
) -> bool {
    a.len() == b.len()
        && a.iter()
            .zip(b.iter())
            .all(|(x, y)| tx_tracking_requests_to_send_response_eq(x, y))
}
