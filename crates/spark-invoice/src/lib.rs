//! Spark Invoice

use bitcoin::hashes::{Hash, HashEngine};
use bitcoin::hashes::sha256::Hash as Sha256Hash;
use bitcoin::Network;
use bitcoin::secp256k1::PublicKey;
use chrono::{DateTime, Utc};
use uuid::Uuid;
use token_identifier::TokenIdentifier;
use crate::proto::{decode_timestamp, read_bytes, read_len, read_string, read_varint_u32, read_varint_u64, skip_field, timestamp_to_datetime, write_len_prefixed_bytes, write_len_prefixed_str, write_timestamp, write_u128_be_bytes, write_varint_u32, write_varint_u64, DecodeError};

pub mod proto;

const INVOICE_VERSION_TAG: u8 = 0x08;
const INVOICE_ID_TAG: u8 = 0x12;
const INVOICE_MEMO_TAG: u8 = 0x2a;
const INVOICE_SENDER_PUBLIC_KEY_TAG: u8 = 0x32;
const INVOICE_EXPIRY_TIME_TAG: u8 = 0x3a;

const TOKENS_PAYMENT_TAG: u8 = 0x1a;
const TOKEN_IDENTIFIER_TAG: u8 = 0x0a;
const TOKEN_AMOUNT_TAG: u8 = 0x12;

const SATS_PAYMENT_TAG: u8 = 0x22;
const SATS_AMOUNT_TAG: u8 = 0x08;

/// Represents a payment in sats.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SatsPayment {
    /// Amount in sats.
    pub amount: u64,
}

impl SatsPayment {
    /// Encode sats payment into proto bytes.
    pub fn encode_proto(&self) -> Vec<u8> {
        let mut out = Vec::new();

        out.push(SATS_AMOUNT_TAG);
        write_varint_u64(&mut out, self.amount);

        out
    }

    /// Decode sats payment from proto bytes.
    pub fn decode_proto(input: &[u8]) -> Result<(Self, usize), DecodeError> {
        let mut pos = 0usize;
        let len = input.len();

        // default value if field missing (protobuf semantics)
        let mut amount_opt: Option<u64> = None;

        while pos < len {
            let tag = read_varint_u32(input, &mut pos)?;
            if tag == 0 { break; }
            let wt = tag & 7;

            if tag == (SATS_AMOUNT_TAG as u32) {
                // amount: varint (wire type 0)
                if wt != 0 { return Err(DecodeError::InvalidWireType); }
                let amount = read_varint_u64(input, &mut pos)?;
                amount_opt = Some(amount);
            } else {
                // unknown fields - skip for forward-compat
                skip_field(wt, input, &mut pos)?;
            }
        }

        let amount = amount_opt.ok_or(DecodeError::MissingField("Missing sats amount".to_string()))?;

        Ok((SatsPayment { amount }, pos))
    }
}

/// Represents a payment in tokens.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct TokensPayment {
    /// Token identifier.
    pub token_identifier: TokenIdentifier,
    /// Token amount.
    pub amount: u128,
}

impl TokensPayment {
    /// Encode token payment into proto bytes.
    pub fn encode_proto(&self) -> Vec<u8> {
        let mut out = Vec::new();

        out.push(TOKEN_IDENTIFIER_TAG);
        write_len_prefixed_bytes(&mut out, &self.token_identifier.to_bytes());

        out.push(TOKEN_AMOUNT_TAG);
        write_u128_be_bytes(&mut out, self.amount);

        out
    }

