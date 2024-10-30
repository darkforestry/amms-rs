use tokio::{
    net::TcpListener,
    sync::mpsc,
    time::{sleep, Duration},
};

use std::sync::Arc;

use alloy::{primitives::address, providers::ProviderBuilder};

use amms::{
    amm::{
        factory::Factory, uniswap_v2::factory::UniswapV2Factory,
        uniswap_v3::factory::UniswapV3Factory,
    },
    sync,
};

#[derive(Debug)]
enum Message {
    Status(String),
    Tick(u64),
}

#[tokio::main]
async fn main() {
    let (tx, mut rx) = mpsc::channel::<Message>(32);
    let tx_clone = tx.clone();

    // Background task.
    tokio::spawn(async move {
        let mut counter = 0;
        loop {
            // Add rpc endpoint here:
            // let rpc_endpoint = std::env::var("ETHEREUM_RPC_ENDPOINT").unwrap();
            // let provider = Arc::new(ProviderBuilder::new().on_http(rpc_endpoint.parse().unwrap()));
            //
            // let factories = vec![
            //     // Add UniswapV3
            //     Factory::UniswapV3Factory(UniswapV3Factory::new(
            //         address!("1F98431c8aD98523631AE4a59f267346ea31F984"),
            //         12369621,
            //     )),
            // ];

            // sync::sync_amms(factories, provider, None, 500)
            //     .await
            //     .unwrap();

            counter += 1;
            tx_clone
                .send(Message::Tick(counter))
                .await
                .expect("Failed to send tick");
            sleep(Duration::from_secs(5)).await;
        }
    });

    let listener = TcpListener::bind("127.0.0.1:8080")
        .await
        .expect("Failed to bind to address");
    println!("Server listening on 127.0.0.1:8080");

    tx.send(Message::Status("Server started".to_string()))
        .await
        .expect("Failed to send status");

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
                    Message::Status(status) => println!("Status update: {}", status),
                    Message::Tick(count) => println!("Background task tick: {}", count),
                }
            }
        }
    }
}
