use std::{error::Error, str::FromStr, sync::Arc};

use ethers::{
    providers::{Http, Provider, Ws},
    types::H160,
};

use damms::{
    amm::{
        factory::Factory, uniswap_v2::factory::UniswapV2Factory,
        uniswap_v3::factory::UniswapV3Factory,
    },
    sync,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    //Add rpc endpoint here:
    let rpc_endpoint =
        std::env::var("ETHEREUM_RPC_ENDPOINT").expect("Could not get ETHEREUM_RPC_ENDPOINT");
    let provider = Arc::new(Provider::<Http>::try_from(rpc_endpoint).unwrap());
    let ws_provider = Arc::new(Provider::<Ws>::connect(rpc_endpoint).await.unwrap());

    let factories = vec![
        //UniswapV2
        Factory::UniswapV2Factory(UniswapV2Factory::new(
            H160::from_str("0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f").unwrap(),
            2638438,
            300,
        )),
        //Add Sushiswap
        Factory::UniswapV2Factory(UniswapV2Factory::new(
            H160::from_str("0xC0AEe478e3658e2610c5F7A4A2E1777cE9e4f2Ac").unwrap(),
            10794229,
            300,
        )),
        //Add UniswapV3
        Factory::UniswapV3Factory(UniswapV3Factory::new(
            H160::from_str("0x1F98431c8aD98523631AE4a59f267346ea31F984").unwrap(),
            12369621,
        )),
    ];

    //Sync pairs
    let (amms, last_synced_block) = sync::sync_amms(factories, provider, None).await?;

    let state_space_manager = StateSpaceManager::new(amms, provider, ws_provider);

    Ok(())
}

use std::{
    collections::{HashMap, HashSet},
    sync::RwLock,
};

use arraydeque::ArrayDeque;
use damms::{
    amm::{AutomatedMarketMaker, AMM},
    errors::EventLogError,
};
use ethers::{
    providers::{Middleware, PubsubClient, StreamExt},
    types::{Block, Filter, Log, H256},
};
use tokio::{
    sync::mpsc::{Receiver, Sender},
    task::JoinHandle,
};

pub type StateSpace = HashMap<H160, AMM>;
pub type StateChangeCache = ArrayDeque<StateChange, 150>;

pub trait MiddlewarePubsub: Middleware {
    type PubsubProvider: 'static + PubsubClient;
}

impl<T> MiddlewarePubsub for T
where
    T: Middleware,
    T::Provider: 'static + PubsubClient,
{
    type PubsubProvider = T::Provider;
}

#[derive(Debug)]
pub struct StateSpaceManager<M, S>
where
    M: 'static + Middleware,
    S: 'static + MiddlewarePubsub,
{
    pub state: Arc<RwLock<StateSpace>>,
    pub middleware: Arc<M>,
    pub stream_middleware: Arc<S>,
}