    /// Decode token payment from proto bytes.
    pub fn decode_proto(input: &[u8]) -> Result<(Self, usize), DecodeError> {
        let mut pos = 0usize;
        let len = input.len();

        let mut token_identifier_opt: Option<TokenIdentifier> = None;
        let mut amount_opt: Option<u128> = None;

        while pos < len {
            let tag = read_varint_u32(input, &mut pos)?;
            if tag == 0 { break; }
            let wt = tag & 7;

            if tag == (TOKEN_IDENTIFIER_TAG as u32) {
                if wt != 2 { return Err(DecodeError::InvalidWireType); }
                let l = read_len(input, &mut pos)?;
                let bytes = read_bytes(input, &mut pos, l)?;

                let token_identifier = TokenIdentifier::from_bytes(&bytes)
                    .map_err(|err| DecodeError::InvalidTokenIdentifier(err))?;

                token_identifier_opt = Some(token_identifier);
            } else if tag == (TOKEN_AMOUNT_TAG as u32) {
                if wt != 2 { return Err(DecodeError::InvalidWireType); }
                let l = read_len(input, &mut pos)?;
                let bytes = read_bytes(input, &mut pos, l)?;

                if bytes.len() > 16 {
                    return Err(DecodeError::InvalidU128BytesLength);
                }

                let mut v: u128 = 0;
                for &b in bytes {
                    v = (v << 8) | (b as u128);
                }
                amount_opt = Some(v);
            } else {
                skip_field(wt, input, &mut pos)?;
            }
        }

        let token_identifier = token_identifier_opt.ok_or(DecodeError::MissingField("Missing token identifier".to_string()))?;
        let amount = amount_opt.ok_or(DecodeError::MissingField("Missing token amount".to_string()))?;

        Ok((TokensPayment {
            token_identifier,
            amount,
        }, pos))
    }
}

/// Payment type: either sats or tokens.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum PaymentType {
    /// Sats-based payment.
    Sats(SatsPayment),
    /// Token-based payment.
    Tokens(TokensPayment),
}

/// Invoice fields for Spark invoices.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SparkInvoiceFields {
    /// Version number of the invoice format.
    pub version: u32,
    /// Unique identifier for the invoice.
    pub id: Uuid,
    /// Payment type (sats or tokens).
    pub payment_type: PaymentType,
    /// Memo attached to the invoice.
    pub memo: Option<String>,
    /// Public key of the sender.
    pub sender_public_key: Option<PublicKey>,
    /// Expiry time of the invoice.
    pub expiry_time: Option<DateTime<Utc>>,
}


impl SparkInvoiceFields {
    /// Encode invoice fields into proto bytes.
    pub fn encode_proto(&self) -> Vec<u8> {
        let mut out = Vec::new();

        out.push(INVOICE_VERSION_TAG);
        write_varint_u32(&mut out, self.version);

        out.push(INVOICE_ID_TAG);
        write_len_prefixed_bytes(&mut out, self.id.as_bytes());

        if let Some(memo) = &self.memo {
            out.push(INVOICE_MEMO_TAG);
            write_len_prefixed_str(&mut out, &memo);
        }

        if let Some(sender_public_key) = &self.sender_public_key {
            out.push(INVOICE_SENDER_PUBLIC_KEY_TAG);
            write_len_prefixed_bytes(&mut out, &sender_public_key.serialize());
        }

        if let Some(expiry_time) = &self.expiry_time {
            out.push(INVOICE_EXPIRY_TIME_TAG);

            let seconds = expiry_time.timestamp();
            let nanos = expiry_time.timestamp_subsec_nanos() as i32;

            let mut timestamp_bytes = vec![];
            write_timestamp(&mut timestamp_bytes, seconds, nanos);
            write_varint_u32(&mut out, timestamp_bytes.len() as u32);
            out.extend_from_slice(&timestamp_bytes);
        }

        match &self.payment_type {
            PaymentType::Sats(sats_payment) => {
                out.push(SATS_PAYMENT_TAG);

                let payment_bytes = sats_payment.encode_proto();

                write_varint_u32(&mut out, payment_bytes.len() as u32);
                out.extend_from_slice(&payment_bytes);
            }
            PaymentType::Tokens(tokens_payment) => {
                out.push(TOKENS_PAYMENT_TAG);

                let payment_bytes = tokens_payment.encode_proto();

                write_varint_u32(&mut out, payment_bytes.len() as u32);
                out.extend_from_slice(&payment_bytes);
            }
        }

        out
    }

