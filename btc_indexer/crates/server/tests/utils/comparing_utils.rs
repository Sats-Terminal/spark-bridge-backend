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

pub fn btc_indexer_meta_eq(
    meta_a: &btc_indexer_api::api::ResponseMeta,
    meta_b: &btc_indexer_api::api::ResponseMeta,
) -> bool {
    meta_a.outpoint == meta_b.outpoint && btc_tx_review_eq(&meta_a.status, &meta_b.status)
}