impl<M, S> StateSpaceManager<M, S>
where
    M: Middleware,
    S: MiddlewarePubsub,
{
    pub fn new(amms: Vec<AMM>, middleware: Arc<M>, stream_middleware: Arc<S>) -> Self {
        let state: HashMap<H160, AMM> = amms
            .into_iter()
            .map(|amm| (amm.address(), amm))
            .collect::<HashMap<H160, AMM>>();

        Self {
            state: Arc::new(RwLock::new(state)),
            middleware,
            stream_middleware,
        }
    }

    pub fn get_block_filter(&self) -> Result<Filter, StateChangeError> {
        let mut event_signatures: Vec<H256> = vec![];
        let mut amm_variants = HashSet::new();

        for amm in self
            .state
            .read()
            .map_err(|_| StateChangeError::PoisonedLockOnState)?
            .values()
        {
            let variant = match amm {
                AMM::UniswapV2Pool(_) => 0,
                AMM::UniswapV3Pool(_) => 1,
                AMM::ERC4626Vault(_) => 2,
            };

            if !amm_variants.contains(&variant) {
                amm_variants.insert(variant);
                event_signatures.extend(amm.sync_on_event_signatures());
            }
        }

        //Create a new filter
        Ok(Filter::new().topic0(event_signatures))
    }

    //listens to new blocks and handles state changes, sending an h256 block hash when a new block is produced
    //pub fn listen_for_new_blocks()-> Result<Receiver<H256>, StateSpaceError<M>> {}
    pub async fn listen_for_new_blocks(
        &self,
        mut last_synced_block: u64,
        channel_buffer: usize,
    ) -> Result<
        (
            Receiver<Block<H256>>,
            Vec<JoinHandle<Result<(), StateSpaceError<M, S>>>>,
        ),
        StateSpaceError<M, S>,
    >
    where
        <S as Middleware>::Provider: PubsubClient,
    {
        let state = self.state.clone();
        let mut state_change_cache: StateChangeCache = ArrayDeque::new();
        let middleware = self.middleware.clone();
        let stream_middleware: Arc<S> = self.stream_middleware.clone();
        let filter = self.get_block_filter()?;

        let (stream_tx, mut stream_rx): (Sender<Block<H256>>, Receiver<Block<H256>>) =
            tokio::sync::mpsc::channel(channel_buffer);

        let stream_handle = tokio::spawn(async move {
            let mut block_stream = stream_middleware
                .subscribe_blocks()
                .await
                .map_err(StateSpaceError::PubsubClientError)?;

            while let Some(block) = block_stream.next().await {
                stream_tx.send(block).await?;
            }

            Ok::<(), StateSpaceError<M, S>>(())
        });

        let (new_block_tx, new_block_rx) = tokio::sync::mpsc::channel(channel_buffer);

        let new_block_handle: JoinHandle<Result<(), StateSpaceError<M, S>>> =
            tokio::spawn(async move {
                while let Some(block) = stream_rx.recv().await {
                    if let Some(chain_head_block_number) = block.number {
                        let chain_head_block_number = chain_head_block_number.as_u64();

                        //If there is a reorg, unwind state changes from last_synced block to the chain head block number
                        if chain_head_block_number <= last_synced_block {
                            unwind_state_changes(
                                state.clone(),
                                &mut state_change_cache,
                                chain_head_block_number,
                            )?;

                            //TODO: update this comment to explain why we set it to -1
                            last_synced_block = chain_head_block_number - 1;
                        }

                        let from_block: u64 = last_synced_block + 1;
                        let logs = middleware
                            .get_logs(
                                &filter
                                    .clone()
                                    .from_block(from_block)
                                    .to_block(chain_head_block_number),
                            )
                            .await
                            .map_err(StateSpaceError::MiddlewareError)?;

                        if logs.is_empty() {
                            for block_number in from_block..=chain_head_block_number {
                                add_state_change_to_cache(
                                    &mut state_change_cache,
                                    StateChange::new(None, block_number),
                                )?;
                            }
                        } else {
                            handle_state_changes_from_logs(
                                state.clone(),
                                &mut state_change_cache,
                                logs,
                            )?;
                        }

                        last_synced_block = chain_head_block_number;

                        new_block_tx.send(block).await?;
                    } else {
                        return Err(StateSpaceError::BlockNumberNotFound);
                    }
                }

                Ok::<(), StateSpaceError<M, S>>(())
            });

        Ok((new_block_rx, vec![stream_handle, new_block_handle]))
    }

    pub async fn listen_for_state_changes(
        &self,
        mut last_synced_block: u64,
        channel_buffer: usize,
    ) -> Result<
        (
            Receiver<Vec<H160>>,
            Vec<JoinHandle<Result<(), StateSpaceError<M, S>>>>,
        ),
        StateSpaceError<M, S>,
    >
    where
        <S as Middleware>::Provider: PubsubClient,
    {
        let state = self.state.clone();
        let mut state_change_cache: StateChangeCache = ArrayDeque::new();
        let middleware = self.middleware.clone();
        let stream_middleware: Arc<S> = self.stream_middleware.clone();
        let filter = self.get_block_filter()?;

        let (stream_tx, mut stream_rx): (Sender<Block<H256>>, Receiver<Block<H256>>) =
            tokio::sync::mpsc::channel(channel_buffer);

        let stream_handle = tokio::spawn(async move {
            let mut block_stream = stream_middleware
                .subscribe_blocks()
                .await
                .map_err(StateSpaceError::PubsubClientError)?;

            while let Some(block) = block_stream.next().await {
                stream_tx.send(block).await?;
            }

            Ok::<(), StateSpaceError<M, S>>(())
        });

        let (amms_updated_tx, amms_updated_rx) = tokio::sync::mpsc::channel(channel_buffer);

        let updated_amms_handle: JoinHandle<Result<(), StateSpaceError<M, S>>> =
            tokio::spawn(async move {
                while let Some(block) = stream_rx.recv().await {
                    if let Some(chain_head_block_number) = block.number {
                        let chain_head_block_number = chain_head_block_number.as_u64();

                        //If there is a reorg, unwind state changes from last_synced block to the chain head block number
                        if chain_head_block_number <= last_synced_block {
                            unwind_state_changes(
                                state.clone(),
                                &mut state_change_cache,
                                chain_head_block_number,
                            )?;

                            //set the last synced block to the head block number
                            last_synced_block = chain_head_block_number - 1;
                        }

                        let from_block: u64 = last_synced_block + 1;
                        let logs = middleware
                            .get_logs(
                                &filter
                                    .clone()
                                    .from_block(from_block)
                                    .to_block(chain_head_block_number),
                            )
                            .await
                            .map_err(StateSpaceError::MiddlewareError)?;

                        if logs.is_empty() {
                            for block_number in from_block..=chain_head_block_number {
                                add_state_change_to_cache(
                                    &mut state_change_cache,
                                    StateChange::new(None, block_number),
                                )?;
                            }
                        } else {
                            let amms_updated = handle_state_changes_from_logs(
                                state.clone(),
                                &mut state_change_cache,
                                logs,
                            )?;

                            amms_updated_tx.send(amms_updated).await?;
                        }

                        last_synced_block = chain_head_block_number;
                    } else {
                        return Err(StateSpaceError::BlockNumberNotFound);
                    }
                }

                Ok::<(), StateSpaceError<M, S>>(())
            });

        Ok((amms_updated_rx, vec![stream_handle, updated_amms_handle]))
    }
}

