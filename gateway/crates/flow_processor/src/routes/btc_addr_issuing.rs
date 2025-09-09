use crate::error::FlowProcessorError;
use crate::flow_router::FlowProcessorRouter;
use crate::types::DkgFlowRequest;
use bitcoin::key::{Keypair, TweakedPublicKey, UntweakedKeypair, UntweakedPublicKey};
use bitcoin::secp256k1::scalar::OutOfRangeError;
use bitcoin::secp256k1::{Parity, Scalar, Secp256k1};
use bitcoin::{secp256k1, Address, KnownHrp, Network, PublicKey};
use frost::traits::{AggregatorMusigIdStorage, AggregatorSignSessionStorage};
use frost::types::{AggregatorDkgState, AggregatorMusigIdData, RuneId};
use frost::utils::convert_public_key_package;
use frost_secp256k1_tr::keys::PublicKeyPackage;
use global_utils::tweak_generation::{GeneratedTweakScalar, Nonce, TweakGenerator};
use tracing::info;

const LOG_PATH: &str = "flow_processor:routes:btc_addr_issuing";

pub async fn handle(
    flow_processor: &mut FlowProcessorRouter,
    request: DkgFlowRequest,
) -> Result<Address, FlowProcessorError> {
    info!("[{LOG_PATH}] Handling btc addr issuing ...");
    let nonce = TweakGenerator::generate_nonce();
    let (msg, scalar, (tweaked_x, _parity)) = match flow_processor.storage.get_musig_id_data(&request.musig_id).await? {
        None => {
            let public_key_package = flow_processor
                .frost_aggregator
                .run_dkg_flow(&request.musig_id)
                .await
                .map_err(|e| FlowProcessorError::FrostAggregatorError(e.to_string()))?;
            tweak_pub_key_package(&request, &public_key_package, nonce)?
            //todo: store tweak value here
            //todo: implement db struct for storing
            // todo: store nonce and tweak values
        }
        Some(x) => match x.dkg_state {
            //todo: extract available nonce
            AggregatorDkgState::DkgRound1 { .. } | AggregatorDkgState::DkgRound2 { .. } => {
                let public_key_package = flow_processor
                    .frost_aggregator
                    .run_dkg_flow(&request.musig_id)
                    .await
                    .map_err(|e| FlowProcessorError::FrostAggregatorError(e.to_string()))?;
                tweak_pub_key_package(&request, &public_key_package, nonce)?
            }
            AggregatorDkgState::DkgFinalized { public_key_package } => {
                tweak_pub_key_package(&request, &public_key_package, nonce)?
            }
        },
    };
    Ok(Address::p2tr_tweaked(tweaked_x, KnownHrp::Mainnet))
}

fn tweak_pub_key_package(
    request: &DkgFlowRequest,
    public_key_package: &PublicKeyPackage,
    nonce: Nonce,
) -> Result<(Vec<u8>, Scalar, (TweakedPublicKey, Parity)), FlowProcessorError> {
    let pubkey = convert_public_key_package(&public_key_package)
        .map_err(|e| FlowProcessorError::InvalidDataError(e.to_string()))?;
    //todo: add request amount value
    let byte_vec = generate_byte_vec(pubkey, request.musig_id.get_rune_id(), 0, nonce);
    let tweak_scalar = generate_tweak_scalar(&byte_vec).unwrap();
    let tweaked_pubkey = TweakGenerator::tweak_pubkey(pubkey, &tweak_scalar)?;
    Ok((byte_vec, tweak_scalar, tweaked_pubkey))
}

fn generate_byte_vec(pubkey: secp256k1::PublicKey, rune_id: RuneId, amount: u128, nonce: Nonce) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(pubkey.to_string().as_bytes());
    data.extend_from_slice(rune_id.as_bytes());
    data.extend_from_slice(&amount.to_be_bytes());
    data.extend_from_slice(&nonce);
    data
}

fn generate_tweak_scalar(data: impl AsRef<[u8]>) -> Result<Scalar, OutOfRangeError> {
    TweakGenerator::generate_tweak_scalar(data)
}

#[cfg(test)]
mod tweak_signature_test {
    use global_utils::logger::{init_logger, LoggerGuard};
    use std::collections::BTreeMap;
    use std::str::FromStr;
    use std::sync::LazyLock;

