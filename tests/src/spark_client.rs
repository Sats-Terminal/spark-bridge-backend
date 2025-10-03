use crate::error::SparkClientError;
use bitcoin::hashes::{Hash, sha256};
use bitcoin::secp256k1::PublicKey;
use bitcoin::secp256k1::{Keypair, Message as BitcoinMessage, Secp256k1};
use hex;
use rand_core::OsRng;
use rustls;
use spark_address::decode_spark_address;
use spark_protos::prost::Message;
use spark_protos::spark::QueryTokenOutputsRequest;
use spark_protos::spark::spark_service_client::SparkServiceClient;
use spark_protos::spark_authn::spark_authn_service_client::SparkAuthnServiceClient;
use spark_protos::spark_authn::{GetChallengeRequest, VerifyChallengeRequest};
use spark_protos::spark_token::spark_token_service_client::SparkTokenServiceClient;
use spark_protos::spark_token::{
    CommitTransactionRequest, CommitTransactionResponse, StartTransactionRequest, StartTransactionResponse,
};
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Once;
use std::time::{SystemTime, UNIX_EPOCH};
use token_identifier::TokenIdentifier;
use tonic::Request;
use tonic::metadata::MetadataValue;
use tonic::transport::{Certificate, Channel, ClientTlsConfig, Uri};
use tracing;

pub fn current_epoch_time_in_seconds() -> u64 {
    let now = SystemTime::now();
    let since_the_epoch = now.duration_since(UNIX_EPOCH).unwrap();
    since_the_epoch.as_secs()
}

fn install_rustls_provider() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        rustls::crypto::ring::default_provider()
            .install_default()
            .expect("install rustls crypto provider");
    });
}

#[derive(Clone)]
pub struct SparkClient {
    client: SparkServiceClient<Channel>,
    keypair: Keypair,
    authn_client: SparkAuthnServiceClient<Channel>,
    token_client: SparkTokenServiceClient<Channel>,
    session_tokens: HashMap<PublicKey, SparkAuthSession>,
    operator_public_keys: Vec<PublicKey>,
}

#[derive(Clone, Debug)]
pub struct SparkClientConfig {
    pub coordinator_url: String,
    pub certificate_path: String,
    pub operator_public_keys: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct GetSparkAddressDataRequest {
    pub spark_address: String,
}

#[derive(Clone, Debug)]
pub struct GetSparkAddressDataResponse {
    pub token_outputs: Vec<SparkTokenOutput>,
}

#[derive(Clone, Debug)]
pub struct SparkTokenOutput {
    pub token_identifier: TokenIdentifier,
    pub amount: u128,
    pub prev_token_transaction_hash: Vec<u8>,
    pub prev_token_transaction_vout: u32,
}

#[derive(Clone, Debug)]
pub struct SparkAuthSession {
    pub session_token: String,
    pub expiration_time: u64,
}

impl SparkClient {
    pub async fn new(config: SparkClientConfig) -> Result<Self, SparkClientError> {
        install_rustls_provider();
        let channel = create_tls_channel(config.clone()).await?;
        let operator_public_keys = config
            .operator_public_keys
            .iter()
            .map(|key| PublicKey::from_str(key))
            .collect::<Result<Vec<PublicKey>, _>>()
            .map_err(|e| SparkClientError::DecodeError(format!("Failed to decode operator public key: {}", e)))?;
        let token_client = SparkTokenServiceClient::new(channel.clone());
        Ok(Self {
            client: SparkServiceClient::new(channel.clone()),
            authn_client: SparkAuthnServiceClient::new(channel.clone()),
            token_client,
            session_tokens: HashMap::new(),
            keypair: Keypair::new(&Secp256k1::new(), &mut OsRng),
            operator_public_keys,
        })
    }

    pub fn get_operator_public_keys(&self) -> Vec<PublicKey> {
        self.operator_public_keys.clone()
    }

    pub async fn authenticate(&mut self, keypair: Keypair) -> Result<SparkAuthSession, SparkClientError> {
        let need_to_authenticate = match self.session_tokens.get(&keypair.public_key()) {
            Some(session) => session.expiration_time >= current_epoch_time_in_seconds(),
            None => true,
        };

        if need_to_authenticate {
            let response = self
                .authn_client
                .get_challenge(GetChallengeRequest {
                    public_key: keypair.public_key().serialize().to_vec(),
                })
                .await
                .map_err(Box::new)?
                .into_inner();

            let protected_challenge = response.protected_challenge;
            let challenge = protected_challenge
                .clone()
                .ok_or(SparkClientError::DecodeError("Challenge is not found".to_string()))?
                .challenge
                .ok_or(SparkClientError::DecodeError("Challenge is not found".to_string()))?;

            let message_hash = sha256::Hash::hash(challenge.encode_to_vec().as_slice());
            let message = BitcoinMessage::from_digest(*message_hash.as_byte_array());

            let secp = Secp256k1::new();
            let signature = secp.sign_schnorr_no_aux_rand(&message, &keypair);

            let response = self
                .authn_client
                .verify_challenge(VerifyChallengeRequest {
                    protected_challenge,
                    signature: signature.serialize().to_vec(),
                    public_key: keypair.public_key().serialize().to_vec(),
                })
                .await
                .map_err(Box::new)?
                .into_inner();

            self.session_tokens.insert(
                keypair.public_key(),
                SparkAuthSession {
                    session_token: response.session_token,
                    expiration_time: response.expiration_timestamp as u64,
                },
            );
        }

        self.session_tokens
            .get(&keypair.public_key())
            .cloned()
            .ok_or(SparkClientError::SessionTokenNotFound(format!(
                "Session token not found for public key: {}",
                keypair.public_key()
            )))
    }

