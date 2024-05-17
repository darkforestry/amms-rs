use amms::state_space::StateSpaceManager;
use reth::builder::FullNodeComponents;
use reth_exex::ExExContext;
use reth_node_ethereum::EthereumNode;
use std::future::Future;

async fn init_exex<Node: FullNodeComponents>(
    ctx: ExExContext<Node>,
) -> eyre::Result<impl Future<Output = eyre::Result<()>>> {
    //TODO: load config

    let ws_endpoint = std::env::var("ETHEREUM_WS_ENDPOINT")?;

    // Initialize WS provider
    let ws = WsConnect::new(ws_endpoint);
    let provider = Arc::new(ProviderBuilder::new().on_ws(ws).await?);

    // Initialize factories
    let factories = vec![
        // Add UniswapV2
        Factory::UniswapV2Factory(UniswapV2Factory::new(
            address!("5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f"),
            2638438,
            300,
        )),
        // Add Sushiswap
        Factory::UniswapV2Factory(UniswapV2Factory::new(
            address!("C0AEe478e3658e2610c5F7A4A2E1777cE9e4f2Ac"),
            10794229,
            300,
        )),
    ];

    let step: u64 = 1000;

    // Sync amms
    let (mut amms, last_synced_block) =
        sync::sync_amms(factories, provider.clone(), None, step).await?;

    // Discover vaults and add them to amms
    let vaults = discovery::erc_4626::discover_erc_4626_vaults(provider.clone(), step)
        .await?
        .into_iter()
        .map(AMM::ERC4626Vault)
        .collect::<Vec<AMM>>();

    amms.extend(vaults);

    //TODO: init state space manager
    // state_space_exex(ctx, state_space_manager)
}

fn main() -> eyre::Result<()> {
    reth::cli::Cli::parse_args().run(|builder, _| async move {
        let handle = builder
            .node(EthereumNode::default())
            .install_exex(
                "StateSpaceManager",
                |ctx| async move { init_exex(ctx).await },
            )
            .launch()
            .await?;

        handle.wait_for_node_exit().await
    })
}

async fn state_space_exex<Node: FullNodeComponents>(
    mut ctx: ExExContext<Node>,
    mut state_space_manager: StateSpaceManager,
) -> eyre::Result<()> {
    while let Some(notification) = ctx.notifications.recv().await {
        match notification {}
    }

    Ok(())
}
