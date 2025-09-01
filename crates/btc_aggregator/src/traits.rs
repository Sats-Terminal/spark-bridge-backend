// Will be on gateway

struct Aggregator {
    verifiers: Hashmap<Identifier, SingerClinet> // always 3
    users: Hashmap<UserId(String), UserSpesificInfo> // database that contains users keypackage and signing process
}

pub trait Aggregator {
    fn check_user_id(&self, user_id: &str) -> bool;

    fn run_dkg_flow(&self, user_id: &str) -> Result<PublicKeyPackage, Error>;

    fn run_signing_flow(&self, user_id: &str, message: &[u8]) -> Result<Signature, Error>;

    fn get_public_key_package(&self, user_id: &str) -> Result<PublicKeyPackage, Error>;

}

pub trait SignerClient {
    pub fn dkg_round_1(&self, Metadata1) -> Result<Metadata, Error>;

    pub fn dkg_round_2(&self, Metadata2) -> Result<Metadata, Error>;

    pub fn dkg_round_3(&self, Metadata3) -> Result<Metadata, Error>;

    pub fn sign_round_1(&self, Metadata1) -> Result<Metadata, Error>;

    pub fn sign_round_2(&self, Metadata2) -> Result<Metadata, Error>;
    
}


// Will be on verificator

pub struct VerificatorSigner {
    users: Hashmap<UserId(String), UserSpesificInfo> // database that contains users keypackage and signing process
}
