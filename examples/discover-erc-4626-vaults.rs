use std::sync::Arc;

use alloy::providers::ProviderBuilder;

use amms::discovery;

#[tokio::main]
async fn main() -> eyre::Result<()> {
    tracing_subscriber::fmt::init();

    // Add rpc endpoint here:
    let rpc_endpoint = std::env::var("ETHEREUM_RPC_ENDPOINT")?;
    let provider = Arc::new(ProviderBuilder::new().on_http(rpc_endpoint.parse()?));

    // discover vaults
    let vaults = discovery::erc_4626::discover_erc_4626_vaults(provider, 30000).await?;

    println!("Vaults: {:?}", vaults);

    Ok(())
}
