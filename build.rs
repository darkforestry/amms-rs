use std::{
    fs,
    hash::{self, DefaultHasher, Hash, Hasher},
    path::PathBuf,
    process::Command,
};

use serde_json::Value;

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
        let new_abi = forge_out_dir
            .join(format!("{contract}.sol"))
            .join(format!("{contract}.json"));

        let prev_abi = abi_out_dir.join(format!("{contract}.json"));
        if !prev_abi.exists() {
            fs::copy(&new_abi, &prev_abi)?;
            continue;
        }

        let prev_contents: Value = serde_json::from_str(&fs::read_to_string(&prev_abi)?)?;
        let prev_bytecode = prev_contents["bytecode"]
            .as_str()
            .expect("Could not get previous bytecode");

        let new_contents: Value = serde_json::from_str(&fs::read_to_string(&new_abi)?)?;
        let new_bytecode = new_contents["bytecode"]
            .as_str()
            .expect("Could not get new bytecode");

        let prev_bytecode_hash = hash(prev_bytecode);
        let new_bytecode_hash = hash(new_bytecode);

        if prev_bytecode_hash != new_bytecode_hash {
            fs::copy(&new_abi, &prev_abi)?;
        }
    }

    Ok(())
}

fn hash(value: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}