pub fn initialize_state_space(amms: Vec<AMM>) -> StateSpace {
    amms.into_iter()
        .map(|amm| (amm.address(), amm))
        .collect::<HashMap<H160, AMM>>()
}

#[derive(Debug)]
pub struct StateChange {
    pub state_change: Option<Vec<AMM>>,
    pub block_number: u64,
}

impl StateChange {
    pub fn new(state_change: Option<Vec<AMM>>, block_number: u64) -> Self {
        Self {
            block_number,
            state_change,
        }
    }
}

//Unwinds the state changes cache for every block from the most recent state change cache back to the block to unwind -1
fn unwind_state_changes(
    state: Arc<RwLock<StateSpace>>,
    state_change_cache: &mut StateChangeCache,
    block_to_unwind: u64,
) -> Result<(), StateChangeError> {
    //TODO: update this to use a range and not a loop
    loop {
        //check if the most recent state change block is >= the block to unwind,
        if let Some(state_change) = state_change_cache.get(0) {
            if state_change.block_number >= block_to_unwind {
                if let Some(option_state_changes) = state_change_cache.pop_front() {
                    if let Some(state_changes) = option_state_changes.state_change {
                        for amm_state in state_changes {
                            state
                                .write()
                                .map_err(|_| StateChangeError::PoisonedLockOnState)?
                                .insert(amm_state.address(), amm_state);
                        }
                    }
                } else {
                    //We know that there is a state change from state_change_cache.get(0) so when we pop front without returning a value, there is an issue
                    return Err(StateChangeError::PopFrontError);
                }
            } else {
                return Ok(());
            }
        } else {
            //We return an error here because we never want to be unwinding past where we have state changes.
            //For example, if you initialize a state space that syncs to block 100, then immediately after there is a chain reorg to 95, we can not roll back the state
            //changes for an accurate state space. In this case, we return an error
            return Err(StateChangeError::NoStateChangesInCache);
        }
    }
}

fn add_state_change_to_cache(
    state_change_cache: &mut StateChangeCache,
    state_change: StateChange,
) -> Result<(), StateChangeError> {
    if state_change_cache.is_full() {
        state_change_cache.pop_back();
        state_change_cache
            .push_front(state_change)
            .map_err(|_| StateChangeError::CapacityError)?
    } else {
        state_change_cache
            .push_front(state_change)
            .map_err(|_| StateChangeError::CapacityError)?
    }
    Ok(())
}

