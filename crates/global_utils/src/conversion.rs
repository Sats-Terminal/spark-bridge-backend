use bitcoin::{Address, Network};
use std::str::FromStr;

pub fn decode_address(address: &str, network: Network) -> Result<Address, String> {
    let address = Address::from_str(address).map_err(|e| format!("Failed to decode address: {}", e))?;
    let address = address
        .require_network(network)
        .map_err(|e| format!("Failed to check network: {}", e))?;
    Ok(address)
}
