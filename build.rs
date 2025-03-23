use std::{fs, path::PathBuf, process::Command};

const TARGET_CONTRACTS: &[&str] = &[
    "GetERC4626VaultDataBatchRequest",
    "GetTokenDecimalsBatchRequest",
    "GetBalancerPoolDataBatchRequest",
    "WethValueInPools",
    "WethValueInPoolsBatchRequest",
    "GetUniswapV2PairsBatchRequest",
    "GetUniswapV2PoolDataBatchRequest",
    "GetUniswapV3PoolDataBatchRequest",
    "GetUniswapV3PoolSlot0BatchRequest",
    "GetUniswapV3PoolTickBitmapBatchRequest",
    "GetUniswapV3PoolTickDataBatchRequest",
];

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    let status = Command::new("forge")
        .arg("build")
        .current_dir("contracts")
        .status()?;

    if !status.success() {
        panic!("forge build failed");
    }

    let forge_out_dir = manifest_dir.join("contracts/out");
    let abi_out_dir = manifest_dir.join("src/amms/abi/");
    fs::create_dir_all(&abi_out_dir)?;

    for contract in TARGET_CONTRACTS {
        let json_file = forge_out_dir
            .join(format!("{contract}.sol"))
            .join(format!("{contract}.json"));
        let dest_file = abi_out_dir.join(format!("{contract}.json"));
        fs::copy(&json_file, &dest_file)?;
    }

    Ok(())
}