    pub static TEST_LOGGER: LazyLock<LoggerGuard> = LazyLock::new(|| init_logger());

    use crate::routes::btc_addr_issuing::tweak_pub_key_package;
    use crate::types::DkgFlowRequest;
    use bitcoin::hashes::{sha256, Hash};
    use bitcoin::key::{TapTweak, TweakedPublicKey};
    use bitcoin::secp256k1::{Parity, PublicKey, Scalar, Secp256k1, SecretKey};
    use bitcoin::{secp256k1, TapNodeHash};
    use frost::signer::FrostSigner;
    use frost::traits::SignerClient;
    use frost::types::{MusigId, SigningMetadata, TokenTransactionMetadata};
    use frost::{aggregator::FrostAggregator, mocks::*};
    use frost_secp256k1_tr::keys::PublicKeyPackage;
    use frost_secp256k1_tr::keys::Tweak;
    use frost_secp256k1_tr::Identifier;
    use global_utils::tweak_generation::{GeneratedTweakScalar, TweakGenerator};
    use lrc20::token_transaction::{
        TokenTransaction, TokenTransactionCreateInput, TokenTransactionInput, TokenTransactionVersion,
    };
    use std::sync::Arc;

    #[tokio::test]
    async fn test_aggregator_signer_integration() -> anyhow::Result<()> {
        let msg = b"test_message";
        let message_hash = bitcoin::hashes::sha256::Hash::hash(msg).to_byte_array();
        let nonce = TweakGenerator::generate_nonce();

        let generate_tweak =
            |public_key_package: &PublicKeyPackage| -> (GeneratedTweakScalar, (TweakedPublicKey, Parity)) {
                let musig = DkgFlowRequest {
                    musig_id: MusigId::User {
                        user_public_key: PublicKey::from_str(
                            "038144ac71b61ab0e0a56967696a4f31a0cdd492cd3753d59aa978e0c8eaa5a60e",
                        )
                        .unwrap(),
                        rune_id: "RANDOM_1D".to_string(),
                    },
                };
                let (x, y, z) = tweak_pub_key_package(&musig, public_key_package, nonce).unwrap();
                (
                    GeneratedTweakScalar {
                        input_data: x,
                        scalar: y,
                    },
                    z,
                )
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

        let (tweak, scalar) = generate_tweak(&public_key_package);
        let tweak_gen_bytes: Option<&[u8]> = Some(&sha256::Hash::hash(&tweak.input_data).to_byte_array());
        println!("tweaked1: {:02X?}", tweak.scalar.to_be_bytes());

        let metadata = create_signing_metadata();
        let signature = aggregator
            .run_signing_flow(musig_id.clone(), &message_hash, metadata, tweak_gen_bytes)
            .await?;
        let source_pubkey = public_key_package.verifying_key();
        let pubkey_to_check = PublicKey::from_slice(&source_pubkey.serialize()?)?;
        let (tweaked_pubkey_to_check, _) = pubkey_to_check.x_only_public_key().0.tap_tweak(
            &secp,
            tweak_gen_bytes
                .map(|bytes| TapNodeHash::from_slice(&bytes))
                .transpose()?,
        );

        let signature_to_check = secp256k1::schnorr::Signature::from_slice(&signature.serialize()?)?;

        secp.verify_schnorr(
            &signature_to_check,
            &secp256k1::Message::from_digest_slice(&message_hash)?,
            &tweaked_pubkey_to_check.as_x_only_public_key(),
        )?;

        let tweaked_public_key_package = match tweak_gen_bytes.clone() {
            Some(tweak) => public_key_package.clone().tweak(Some(tweak.to_vec())),
            None => public_key_package.clone(),
        };
        tweaked_public_key_package
            .verifying_key()
            .verify(&message_hash, &signature)?;

        let public_key = tweaked_public_key_package.verifying_key();

        let tweaked_pubkey_to_check = PublicKey::from_slice(&public_key.serialize()?)?;
        let signature_to_check = secp256k1::schnorr::Signature::from_slice(&signature.serialize()?)?;

        secp.verify_schnorr(
            &signature_to_check,
            &secp256k1::Message::from_digest_slice(&message_hash)?,
            &tweaked_pubkey_to_check.x_only_public_key().0,
        )?;

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
