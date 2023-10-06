use amms::discovery::{self, erc_4626::Erc4626DiscoveryOptionsBuilder};
use ethers::providers::{Http, Provider};
use std::sync::Arc;

#[tokio::main]
async fn main() -> eyre::Result<()> {
    //Add rpc endpoint here:
    let rpc_endpoint = std::env::var("ETHEREUM_RPC_ENDPOINT")?;
    let provider = Arc::new(Provider::<Http>::try_from(rpc_endpoint)?);

    let options = Erc4626DiscoveryOptionsBuilder::default()
        .from_block(10000835)
        .step(30000)
        .build()?;
    //discover vaults
    let _vaults = discovery::erc_4626::discover_erc_4626_vaults(provider, Some(options)).await?;

    Ok(())
}
