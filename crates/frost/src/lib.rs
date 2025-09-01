use frost_secp256k1_tr::{
        keys::{dkg::{
        part1, part2, part3, 
        round1::{Package as Round1Package, SecretPackage as Round1SecretPackage},
        round2::{Package as Round2Package, SecretPackage as Round2SecretPackage},
    }, KeyPackage, PublicKeyPackage}, round1::{
        commit, SigningCommitments, SigningNonces
    }, Identifier, SigningPackage,
    round2::{
        sign, SignatureShare
    }, Signature
};
use eyre::{eyre, Result};
use std::collections::BTreeMap;
use rand_core::{OsRng, RngCore};

#[derive(Clone)]
pub struct Signer {
    rng: OsRng,
    identifier: Identifier,
    max_signers: u16,
    min_signers: u16,
    round1_secret_package: Option<Round1SecretPackage>,
    round1_packages: Option<BTreeMap<Identifier, Round1Package>>,
    round2_secret_package: Option<Round2SecretPackage>,
    key_package: Option<KeyPackage>,
    public_key_package: Option<PublicKeyPackage>,
    nonce: Option<SigningNonces>,
}

impl Signer {
    pub fn new(
        id: u16,
        max_signers: u16,
        min_signers: u16,
    ) -> Self {
        Self {
            rng: OsRng,
            max_signers,
            min_signers,
            identifier: id.try_into().unwrap(),
            round1_secret_package: None,
            round2_secret_package: None,
            round1_packages: None,
            key_package: None,
            public_key_package: None,
            nonce: None,
        }
    }

    pub fn get_identifier(&self) -> Identifier {
        self.identifier
    }

    pub fn dkg_part1(&mut self) -> Result<Round1Package> {
        let (secret_package, package) = part1(
            self.identifier, 
            self.max_signers, 
            self.min_signers, 
            self.rng
        )?;

        self.round1_secret_package = Some(secret_package);

        Ok(package)
    }

    pub fn dkg_part2(&mut self, round1_packages: &BTreeMap<Identifier, Round1Package>) -> Result<BTreeMap<Identifier, Round2Package>> {
        let round1_secret_package = self.round1_secret_package.clone()
            .ok_or(eyre!("Secret package not found"))?;
        let (secret_package, package) = part2(
            round1_secret_package,
            round1_packages,
        )?;

        self.round2_secret_package = Some(secret_package);
        self.round1_secret_package = None;
        self.round1_packages = Some(round1_packages.clone());

        Ok(package)
    }

    pub fn dkg_part3(&mut self, round2_packages: &BTreeMap<Identifier, Round2Package>) -> Result<PublicKeyPackage> {
        let round2_secret_package = self.round2_secret_package.clone()
            .ok_or(eyre!("Secret package not found"))?;
        let round1_packages = self.round1_packages.clone()
            .ok_or(eyre!("Round1 packages not found"))?;
        let (key_package, public_key_package) = part3(
            &round2_secret_package,
            &round1_packages,
            round2_packages,
        )?;

        self.round2_secret_package = None;
        self.round1_packages = None;
        self.key_package = Some(key_package);
        self.public_key_package = Some(public_key_package.clone());

        Ok(public_key_package)
    }

    pub fn sign_part1(&mut self) -> Result<SigningCommitments> {
        let key_package = self.key_package.clone().ok_or(eyre!("Key package not found"))?;
        let (nonce, commitment) = commit(
            key_package.signing_share(),
            &mut self.rng,
        );

        self.nonce = Some(nonce);

        Ok(commitment)
    }

    pub fn sign_part2(&mut self, signing_package: SigningPackage) -> Result<SignatureShare> {
        let key_package = self.key_package.clone().ok_or(eyre!("Key package not found"))?;
        let nonce = self.nonce.clone().ok_or(eyre!("Nonce not found"))?;
        let signature_share = sign(
            &signing_package,
            &nonce,
            &key_package,
        )?;

        self.nonce = None;

        Ok(signature_share)
    }

}

pub fn generate_random_messsage() -> [u8; 32] {
    let mut rng = OsRng;
    let mut message = [0; 32];
    rng.fill_bytes(&mut message);
    message
}

pub struct Aggregator {
    public_key_package: PublicKeyPackage,
    message: [u8; 32],
    signing_package: Option<SigningPackage>,
}

impl Aggregator {
    pub fn new(public_key_package: PublicKeyPackage, message: [u8; 32]) -> Self {
        Self { 
            public_key_package,
            message,
            signing_package: None,
        }
    }

    pub fn aggregate_part1(&mut self, signing_commitments: BTreeMap<Identifier, SigningCommitments>) -> Result<SigningPackage> {
        let signing_package = frost_secp256k1_tr::SigningPackage::new(
            signing_commitments,
            &self.message,
        );

        self.signing_package = Some(signing_package.clone());

        Ok(signing_package)
    }

    pub fn aggregate_part2(&mut self, signature_shares: BTreeMap<Identifier, SignatureShare>) -> Result<Signature> {
        let signing_package = self.signing_package.clone().ok_or(eyre!("Signing package not found"))?;
        let group_signature = frost_secp256k1_tr::aggregate(
            &signing_package,
            &signature_shares,
            &self.public_key_package,
        )?;

        self.signing_package = None;

        Ok(group_signature)
    }
}