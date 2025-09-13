use gateway_rune_transfer::transfer::{create_rune_transfer, sign_message_hash, add_signature_to_transaction, create_message_hash};
use bitcoin::key::PrivateKey;
use bitcoin::secp256k1::SecretKey;
use bitcoin::address::Address;
use bitcoin::secp256k1::Secp256k1;
use bitcoin::network::Network;
use bitcoin::hashes::sha256d::Hash as Sha256dHash;
use bitcoin::consensus::Encodable;
use ordinals::RuneId;
use bitcoin::{OutPoint, Txid};

#[test]
fn test_rune_transfer() {
    let ctx = Secp256k1::new();
    let network = Network::Regtest;

    let rune_id = RuneId::new(18417, 1).unwrap();

    let private_key = PrivateKey::from_wif("cSYFixQzjSrZ4b4LBT16Q7RXBk52DZ5cpJydE7DzuZS1RhzaXpEN").unwrap();
    let secret_key = SecretKey::from_slice(&private_key.to_bytes()).unwrap();
    let public_key = private_key.public_key(&ctx);
    let (xonly_public_key, _) = secret_key.x_only_public_key(&ctx);

    let address = Address::p2tr(
        &ctx,
        xonly_public_key,
        None,
        Network::Regtest,
    );

    println!("Address: {}", address.to_string());

    let mut txid_bytes: [u8; 32] = hex::decode("8aa2e9a78cc0ece70fc8fdc2472d4ad8bf9724a4bf459e0539590953e0ef86fa").unwrap().try_into().unwrap();
    let txid = *Sha256dHash::from_bytes_mut(&mut txid_bytes);

    let outputs_to_spend = vec![
        OutPoint {
            txid: Txid::from_raw_hash(txid),
            vout: 1,
        },
        OutPoint {
            txid: Txid::from_raw_hash(txid),
            vout: 2,
        },
    ];

    let output_addresses = vec![address.clone(), address.clone()];
    let output_sats_amounts = vec![49_990_000, 49_990_000];
    let output_runes_amounts = vec![500_000, 500_000];

    let mut transaction = create_rune_transfer(outputs_to_spend, output_addresses.clone(), output_sats_amounts.clone(), output_runes_amounts.clone(), rune_id).unwrap();

    // for the test we use the same addresses in the input and the output
    let previous_output_addresses = vec![address.clone(), address.clone()];
    let previous_output_sats_amounts = vec![546, 99994454];

    let message_hashes = vec![
        create_message_hash(&transaction, &previous_output_addresses, &previous_output_sats_amounts, 0).unwrap(),
        create_message_hash(&transaction, &previous_output_addresses, &previous_output_sats_amounts, 1).unwrap(),
    ];

    let signatures = vec![
        sign_message_hash(message_hashes[0], secret_key),
        sign_message_hash(message_hashes[1], secret_key),
    ];

    add_signature_to_transaction(&mut transaction, 0, signatures[0]);
    add_signature_to_transaction(&mut transaction, 1, signatures[1]);

    // Convert transaction to raw hex string
    let mut raw_bytes = Vec::new();
    transaction.consensus_encode(&mut raw_bytes).unwrap();
    let raw_transaction = hex::encode(&raw_bytes);
    
    println!("Raw transaction: {}", raw_transaction);
    println!("Transaction ID: {}", transaction.compute_txid());
}
