use alloy::primitives::{address, U256};

use amms::amms::uniswap_v2::UniswapV2Pool;

#[tokio::main]
async fn main() -> eyre::Result<()> {
    // Initialize the pool
    let pool = UniswapV2Pool {
        address: address!("B4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc"),
        token_a: address!("6B175474E89094C44Da98b954EedeAC495271d0F"),
        token_a_decimals: 18,
        token_b: address!("EeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE"),
        token_b_decimals: 18,
        reserve_0: 1e24 as u128,
        reserve_1: 1e24 as u128,
        fee: 300,
    };

    // Generate the swap calldata
    let to_address = address!("DecafC0ffee15BadDecafC0ffee15BadDecafC0f");
    let swap_calldata = pool.swap_calldata(U256::from(10000), U256::ZERO, to_address, vec![]);

    println!("Swap calldata: {:?}", swap_calldata);

    Ok(())
}
