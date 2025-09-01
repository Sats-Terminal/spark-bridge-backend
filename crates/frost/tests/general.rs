use eyre::{Result};
use frost_lib::{Signer, generate_random_messsage, Aggregator};
use std::collections::BTreeMap;
use frost_secp256k1_tr::Identifier;


#[test]
fn test_flow() -> Result<()> {
    // ------------ Setup Signers ------------
    let min_signers = 2;
    let max_signers = 3;

    let mut signers = (1..(max_signers+1))
        .map(|i| Signer::new(i, max_signers, min_signers))
        .collect::<Vec<_>>();

    let identifiers = signers
        .iter()
        .map(|signer| signer.get_identifier())
        .collect::<Vec<_>>();

    // ------------ DKG Round 1 ------------
    let mut round1_packages = BTreeMap::<Identifier, BTreeMap<Identifier, _>>::new();
    for (i, signer_i) in signers.iter_mut().enumerate() {
        let round1_package = signer_i.dkg_part1()?;
        for (j, identifier_j) in identifiers.iter().enumerate() {
            if i == j {
                continue;
            }
            round1_packages.entry(identifier_j.clone())
                .or_insert(BTreeMap::new())
                .insert(signer_i.get_identifier(), round1_package.clone());
        }
    }

    // ------------ DKG Round 2 ------------
    let mut round2_packages = BTreeMap::<Identifier, BTreeMap<Identifier, _>>::new();
    for signer_i in signers.iter_mut() {
        let new_round2_packages = signer_i.dkg_part2(round1_packages.get(&signer_i.get_identifier()).unwrap())?;
        
        for (receiver_identidier, package) in new_round2_packages {
            round2_packages.entry(receiver_identidier)
                .or_insert(BTreeMap::new())
                .insert(signer_i.get_identifier(), package);
        }
    }

    // ------------ DKG Round 3 ------------
    let mut public_key_packages = BTreeMap::new();
    for signer_i in signers.iter_mut() {
        let public_key_package = signer_i.dkg_part3(round2_packages.get(&signer_i.get_identifier()).unwrap())?;
        public_key_packages.insert(signer_i.get_identifier(), public_key_package);
    }

    // ------------ DKG Validate ------------
    let public_key_package = public_key_packages.get(&signers[0].get_identifier()).unwrap().clone();

    let public_key_encoded = hex::encode(public_key_package.verifying_key().serialize()?);
    println!("Public key encoded: {}", public_key_encoded);

    public_key_packages.iter().for_each(|(_, other_public_key_package)| {
        assert_eq!(&public_key_package, other_public_key_package);
    });

    // ------------ Setup Aggregator ------------
    let message = generate_random_messsage();
    let mut aggregator = Aggregator::new(public_key_package.clone(), message);

    let message_encoded = hex::encode(message);
    println!("Message encoded: {}", message_encoded);

    // ------------ Signing Round 1 ------------
    let mut signing_commitments = BTreeMap::new();
    for signer_i in signers.iter_mut() {
        let signing_commitment = signer_i.sign_part1()?;
        signing_commitments.insert(signer_i.get_identifier(), signing_commitment);
    }

    let signing_package = aggregator.aggregate_part1(signing_commitments)?;

    // ------------ Signing Round 2 ------------
    let mut signature_shares = BTreeMap::new();
    for signer_i in signers.iter_mut() {
        let signature_share = signer_i.sign_part2(signing_package.clone())?;
        signature_shares.insert(signer_i.get_identifier(), signature_share);
    }
    let group_signature = aggregator.aggregate_part2(signature_shares)?;

    let group_signature_encoded = hex::encode(group_signature.serialize()?);
    println!("Group signature encoded: {}", group_signature_encoded);

    // ------------ Signing Validate ------------

    let is_signature_valid = public_key_package
        .clone()
        .verifying_key()
        .verify(&message, &group_signature)
        .is_ok();

    assert!(is_signature_valid);

    let signature_length = group_signature.serialize()?.len();
    assert_eq!(signature_length, 64);

    Ok(())
}
