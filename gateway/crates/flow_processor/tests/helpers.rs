use bitcoin::{secp256k1, Network};
use frost::types::MusigId;
use gateway_config_parser::config::VerifierConfig;
use uuid::Uuid;

pub fn create_test_issuer_musig_id() -> MusigId {
    use bitcoin::secp256k1::PublicKey;
    use std::str::FromStr;

    MusigId::Issuer {
        issuer_public_key: PublicKey::from_str(
            "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798"
        ).unwrap(),
        rune_id: "840000:1".to_string(),
    }
}

pub fn create_test_user_musig_id() -> MusigId {
    use bitcoin::secp256k1::PublicKey;
    use std::str::FromStr;

    MusigId::User {
        user_public_key: PublicKey::from_str(
            "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798"
        ).unwrap(),
        rune_id: "840000:1".to_string(),
    }
}

pub fn create_test_musig_id_with_pubkey(pubkey: &str, is_issuer: bool) -> MusigId {
    use bitcoin::secp256k1::PublicKey;
    use std::str::FromStr;

    let public_key = PublicKey::from_str(pubkey).unwrap();

    if is_issuer {
        MusigId::Issuer {
            issuer_public_key: public_key,
            rune_id: "840000:1".to_string(),
        }
    } else {
        MusigId::User {
            user_public_key: public_key,
            rune_id: "840000:1".to_string(),
        }
    }
}

pub fn create_test_verifier_configs() -> Vec<VerifierConfig> {
    vec![
        VerifierConfig {
            id: 1,
            address: "http://localhost:8081".to_string(),
        },
        VerifierConfig {
            id: 2,
            address: "http://localhost:8082".to_string(),
        },
        VerifierConfig {
            id: 3,
            address: "http://localhost:8083".to_string(),
        },
    ]
}

pub fn create_test_btc_address(network: Network) -> bitcoin::Address {
    use bitcoin::key::Secp256k1;
    use bitcoin::secp256k1::rand::thread_rng;
    use bitcoin::{CompressedPublicKey, PrivateKey};

    let secp = Secp256k1::new();
    let private_key = PrivateKey::new(secp256k1::SecretKey::new(&mut thread_rng()), network);
    let public_key = CompressedPublicKey::from_private_key(&secp, &private_key).unwrap();

    bitcoin::Address::p2wpkh(&public_key, network)
}

pub fn create_test_spark_address() -> String {
    "sprt1pgssy7d7vel0nh9m4326qc54e6rskpczn07dktww9rv4nu5ptvt0s9ucd5rgc0".to_string()
}

pub async fn wait_for_completion_with_timeout(
    duration: std::time::Duration,
) -> Result<(), tokio::time::error::Elapsed> {
    tokio::time::timeout(duration, tokio::time::sleep(duration)).await
}

pub async fn assert_channel_closed<T>(mut receiver: tokio::sync::mpsc::Receiver<T>) {
    assert!(
        receiver.recv().await.is_none(),
        "Expected channel to be closed"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_test_issuer_musig_id() {
        let musig_id = create_test_issuer_musig_id();
        match musig_id {
            MusigId::Issuer { issuer_public_key, rune_id } => {
                assert_eq!(
                    issuer_public_key.to_string(),
                    "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798"
                );
                assert_eq!(rune_id, "840000:1");
            }
            _ => panic!("Expected MusigId::Issuer"),
        }
    }

    #[test]
    fn test_create_test_user_musig_id() {
        let musig_id = create_test_user_musig_id();
        match musig_id {
            MusigId::User { user_public_key, rune_id } => {
                assert_eq!(
                    user_public_key.to_string(),
                    "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798"
                );
                assert_eq!(rune_id, "840000:1");
            }
            _ => panic!("Expected MusigId::User"),
        }
    }

    #[test]
    fn test_create_musig_id_with_custom_pubkey() {
        let custom_pubkey = "02c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5";

        let issuer_id = create_test_musig_id_with_pubkey(custom_pubkey, true);
        assert!(matches!(issuer_id, MusigId::Issuer { .. }));

        let user_id = create_test_musig_id_with_pubkey(custom_pubkey, false);
        assert!(matches!(user_id, MusigId::User { .. }));
    }

    #[test]
    fn test_create_test_btc_address() {
        let address = create_test_btc_address(Network::Regtest);
        assert!(address.to_string().starts_with("bcrt1"));
    }

    #[test]
    fn test_create_verifier_configs() {
        let configs = create_test_verifier_configs();
        assert_eq!(configs.len(), 3);
        assert_eq!(configs[0].address, "http://localhost:8081/");
    }

    #[test]
    fn test_create_test_spark_address() {
        let address = create_test_spark_address();
        assert!(address.starts_with("sprt1"));
    }

    #[tokio::test]
    async fn test_assert_channel_closed() {
        let (tx, rx) = tokio::sync::mpsc::channel::<i32>(1);
        drop(tx); 
        assert_channel_closed(rx).await;
    }
}