use artemis_core::types::{Collector, CollectorStream};
use async_trait::async_trait;
use ethers::{
    providers::{Middleware, PubsubClient},
    types::H160,
};
use futures::StreamExt;
use tokio_stream::wrappers::ReceiverStream;

use super::StateSpaceManager;

#[async_trait]
impl<M, P> Collector<Vec<H160>> for StateSpaceManager<M, P>
where
    M: Middleware,
    M::Error: 'static,
    P: Middleware + 'static,
    P::Provider: PubsubClient,
{
    /// Artemis collector implementation for state space manager.
    ///
    /// Returns a `CollectorStream` of `Vec<H160>` representing the AMM addresses that incurred a state change in the block.
    async fn get_event_stream(&self) -> anyhow::Result<CollectorStream<'_, Vec<H160>>> {
        let (state_change_rx, mut join_handles) = self.subscribe_state_changes().await?;

        let stream_handle = join_handles.swap_remove(0);
        let state_change_handle = join_handles.swap_remove(0);

        let early_handle_exit = async move {
            tokio::select! {
                result = stream_handle => {
                  if let Err(e) = result {
                      tracing::error!(?e, "Stream buffer exited early");
                  }
                },
                result = state_change_handle => {
                    if let Err(e) = result {
                        tracing::error!(?e, "State change handler exited early");
                    }
                }
            }
        };

        let stream = ReceiverStream::new(state_change_rx).take_until(early_handle_exit);

        Ok(Box::pin(stream) as CollectorStream<'_, Vec<H160>>)
    }
}
