use alloy::{
    primitives::{address, Address},
    providers::ProviderBuilder,
    rpc::client::ClientBuilder,
    transports::layers::{RetryBackoffLayer, ThrottleLayer},
};
use alloy_provider::WsConnect;
use amms::{
    amms::{uniswap_v2::UniswapV2Pool, uniswap_v3::UniswapV3Pool},
    state_space::{hooks::StateHook, StateSpaceBuilder},
};
use futures::StreamExt;
use std::sync::Arc;
use tracing::Level;

#[tokio::main]
async fn main() -> eyre::Result<()> {
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();
    let rpc_endpoint = std::env::var("ETHEREUM_PROVIDER_WS")?;

    let client = ClientBuilder::default()
        .layer(ThrottleLayer::new(500))
        .layer(RetryBackoffLayer::new(5, 200, 330))
        .ws(WsConnect::new(rpc_endpoint))
        .await?;

    let sync_provider = Arc::new(ProviderBuilder::new().connect_client(client));

    let amms = vec![
        UniswapV2Pool::new(address!("B4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc"), 300).into(),
        UniswapV3Pool::new(address!("88e6A0c2dDD26FEEb64F039a2c41296FcB3f5640")).into(),
    ];

    let state_space_manager = StateSpaceBuilder::new(sync_provider.clone())
        .with_amms(amms)
        // Enables background snapshotting
        // See: [`hooks::SnapshotConfig`] for configuration options
        .with_snapshot_enabled(None)
        // start syncing from a snapshotted state
        // .with_snapshot_path("./snapshots/<Your snapshot>") --- IGNORE ---
        // Registers hooks to be called after every block is processed
        .with_hooks(vec![simple_counter()])
        .sync()
        .await?;

    let mut stream = state_space_manager.subscribe().await?;
    let _res = stream.next().await.iter().take(20);

    std::fs::remove_dir_all("./snapshots")?;

    Ok(())
}

pub fn simple_counter() -> StateHook<Vec<Address>> {
    let count = Arc::new(std::sync::atomic::AtomicU64::new(0));

    let hook = move |updated_pools: &Vec<Address>| {
        let total = count.fetch_add(
            updated_pools.len() as u64,
            std::sync::atomic::Ordering::Relaxed,
        );

        println!(
            "total AMMs updated so far: {}",
            total + updated_pools.len() as u64
        );
    };

    Arc::new(hook)
}