pub fn handle_state_changes_from_logs(
    state: Arc<RwLock<StateSpace>>,
    state_change_cache: &mut StateChangeCache,
    logs: Vec<Log>,
) -> Result<Vec<H160>, StateChangeError> {
    let mut updated_amms_set = HashSet::new();
    let mut updated_amms = vec![];
    let mut state_changes = vec![];

    let mut last_log_block_number = if let Some(log) = logs.get(0) {
        get_block_number_from_log(log)?
    } else {
        return Ok(updated_amms);
    };

    for log in logs.into_iter() {
        let log_block_number = get_block_number_from_log(&log)?;

        // check if the log is from an amm in the state space
        if let Some(amm) = state
            .write()
            .map_err(|_| StateChangeError::PoisonedLockOnState)?
            .get_mut(&log.address)
        {
            if !updated_amms_set.contains(&log.address) {
                updated_amms_set.insert(log.address);
                updated_amms.push(log.address);
            }

            state_changes.push(amm.clone());
            amm.sync_from_log(log)?;
        }

        //Commit state changes if the block has changed since last log
        if log_block_number != last_log_block_number {
            if state_changes.is_empty() {
                add_state_change_to_cache(
                    state_change_cache,
                    StateChange::new(None, last_log_block_number),
                )?;
            } else {
                add_state_change_to_cache(
                    state_change_cache,
                    StateChange::new(Some(state_changes), last_log_block_number),
                )?;
                state_changes = vec![];
            };

            last_log_block_number = log_block_number;
        }
    }

    if state_changes.is_empty() {
        add_state_change_to_cache(
            state_change_cache,
            StateChange::new(None, last_log_block_number),
        )?;
    } else {
        add_state_change_to_cache(
            state_change_cache,
            StateChange::new(Some(state_changes), last_log_block_number),
        )?;
    };

    Ok(updated_amms)
}

pub fn get_block_number_from_log(log: &Log) -> Result<u64, EventLogError> {
    if let Some(block_number) = log.block_number {
        Ok(block_number.as_u64())
    } else {
        Err(damms::errors::EventLogError::LogBlockNumberNotFound)
    }
}

use damms::errors::{ArithmeticError, DAMMError};

use ethers::prelude::{AbiError, ContractError};

use ethers::providers::ProviderError;

use ethers::signers::WalletError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum StateSpaceError<M, S>
where
    M: Middleware,
    S: MiddlewarePubsub,
{
    #[error("Middleware error")]
    MiddlewareError(<M as Middleware>::Error),
    #[error("Pubsub client error")]
    PubsubClientError(<S as Middleware>::Error),
    #[error("Provider error")]
    ProviderError(#[from] ProviderError),
    #[error("Contract error")]
    ContractError(#[from] ContractError<M>),
    #[error("ABI Codec error")]
    ABICodecError(#[from] AbiError),
    #[error("Eth ABI error")]
    EthABIError(#[from] ethers::abi::Error),
    #[error("CFMM error")]
    DAMMError(#[from] DAMMError<M>),
    #[error("Arithmetic error")]
    ArithmeticError(#[from] ArithmeticError),
    #[error("Wallet error")]
    WalletError(#[from] WalletError),
    #[error("Insufficient wallet funds for execution")]
    InsufficientWalletFunds(),
    #[error("Event log error")]
    EventLogError(#[from] EventLogError),
    #[error("State change error")]
    StateChangeError(#[from] StateChangeError),
    #[error("Block number not found")]
    BlockNumberNotFound,
    #[error("Could not send state changes through channel")]
    StateChangeSendError(#[from] tokio::sync::mpsc::error::SendError<Vec<H160>>),
    #[error("Could not send block hash through channel")]
    BlockHashSendError(#[from] tokio::sync::mpsc::error::SendError<Block<H256>>),
    #[error("Already listening for state changes")]
    AlreadyListeningForStateChanges,
}

#[derive(Error, Debug)]
pub enum StateChangeError {
    #[error("No state changes in cache")]
    NoStateChangesInCache,
    #[error("Error when removing a state change from the front of the deque")]
    PopFrontError,
    #[error("State change cache capacity error")]
    CapacityError,
    #[error("Poisoned RWLock on AMM state")]
    PoisonedLockOnState,
    #[error("Event log error")]
    EventLogError(#[from] EventLogError),
}
