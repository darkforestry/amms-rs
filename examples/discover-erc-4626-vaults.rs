use std::{error::Error, sync::Arc};

use ethers::providers::{Http, Provider};

use damms::discovery;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    //Add rpc endpoint here:
    let rpc_endpoint =
        std::env::var("ETHEREUM_RPC_ENDPOINT").expect("Could not get ETHEREUM_RPC_ENDPOINT");
    let provider = Arc::new(Provider::<Http>::try_from(rpc_endpoint).unwrap());

    //discover vaults
    let _vaults = discovery::erc_4626::discover_erc_4626_vaults(provider,30000).await?;

    Ok(())
}
