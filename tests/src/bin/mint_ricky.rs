use std::env;
use std::str::FromStr;
use std::time::Duration;

use bitcoin::Address;
use bitcoin::Network;
use tests::bitcoin_client::{BitcoinClient, BitcoinClientConfig};
use tests::constants::DEFAULT_FAUCET_AMOUNT;
use tests::rune_manager::mint_specific_rune;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: cargo run -p tests --bin mint_ricky -- <bcrt1-destination-address>");
        return Ok(());
    }

    let destination_unchecked = Address::from_str(&args[1])?;
    let destination = destination_unchecked.require_network(Network::Regtest)?;
    let rune_name = env::var("RUNE_NAME").unwrap_or_else(|_| "RICKY".to_string());

    let mut bitcoin_client = BitcoinClient::new(BitcoinClientConfig {
        bitcoin_url: "http://127.0.0.1:18443".to_string(),
        titan_url: "http://127.0.0.1:3030".to_string(),
        bitcoin_username: "bitcoin".to_string(),
        bitcoin_password: "bitcoinpass".to_string(),
    })?;

    println!("Funding temporary taproot address with {DEFAULT_FAUCET_AMOUNT} sats...");
    let bitcoin_client_clone = bitcoin_client.clone();

    let (rune_id, mint_txid) = mint_specific_rune(bitcoin_client_clone, &rune_name, destination.clone()).await?;

    let rune_info = {
        let mut info = None;
        let mut last_err = None;
        for _ in 0..20 {
            match bitcoin_client.get_rune(rune_id.to_string()).await {
                Ok(r) => {
                    info = Some(format!("{r:#?}"));
                    break;
                }
                Err(e) => {
                    last_err = Some(e);
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            }
        }
        info.unwrap_or_else(|| match last_err {
            Some(e) => format!("Failed to fetch rune info after retries: {e}"),
            None => "Rune info fetched successfully".to_string(),
        })
    };

    println!("Minted rune '{rune_name}' with id {rune_id} to {}", destination);
    println!("Mint transaction id: {mint_txid}");
    println!("Rune info: {rune_info}");

    Ok(())
}