    /// Decode invoice fields from proto bytes.
    pub fn decode_proto(input: &[u8]) -> Result<(Self, usize), DecodeError> {
        let mut pos = 0usize;
        let len = input.len();

        let mut version_opt = None;
        let mut id_opt = None;
        let mut payment_type_opt = None;
        let mut memo_opt = None;
        let mut sender_public_key_opt = None;
        let mut expiry_time_opt = None;

        while pos < len {
            let tag = read_varint_u32(input, &mut pos)?;
            if tag == 0 { break; }
            let field = tag >> 3;
            let wt = tag & 7;

            match field {
                1 => {
                    if wt != 0 { return Err(DecodeError::InvalidWireType); }
                    let version = read_varint_u32(input, &mut pos)?;
                    version_opt = Some(version);
                }
                2 => {
                    if wt != 2 { return Err(DecodeError::InvalidWireType); }
                    let l = read_len(input, &mut pos)?;
                    let id_bytes = read_bytes(input, &mut pos, l)?.to_vec();
                    let id_bytes: [u8; 16] = id_bytes.try_into().map_err(|_| DecodeError::InvalidUuidBytesLength)?;
                    let id = Uuid::from_bytes(id_bytes);

                    id_opt = Some(id);
                }
                3 => {
                    if wt != 2 { return Err(DecodeError::InvalidWireType); }
                    let l = read_len(input, &mut pos)?;
                    let bytes = read_bytes(input, &mut pos, l)?;
                    let (tokens, _) = TokensPayment::decode_proto(bytes)?;
                    payment_type_opt = Some(PaymentType::Tokens(tokens));
                }
                4 => {
                    if wt != 2 { return Err(DecodeError::InvalidWireType); }
                    let l = read_len(input, &mut pos)?;
                    let bytes = read_bytes(input, &mut pos, l)?;
                    let (sats, _) = SatsPayment::decode_proto(bytes)?;
                    payment_type_opt = Some(PaymentType::Sats(sats));
                }
                5 => {
                    if wt != 2 { return Err(DecodeError::InvalidWireType); }
                    let l = read_len(input, &mut pos)?;
                    memo_opt = Some(read_string(input, &mut pos, l)?);
                }
                6 => {
                    if wt != 2 { return Err(DecodeError::InvalidWireType); }
                    let l = read_len(input, &mut pos)?;
                    sender_public_key_opt = Some(read_bytes(input, &mut pos, l)?.to_vec());
                }
                7 => {
                    if wt != 2 { return Err(DecodeError::InvalidWireType); }
                    let l = read_len(input, &mut pos)?;
                    let bytes = read_bytes(input, &mut pos, l)?;
                    let ts = decode_timestamp(bytes)?;
                    expiry_time_opt = Some(timestamp_to_datetime(ts)?);
                }
                _ => {
                    skip_field(wt, input, &mut pos)?;
                }
            }
        }

        let version = version_opt.ok_or(DecodeError::MissingField("Missing version".to_string()))?;
        let id = id_opt.ok_or(DecodeError::MissingField("Missing id".to_string()))?;
        let payment_type = payment_type_opt.ok_or(DecodeError::MissingField("Missing payment type".to_string()))?;

        let sender_public_key_opt = sender_public_key_opt
            .map(|sender_public_key_bytes| PublicKey::from_slice(&sender_public_key_bytes))
            .transpose()
            .map_err(DecodeError::InvalidPublicKey)?;

        Ok((Self {
            version,
            id,
            payment_type,
            memo: memo_opt,
            sender_public_key: sender_public_key_opt,
            expiry_time: expiry_time_opt,
        }, pos))
    }
}

/// A Spark invoice with receiver, invoice fields, and network.
#[derive(Debug, Clone)]
pub struct SparkInvoice {
    /// Public key of the receiver.
    pub receiver: PublicKey,
    /// Associated invoice fields.
    pub invoice_fields: SparkInvoiceFields,
    /// Bitcoin network (mainnet, testnet, etc.).
    pub network: Network,
}

