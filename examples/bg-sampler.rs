use futures::future::join_all;
use tokio::{
    net::TcpListener,
    sync::mpsc,
    time::{sleep, Duration},
};

use std::sync::Arc;

use alloy::{
    primitives::{address, Address},
    providers::{Provider, ProviderBuilder},
    rpc::client::WsConnect,
};

use amms::{
    amm::{
        factory::Factory,
        uniswap_v2::factory::UniswapV2Factory,
        uniswap_v3::{factory::UniswapV3Factory, UniswapV3Pool},
        AMM,
    },
    state_space::StateSpaceManager,
    sync,
};

#[derive(Debug)]
enum Message {
    Status(String),
    Tick(u64),
    Pools(Vec<UniswapV3Pool>),
    StateChange(Vec<Address>),
}

const WETH_USDC_POOLS: [Address; 4] = [
    address!("E0554a476A092703abdB3Ef35c80e0D76d32939F"), // 1bps
    address!("88e6a0c2ddd26feeb64f039a2c41296fcb3f5640"), // 5bps
    address!("8ad599c3A0ff1De082011EFDDc58f1908eb6e6D8"), // 30bps
    address!("7BeA39867e4169DBe237d55C8242a8f2fcDcc387"), // 100bps
];

const FACTORY_ADDRESS: Address = address!("1F98431c8aD98523631AE4a59f267346ea31F984");

#[tokio::main]
async fn main() {
    let (tx, mut rx) = mpsc::channel::<Message>(32);
    let tx_clone = tx.clone();

    // Background task.
    tokio::spawn(async move {
        let mut counter = 0;
        let mut cached_pools = None;

        let rpc_endpoint = std::env::var("RPC_HTTP").unwrap();
        let http_provider = Arc::new(ProviderBuilder::new().on_http(rpc_endpoint.parse().unwrap()));
        let ws_endpoint = std::env::var("RPC_WS").unwrap();
        let ws = WsConnect::new(ws_endpoint);
        let ws_provider = Arc::new(ProviderBuilder::new().on_ws(ws).await.unwrap());

        let current_block = http_provider
            .get_block_number()
            .await
            .expect("Failed to get current block");
        println!("Current block number: {}", current_block);

        if cached_pools.is_none() {
            let mut futures = Vec::with_capacity(WETH_USDC_POOLS.len());
            for p in WETH_USDC_POOLS.iter().skip(1).cloned() {
                let handle = UniswapV3Pool::new_from_address(
                    p,
                    None,
                    current_block as u64 - 1000,
                    Arc::clone(&http_provider),
                );
                futures.push(handle);
            }

            let pools = join_all(futures)
                .await
                .into_iter()
                .filter_map(|result| {
                    if let Err(ref e) = result {
                        println!("Pool initialization error: {:?}", e);
                        None
                    } else {
                        result.ok()
                    }
                })
                .collect::<Vec<_>>();

            println!("Initialized {} pools", pools.len());

            tx_clone
                .send(Message::Pools(
                    pools.clone().into_iter().map(|p| p.0).collect(),
                ))
                .await
                .expect("failed to send pools");
            cached_pools = Some(pools);
        }

        let last_synced_block = cached_pools
            .as_ref()
            .unwrap()
            .iter()
            .min_by(|x, y| x.1.cmp(&y.1))
            .unwrap()
            .1;
        println!("Starting from block: {}", last_synced_block);

        let amms = cached_pools
            .as_ref()
            .unwrap()
            .into_iter()
            .map(|(p, _)| AMM::UniswapV3Pool(p.clone()))
            .collect();
        let state_space_manager = StateSpaceManager::new(amms, ws_provider);

        let (mut rx, _join_handles) = state_space_manager
            .subscribe_state_changes(
                current_block as u64 - 100, // Start from 100 blocks ago
                10,                         // Reduce batch size for more frequent updates
            )
            .await
            .unwrap();

        println!("Subscribed to state changes");

        loop {
            if let Some(state_changes) = rx.recv().await {
                println!("Received state change: {:?}", &state_changes);
                tx_clone
                    .send(Message::StateChange(state_changes))
                    .await
                    .expect("failed to send state changes");
            } else {
                println!("No state changes received");
                sleep(Duration::from_secs(1)).await;
            }
        }
    });

    let listener = TcpListener::bind("127.0.0.1:8080")
        .await
        .expect("Failed to bind to address");
    println!("Server listening on 127.0.0.1:8080");

    // Main task.
    loop {
        tokio::select! {
            Ok((socket, addr)) = listener.accept() => {
                println!("New connection from: {}", addr);
                tx.send(Message::Status(format!("New connection from: {}", addr)))
                    .await
                    .expect("Failed to send connection status");

                tokio::spawn(async move {
                    println!("Handling connection from: {}", addr);
                });
            }

            Some(message) = rx.recv() => {
                match message {
                    Message::Status(status) => println!("Status update: {}",status),
                    Message::Tick(count) => println!("Background task tick: {}",count),
                    Message::Pools(ref pools) => {
                        println!("pools are here! {:?}", pools.len());
                    },
                    Message::StateChange(ref addresses) => {
                        println!("these changed: {addresses:?}");
                    }
                }
            }
        }
    }
}
