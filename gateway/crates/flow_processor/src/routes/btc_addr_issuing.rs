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
        let msg = b"test_message";
        let message_hash = TweakGenerator::hash(msg);
        let nonce = TweakGenerator::generate_nonce();

        let generate_byte_seq = |public_key_package: &PublicKeyPackage| -> Vec<u8> {
            let musig = IssueBtcDepositAddressRequest {
                musig_id: MusigId::User {
                    user_public_key: PublicKey::from_str(
                        "038144ac71b61ab0e0a56967696a4f31a0cdd492cd3753d59aa978e0c8eaa5a60e",
                    )
                    .unwrap(),
                    rune_id: "RANDOM_1D".to_string(),
                },
                amount: 100,
            };
            generate_byte_seq(&musig, nonce)
        };
        let _logger_guard = &*TEST_LOGGER;
        let secp = Secp256k1::new();

        let verifiers_map = init_objects()?;
        let aggregator = FrostAggregator::new(
            verifiers_map,
            Arc::new(MockAggregatorMusigIdStorage::new()),
            Arc::new(MockAggregatorSignSessionStorage::new()),
        );

        let secret_key = SecretKey::from_slice(&[1u8; 32])?;
        let musig_id = MusigId::User {
            user_public_key: PublicKey::from_secret_key(&secp, &secret_key),
            rune_id: "test_rune_id".to_string(),
        };
        let public_key_package = aggregator.run_dkg_flow(&musig_id.clone()).await?;

        // === Running dkg flow
        let input_data = generate_byte_seq(&public_key_package);
        let hashed_input_data = TweakGenerator::hash(&input_data);

        let metadata = create_signing_metadata();
        let signature = aggregator
            .run_signing_flow(musig_id.clone(), &message_hash, metadata, Some(&hashed_input_data))
            .await?;
        let source_pubkey = public_key_package.verifying_key();
        let pubkey_to_check = PublicKey::from_slice(&source_pubkey.serialize()?)?;

        // === Tweaking btc pubkey
        let (tweaked_pubkey_btc, _) = TweakGenerator::tweak_btc_pubkey(&secp, pubkey_to_check, &hashed_input_data)?;

        let signature_to_check = secp256k1::schnorr::Signature::from_slice(&signature.serialize()?)?;

        secp.verify_schnorr(
            &signature_to_check,
            &secp256k1::Message::from_digest_slice(&message_hash)?,
            &tweaked_pubkey_btc.as_x_only_public_key(),
        )?;

        // === Tweaking pubkey_package
        let tweaked_public_key_package_frost =
            TweakGenerator::tweak_pubkey_package(public_key_package, &hashed_input_data);
        tweaked_public_key_package_frost
            .verifying_key()
            .verify(&message_hash, &signature)?;

        let frost_verifying_key = tweaked_public_key_package_frost.verifying_key();

        // === Converted frost pubkey
        let (tweaked_frost_pubkey_converted, _parity) =
            TweakGenerator::tweaked_verifying_key_to_tweaked_pubkey(frost_verifying_key)?;

        let signature_to_check = secp256k1::schnorr::Signature::from_slice(&signature.serialize()?)?;
        secp.verify_schnorr(
            &signature_to_check,
            &secp256k1::Message::from_digest_slice(&message_hash)?,
            &tweaked_frost_pubkey_converted.to_x_only_public_key(),
        )?;

        // === Tweaked pubkey from btc == Tweaked pubkey from frost lib
        assert_eq!(tweaked_pubkey_btc, tweaked_frost_pubkey_converted);

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
