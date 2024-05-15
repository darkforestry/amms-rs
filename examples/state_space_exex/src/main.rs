use amms::state_space::StateSpaceManager;
use reth::builder::FullNodeComponents;
use reth_exex::ExExContext;
use reth_node_ethereum::EthereumNode;
use std::future::Future;

async fn init_exex<Node: FullNodeComponents>(
    ctx: ExExContext<Node>,
) -> eyre::Result<impl Future<Output = eyre::Result<()>>> {
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
    state_space_manager: StateSpaceManager,
) -> eyre::Result<()> {
    while let Some(notification) = ctx.notifications.recv().await {
        // match notification {

        // }
    }

    Ok(())
}