impl SparkInvoice {
    /// Create a new token-based Spark invoice.
    pub fn new_token_invoice(
        amount: u128,
        token_identifier: TokenIdentifier,
        memo: Option<String>,
        receiver_public_key: PublicKey,
        sender_public_key: Option<PublicKey>,
        expiry_time: Option<DateTime<Utc>>,
        network: Network,
    ) -> Self {
        let payment_type = PaymentType::Tokens(TokensPayment {
            token_identifier,
            amount,
        });

        let invoice_fields = SparkInvoiceFields {
            version: 1,
            id: Uuid::now_v7(),
            payment_type,
            memo,
            sender_public_key,
            expiry_time,
        };

        Self { receiver: receiver_public_key, invoice_fields, network }
    }

    /// Create a new sats-based Spark invoice.
    pub fn new_sats_invoice(
        amount: u64,
        memo: Option<String>,
        receiver_public_key: PublicKey,
        sender_public_key: Option<PublicKey>,
        expiry_time: Option<DateTime<Utc>>,
        network: Network,
    ) -> Self {
        let payment_type = PaymentType::Sats(SatsPayment {
            amount,
        });

        let invoice_fields = SparkInvoiceFields {
            version: 1,
            id: Uuid::now_v7(),
            payment_type,
            memo,
            sender_public_key,
            expiry_time,
        };

        Self { receiver: receiver_public_key, invoice_fields, network }
    }

    /// Create a new Spark invoice from invoice fields
    pub fn new_from_invoice_fields(
        receiver_public_key: PublicKey,
        fields: SparkInvoiceFields,
        network: Network,
    ) -> Self {
        Self { receiver: receiver_public_key, invoice_fields: fields, network }
    }

    /// Compute a unique SHA256 hash for the invoice.
    pub fn hash(&self) -> Sha256Hash {
        let mut hash_engine = Sha256Hash::engine();

        let SparkInvoiceFields { version, id, payment_type, memo, sender_public_key, expiry_time } = &self.invoice_fields;

        hash_engine.input(&Sha256Hash::hash(&version.to_be_bytes()).to_byte_array());

        hash_engine.input(&Sha256Hash::hash(&id.into_bytes()).to_byte_array());

        let network_magic: u32 = match self.network {
            Network::Bitcoin => 0xd9b4bef9,
            Network::Testnet => 0x0709110b,
            Network::Testnet4 => 0x0709110b,
            Network::Signet => 0x40cf030a,
            Network::Regtest => 0xdab5bffa
        };

        hash_engine.input(&Sha256Hash::hash(&network_magic.to_be_bytes()).to_byte_array());

        hash_engine.input(&Sha256Hash::hash(&self.receiver.serialize()).to_byte_array());

        match payment_type {
            PaymentType::Tokens(TokensPayment { token_identifier, amount }) => {
                let discriminator = 1;
                hash_engine.input(&Sha256Hash::hash(&[discriminator]).to_byte_array());

                hash_engine.input(&Sha256Hash::hash(token_identifier.as_byte_array()).to_byte_array());
                hash_engine.input(&Sha256Hash::hash(&amount.to_be_bytes()).to_byte_array());
            }
            PaymentType::Sats(SatsPayment { amount }) => {
                let discriminator = 2;
                hash_engine.input(&Sha256Hash::hash(&[discriminator]).to_byte_array());

                hash_engine.input(&Sha256Hash::hash(&amount.to_be_bytes()).to_byte_array());
            }
        }

        if let Some(memo) = memo {
            hash_engine.input(&Sha256Hash::hash(&memo.as_bytes()).to_byte_array());
        }

        if let Some(sender_public_key) = sender_public_key {
            hash_engine.input(&Sha256Hash::hash(&sender_public_key.serialize()).to_byte_array());
        }

        if let Some(expiry_time) = expiry_time {
            let expiry_time_seconds = expiry_time.timestamp() as u64;

            hash_engine.input(&Sha256Hash::hash(&expiry_time_seconds.to_be_bytes()).to_byte_array());
        }

        Sha256Hash::from_engine(hash_engine)
    }
}




