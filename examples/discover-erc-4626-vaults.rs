use amms::discovery;
use ethers::providers::{Http, Provider};
use std::sync::Arc;

#[tokio::main]
async fn main() -> eyre::Result<()> {
    tracing_subscriber::fmt::init();

    //Add rpc endpoint here:
    let rpc_endpoint = std::env::var("ETHEREUM_RPC_ENDPOINT")?;
    let provider = Arc::new(Provider::<Http>::try_from(rpc_endpoint)?);

    //discover vaults
    let vaults = discovery::erc_4626::discover_erc_4626_vaults(provider, 30000).await?;

    println!("Vaults: {:?}", vaults);

    Ok(())
}
