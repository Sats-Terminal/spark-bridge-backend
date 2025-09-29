use spark_protos::spark::spark_service_client::SparkServiceClient;
use spark_protos::spark_authn::spark_authn_service_client::SparkAuthnServiceClient;
use spark_protos::spark_authn::{VerifyChallengeRequest, GetChallengeRequest};
use tonic::transport::{Channel, ClientTlsConfig, Uri, Certificate};
use crate::error::SparkClientError;
use std::str::FromStr;
use spark_protos::spark::QueryTokenOutputsRequest;
use spark_address::{decode_spark_address};
use tracing;
use hex;
use token_identifier::TokenIdentifier;
use std::sync::Once;
use rustls;
use std::time::{SystemTime, UNIX_EPOCH};
use bitcoin::key::Keypair;
use bitcoin::secp256k1::{Secp256k1, Message as BitcoinMessage};
use bitcoin::hashes::{sha256, Hash};
use rand_core::OsRng;
use spark_protos::prost::Message;
use tonic::Request;
use tonic::metadata::MetadataValue;
use bitcoin::secp256k1::PublicKey;

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
    session_token: Option<SparkAuthSession>,
}

#[derive(Clone, Debug)]
pub struct SparkClientConfig {
    pub base_url: String,
    pub certificate_path: String,
}

#[derive(Clone, Debug)]
pub struct GetSparkAddressDataRequest {
    pub spark_address: String,
}

#[derive(Clone, Debug)]
pub struct GetSparkAddressDataResponse {
    pub token_identifier: TokenIdentifier,
    pub amount: u128,
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
        Ok(Self { 
            client: SparkServiceClient::new(channel.clone()), 
            authn_client: SparkAuthnServiceClient::new(channel.clone()), 
            session_token: None,
            keypair: Keypair::new(&Secp256k1::new(), &mut OsRng),
        })
    }

    pub async fn authenticate(&mut self) -> Result<SparkAuthSession, SparkClientError> {
        let need_to_authenticate = match self.session_token.clone() {
            Some(session) => {
                if session.expiration_time < current_epoch_time_in_seconds() {
                    false
                } else {
                    true
                }
            }
            None => true,
        };

        if need_to_authenticate {
            let response = self.authn_client.get_challenge(GetChallengeRequest {
                public_key: self.keypair.public_key().serialize().to_vec(),
            }).await?.into_inner();

            let protected_challenge = response.protected_challenge;
            let challenge = protected_challenge
                .clone()
                .ok_or(SparkClientError::DecodeError("Challenge is not found".to_string()))?
                .challenge
                .ok_or(SparkClientError::DecodeError("Challenge is not found".to_string()))?;

            let message_hash = sha256::Hash::hash(challenge.encode_to_vec().as_slice());
            let message = BitcoinMessage::from_digest(message_hash.as_byte_array().clone());
        
            let secp = Secp256k1::new();
            let signature = secp.sign_schnorr_no_aux_rand(&message, &self.keypair);

            let response = self.authn_client.verify_challenge(VerifyChallengeRequest {
                protected_challenge,
                signature: signature.serialize().to_vec(),
                public_key: self.keypair.public_key().serialize().to_vec(),
            }).await?.into_inner();

            self.session_token = Some(SparkAuthSession {
                session_token: response.session_token,
                expiration_time: response.expiration_timestamp as u64,
            });
        }

        self.session_token.clone().ok_or(SparkClientError::DecodeError("Session token is not found".to_string()))
    }

    pub async fn get_spark_address_data(&mut self, request: GetSparkAddressDataRequest) -> Result<GetSparkAddressDataResponse, SparkClientError> {
        tracing::debug!("Getting spark address data for {}", request.spark_address);

        self.authenticate().await?;

        let address_data = decode_spark_address(&request.spark_address)?;
        let public_key = hex::decode(address_data.identity_public_key).unwrap();

        let request = QueryTokenOutputsRequest {
            owner_public_keys: vec![public_key],
            token_identifiers: vec![],
            token_public_keys: vec![],
            network: 2, // Regtest, search spark_network_to_proto_network function
        };

        let mut request = Request::new(request);
        create_request(&mut request, self.keypair.public_key(), self.session_token.clone().unwrap())?;

        let response = self.client.query_token_outputs(
            request
        ).await?.into_inner();

        assert_eq!(response.outputs_with_previous_transaction_data.len(), 1);
        let output = response.outputs_with_previous_transaction_data[0].output.as_ref().unwrap();

        let token_identifier = TokenIdentifier::from_bytes(output.token_identifier.as_ref().unwrap()).unwrap();
        let amount = u128::from_be_bytes(output.token_amount.clone().try_into().unwrap());

        tracing::debug!("Token identifier: {:?}", token_identifier);
        tracing::debug!("Amount: {:?}", amount);

        Ok(GetSparkAddressDataResponse {
            token_identifier,
            amount,
        })
    }
}

async fn create_tls_channel(config: SparkClientConfig) -> Result<Channel, SparkClientError> {
    let uri = Uri::from_str(config.base_url.as_ref())
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
    let id_meta = MetadataValue::try_from(identity_public_key_str).unwrap(); // TODO: handle error
    request
        .metadata_mut()
        .insert("x-identity-public-key", id_meta);

    let session_token = MetadataValue::try_from(spark_session.session_token)
        .map_err(|e| SparkClientError::DecodeError(format!("Failed to decode session token: {}", e)))?;
    request.metadata_mut().insert("authorization", session_token);

    Ok(())
}
