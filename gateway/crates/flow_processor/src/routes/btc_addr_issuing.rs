use crate::error::{BtcAddrIssueErrorEnum, FlowProcessorError};
use crate::flow_router::FlowProcessorRouter;
use crate::types::IssueBtcDepositAddressRequest;
use bitcoin::Address;
use frost::traits::AggregatorMusigIdStorage;
use frost::types::AggregatorDkgState;
use frost::utils::convert_public_key_package;
use gateway_local_db_store::schemas::deposit_address::{DepositAddrInfo, DepositAddressStorage, DepositStatus};
use frost::utils::{get_address, generate_nonce};
use tracing::{debug, info, instrument};

const LOG_PATH: &str = "flow_processor:routes:btc_addr_issuing";

pub async fn handle(
    flow_processor: &mut FlowProcessorRouter,
    request: IssueBtcDepositAddressRequest,
) -> Result<Address, FlowProcessorError> {
    info!("[{LOG_PATH}] Handling btc addr issuing ...");
    _handle_inner(flow_processor, &request)
        .await
        .map_err(|e| FlowProcessorError::BtcAddrIssueError(e))
}

#[instrument(skip(flow_processor, request), level = "trace", ret)]
async fn _handle_inner(
    flow_processor: &mut FlowProcessorRouter,
    request: &IssueBtcDepositAddressRequest,
) -> Result<Address, BtcAddrIssueErrorEnum> {
    let local_db_storage = flow_processor.storage.clone();

    let public_key_package = 
        match flow_processor.storage.get_musig_id_data(&request.musig_id).await? {
            None => {
                debug!("[{LOG_PATH}] Missing musig, running dkg from the beginning ...");
                let pubkey_package = flow_processor.frost_aggregator.run_dkg_flow(&request.musig_id).await?;
                debug!("[{LOG_PATH}] DKG processing was successfully completed");
                pubkey_package
            }
            Some(x) => {
                debug!("[{LOG_PATH}] Musig exists, obtaining dkg pubkey ...");
                // extract data from db, get nonce and generate new one, return it to user
                match x.dkg_state {
                    AggregatorDkgState::DkgRound1 { .. } => {
                        return Err(BtcAddrIssueErrorEnum::UnfinishedDkgState {
                            got: "AggregatorDkgState::DkgRound1".to_string(),
                        });
                    }
                    AggregatorDkgState::DkgRound2 { .. } => {
                        return Err(BtcAddrIssueErrorEnum::UnfinishedDkgState {
                            got: "AggregatorDkgState::DkgRound2".to_string(),
                        });
                    }
                    AggregatorDkgState::DkgFinalized {
                        public_key_package: pubkey_package,
                    } => pubkey_package
                }
            }
        };


    let nonce = generate_nonce();
    let public_key = convert_public_key_package(&public_key_package)
        .map_err(|e| BtcAddrIssueErrorEnum::InvalidDataError(e.to_string()))?;
    let address = get_address(public_key, nonce, flow_processor.network)
        .map_err(|e| BtcAddrIssueErrorEnum::InvalidDataError(format!("Failed to create address: {}", e)))?;

    local_db_storage
        .set_deposit_addr_info(
            &request.musig_id,
            DepositAddrInfo {
                nonce_tweak: nonce.to_vec(),
                address: Some(address.to_string()),
                is_btc: true,
                amount: request.amount,
                confirmation_status: DepositStatus::InitializedRunesSpark,
            },
        )
        .await?;

    Ok(address)
}

#[cfg(test)]
mod tweak_signature_test {
    use global_utils::logger::{LoggerGuard, init_logger};
    use std::collections::BTreeMap;
    use std::str::FromStr;
    use std::sync::LazyLock;

    pub static TEST_LOGGER: LazyLock<LoggerGuard> = LazyLock::new(|| init_logger());

    use crate::types::IssueBtcDepositAddressRequest;
    use bitcoin::secp256k1;
    use bitcoin::secp256k1::{PublicKey, Secp256k1, SecretKey};
    use frost::signer::FrostSigner;
    use frost::traits::SignerClient;
    use frost::types::{MusigId, SigningMetadata, TokenTransactionMetadata};
    use frost::{aggregator::FrostAggregator, mocks::*};
    use frost_secp256k1_tr::Identifier;
    use frost_secp256k1_tr::keys::PublicKeyPackage;
    use lrc20::token_transaction::{
        TokenTransaction, TokenTransactionCreateInput, TokenTransactionInput, TokenTransactionVersion,
    };
    use std::sync::Arc;

    #[tokio::test]
    async fn test_aggregator_signer_integration() -> anyhow::Result<()> {
        // TODO create test that can spend from the address we created
        Ok(())
    }

    fn create_signing_metadata() -> SigningMetadata {
        let token_transaction_metadata = TokenTransactionMetadata::PartialCreateToken {
            token_transaction: TokenTransaction {
                version: TokenTransactionVersion::V2,
                input: TokenTransactionInput::Create(TokenTransactionCreateInput {
                    issuer_public_key: PublicKey::from_secret_key(
                        &Secp256k1::new(),
                        &SecretKey::from_slice(&[1u8; 32]).unwrap(),
                    ),
                    token_name: "test_token".to_string(),
                    token_ticker: "TEST".to_string(),
                    decimals: 8,
                    max_supply: 1000000000000000000,
                    is_freezable: false,
                    creation_entity_public_key: None,
                }),
                leaves_to_create: vec![],
                spark_operator_identity_public_keys: vec![],
                expiry_time: 0,
                network: None,
                client_created_timestamp: 0,
            },
        };

        SigningMetadata {
            token_transaction_metadata,
        }
    }

    fn create_mock_signer(identifier: u16) -> FrostSigner {
        FrostSigner::new(
            identifier,
            Arc::new(MockSignerMusigIdStorage::new()),
            Arc::new(MockSignerSignSessionStorage::default()),
            3,
            2,
        )
    }

    fn init_objects() -> anyhow::Result<BTreeMap<Identifier, Arc<dyn SignerClient>>> {
        let signer1 = create_mock_signer(1);
        let signer2 = create_mock_signer(2);
        let signer3 = create_mock_signer(3);

        let mock_signer_client1 = MockSignerClient::new(signer1);
        let mock_signer_client2 = MockSignerClient::new(signer2);
        let mock_signer_client3 = MockSignerClient::new(signer3);

        let identifier_1: Identifier = 1.try_into()?;
        let identifier_2: Identifier = 2.try_into()?;
        let identifier_3: Identifier = 3.try_into()?;

        Ok(BTreeMap::from([
            (identifier_1, Arc::new(mock_signer_client1) as Arc<dyn SignerClient>),
            (identifier_2, Arc::new(mock_signer_client2) as Arc<dyn SignerClient>),
            (identifier_3, Arc::new(mock_signer_client3) as Arc<dyn SignerClient>),
        ]))
    }
}