    pub async fn get_spark_address_data(
        &mut self,
        request: GetSparkAddressDataRequest,
    ) -> Result<GetSparkAddressDataResponse, SparkClientError> {
        tracing::debug!("Getting spark address data for {}", request.spark_address);

        let session_token = self.authenticate(self.keypair).await?;

        let address_data = decode_spark_address(&request.spark_address)?;
        let public_key = hex::decode(address_data.identity_public_key).unwrap();

        let request = QueryTokenOutputsRequest {
            owner_public_keys: vec![public_key],
            token_identifiers: vec![],
            token_public_keys: vec![],
            network: 2, // Regtest, search spark_network_to_proto_network function
        };

        let mut request = Request::new(request);
        create_request(&mut request, self.keypair.public_key(), session_token)?;

        let response = self
            .client
            .query_token_outputs(request)
            .await
            .map_err(Box::new)?
            .into_inner();

        let mut token_outputs = vec![];
        for output in response.outputs_with_previous_transaction_data {
            let inner_output = output
                .output
                .ok_or(SparkClientError::DecodeError("Output is not found".to_string()))?;

            let new_token_output = SparkTokenOutput {
                token_identifier: TokenIdentifier::from_bytes(
                    inner_output
                        .token_identifier
                        .as_ref()
                        .ok_or(SparkClientError::DecodeError(
                            "Token identifier is not found".to_string(),
                        ))?
                        .as_slice(),
                )?,
                amount: u128::from_be_bytes(
                    inner_output
                        .token_amount
                        .clone()
                        .try_into()
                        .map_err(|_| SparkClientError::DecodeError("Failed to decode token amount".to_string()))?,
                ),
                prev_token_transaction_hash: output.previous_transaction_hash,
                prev_token_transaction_vout: output.previous_transaction_vout,
            };
            token_outputs.push(new_token_output);
        }

        Ok(GetSparkAddressDataResponse { token_outputs })
    }

    pub async fn start_spark_transaction(
        &mut self,
        request: StartTransactionRequest,
        keypair: Keypair,
    ) -> Result<StartTransactionResponse, SparkClientError> {
        let session_token = self.authenticate(keypair).await?;

        let mut request = Request::new(request);
        create_request(&mut request, keypair.public_key(), session_token)?;

        let response = self
            .token_client
            .start_transaction(request)
            .await
            .map_err(Box::new)?
            .into_inner();

        Ok(response)
    }

    pub async fn commit_spark_transaction(
        &mut self,
        request: CommitTransactionRequest,
        keypair: Keypair,
    ) -> Result<CommitTransactionResponse, SparkClientError> {
        let session_token = self.authenticate(keypair).await?;

        let mut request = Request::new(request);
        create_request(&mut request, keypair.public_key(), session_token)?;

        let response = self
            .token_client
            .commit_transaction(request)
            .await
            .map_err(Box::new)?
            .into_inner();

        Ok(response)
    }
}

async fn create_tls_channel(config: SparkClientConfig) -> Result<Channel, SparkClientError> {
    let uri = Uri::from_str(config.coordinator_url.as_ref())
        .map_err(|e| SparkClientError::CreateTlsChannelError(format!("Failed to create URI: {}", e)))?;
    let mut tls = ClientTlsConfig::new();
    let certificate = Certificate::from_pem(std::fs::read(config.certificate_path.clone()).unwrap());
    tls = tls.ca_certificate(certificate);
    if let Some(host) = uri.host() {
        tls = tls.domain_name(host);
    }

    let channel = Channel::from_shared(uri.to_string())
        .map_err(|e| SparkClientError::ConnectionError(format!("Failed to create channel: {}", e)))?
        .tls_config(tls)
        .map_err(|e| SparkClientError::ConnectionError(format!("Failed to create TLS config: {}", e)))?
        .connect()
        .await
        .map_err(|e| SparkClientError::ConnectionError(format!("Failed to connect to operator: {}", e)))?;

    Ok(channel)
}

pub fn create_request<T>(
    request: &mut tonic::Request<T>,
    user_public_key: PublicKey,
    spark_session: SparkAuthSession,
) -> Result<(), SparkClientError> {
    let identity_public_key_str = hex::encode(user_public_key.serialize());
    let id_meta = MetadataValue::try_from(identity_public_key_str).unwrap();
    request.metadata_mut().insert("x-identity-public-key", id_meta);

    let session_token = MetadataValue::try_from(spark_session.session_token)
        .map_err(|e| SparkClientError::DecodeError(format!("Failed to decode session token: {}", e)))?;
    request.metadata_mut().insert("authorization", session_token);

    Ok(())
}
