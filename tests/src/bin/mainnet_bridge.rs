use anyhow::{Context, Result};
use clap::Parser;
use std::io::{self, Write};
use tests::gateway_client::{
    BridgeRunesSparkRequest, CachedRuneMetadata, GatewayClient, GatewayConfig, GetRunesDepositAddressRequest,
};
use tokio::time::{Duration, sleep};
use url::Url;

#[derive(Parser, Debug)]
#[command(author, version, about = "Interactive helper for bridging mainnet runes into wRunes")]
struct Args {
    /// Gateway HTTP base URL
    #[arg(long, default_value = "http://localhost:8060")]
    gateway_url: String,

    /// Compressed secp256k1 public key (hex) that controls the rune deposit
    #[arg(long)]
    user_public_key: String,

    /// Rune identifier (e.g. 840000:42)
    #[arg(long)]
    rune_id: String,

    /// Amount of rune units to bridge
    #[arg(long)]
    amount: u64,

    /// Spark address that should receive the minted wRunes
    #[arg(long)]
    spark_address: String,

    /// Seconds between metadata polling attempts
    #[arg(long, default_value_t = 60)]
    poll_interval_secs: u64,

    /// Maximum seconds to wait for metadata cache to update
    #[arg(long, default_value_t = 10800)]
    max_wait_secs: u64,

    /// Skip metadata polling once the bridge request is submitted
    #[arg(long)]
    skip_metadata_wait: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let gateway_url = Url::parse(&args.gateway_url).context("Failed to parse gateway URL")?;
    let client = GatewayClient::new(GatewayConfig { address: gateway_url });

    println!("Requesting Bitcoin deposit address for rune {} ...", args.rune_id);
    let deposit_response = client
        .get_runes_deposit_address(GetRunesDepositAddressRequest {
            user_public_key: args.user_public_key.clone(),
            rune_id: args.rune_id.clone(),
            amount: args.amount,
        })
        .await
        .context("Failed to request rune deposit address")?;

    println!();
    println!("===================================================================");
    println!("Rune deposit address: {}", deposit_response.address);
    println!("Requested amount     : {}", args.amount);
    println!("Target Spark address : {}", args.spark_address);
    println!("===================================================================");
    println!();
    println!(
        "1. Send at least the requested amount of rune {} to the deposit address above.",
        args.rune_id
    );
    println!("2. Wait for the transaction to receive the confirmations your policy requires.");
    println!("3. When ready, press ENTER to continue.");
    wait_for_enter()?;

    let (txid, vout) = prompt_for_outpoint()?;

    println!("Submitting bridge request for txid {}:{}", txid, vout);
    client
        .bridge_runes(BridgeRunesSparkRequest {
            btc_address: deposit_response.address.clone(),
            bridge_address: args.spark_address.clone(),
            txid: txid.clone(),
            vout,
        })
        .await
        .context("Failed to submit bridge request")?;

    println!("Bridge request accepted. The gateway will mint wRunes after verifiers confirm the deposit.");

    if args.skip_metadata_wait {
        println!("Skipping metadata polling as requested.");
        return Ok(());
    }

    println!(
        "Waiting for cached metadata entry for rune {} (polling every {}s, timeout {}s)...",
        args.rune_id, args.poll_interval_secs, args.max_wait_secs
    );
    let poll_interval = Duration::from_secs(args.poll_interval_secs);
    let max_wait = Duration::from_secs(args.max_wait_secs);

    match wait_for_metadata(&client, &args.rune_id, poll_interval, max_wait).await? {
        Some(metadata) => {
            println!();
            println!("wRune metadata registered in gateway database:");
            print_cached_metadata(&metadata)?;
            println!();
            println!(
                "You can now check your Spark wallet ({}) for the minted wRunes.",
                args.spark_address
            );
        }
        None => {
            println!("Timed out waiting for metadata cache to include rune {}.", args.rune_id);
            println!("The bridge may still complete shortly; inspect gateway and verifier logs for progress.");
        }
    }

    Ok(())
}

fn wait_for_enter() -> Result<()> {
    let mut buffer = String::new();
    print!("Press ENTER to continue... ");
    io::stdout().flush().ok();
    buffer.clear();
    io::stdin()
        .read_line(&mut buffer)
        .context("Failed to read line from stdin")?;
    Ok(())
}

fn prompt_for_outpoint() -> Result<(String, u32)> {
    loop {
        let mut buffer = String::new();
        print!("Enter confirmed deposit outpoint as <txid>:<vout>: ");
        io::stdout().flush().ok();
        buffer.clear();
        io::stdin()
            .read_line(&mut buffer)
            .context("Failed to read txid:vout from stdin")?;
        let trimmed = buffer.trim();
        if trimmed.is_empty() {
            println!("Input cannot be empty.");
            continue;
        }
        let (txid, vout_str) = match trimmed.split_once(':') {
            Some(parts) => parts,
            None => {
                println!("Expected format <txid>:<vout>. Please try again.");
                continue;
            }
        };
        if txid.len() != 64 || !txid.chars().all(|c| c.is_ascii_hexdigit()) {
            println!("Invalid txid format. Provide a 64-character hex string.");
            continue;
        }
        match vout_str.parse::<u32>() {
            Ok(vout) => {
                return Ok((txid.to_lowercase(), vout));
            }
            Err(_) => {
                println!("vout must be an unsigned integer. Please try again.");
            }
        }
    }
}

async fn wait_for_metadata(
    client: &GatewayClient,
    rune_id: &str,
    poll_interval: Duration,
    max_wait: Duration,
) -> Result<Option<CachedRuneMetadata>> {
    let mut elapsed = Duration::ZERO;
    while elapsed <= max_wait {
        match client.list_wrune_metadata().await {
            Ok(list) => {
                if let Some(entry) = list.into_iter().find(|item| item.rune_id == rune_id) {
                    return Ok(Some(entry));
                }
            }
            Err(err) => {
                eprintln!("Failed to fetch metadata map: {err}");
            }
        }
        sleep(poll_interval).await;
        elapsed += poll_interval;
    }
    Ok(None)
}

fn print_cached_metadata(metadata: &CachedRuneMetadata) -> Result<()> {
    println!("Rune ID          : {}", metadata.rune_id);
    println!("Issuer musig key : {}", metadata.issuer_public_key);
    println!("Bitcoin network  : {}", metadata.bitcoin_network);
    println!("Spark network    : {}", metadata.spark_network);
    println!("Created at       : {}", metadata.created_at);
    println!("Updated at       : {}", metadata.updated_at);

    if let Some(rune) = &metadata.rune_metadata {
        match serde_json::to_string_pretty(rune) {
            Ok(pretty) => {
                println!("Rune metadata    :\n{}", indent_block(&pretty));
            }
            Err(err) => {
                println!("Rune metadata    : <failed to format> ({err})");
            }
        }
    } else {
        println!("Rune metadata    : (not provided by Maestro)");
    }

    match serde_json::to_string_pretty(&metadata.wrune_metadata) {
        Ok(pretty) => {
            println!("wRune metadata   :\n{}", indent_block(&pretty));
        }
        Err(err) => {
            println!("wRune metadata   : <failed to format> ({err})");
        }
    }

    Ok(())
}

fn indent_block(block: &str) -> String {
    let indent = "    ";
    block
        .lines()
        .map(|line| format!("{indent}{line}"))
        .collect::<Vec<_>>()
        .join("\n")
}
