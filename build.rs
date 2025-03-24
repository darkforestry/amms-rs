use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use serde_json::Value;
use std::{
    fs,
    hash::{DefaultHasher, Hash, Hasher},
    path::PathBuf,
    process::Command,
};

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

    TARGET_CONTRACTS.par_iter().for_each(|contract| {
        let new_abi = forge_out_dir
            .join(format!("{contract}.sol"))
            .join(format!("{contract}.json"));
        let prev_abi = abi_out_dir.join(format!("{contract}.json"));

        if !prev_abi.exists() {
            fs::copy(&new_abi, &prev_abi).unwrap();
            return;
        }

        let prev_contents: Value =
            serde_json::from_str(&fs::read_to_string(&prev_abi).unwrap()).unwrap();
        let new_contents: Value =
            serde_json::from_str(&fs::read_to_string(&new_abi).unwrap()).unwrap();

        let prev_bytecode = prev_contents["bytecode"]["object"]
            .as_str()
            .expect("Missing prev bytecode");
        let new_bytecode = new_contents["bytecode"]["object"]
            .as_str()
            .expect("Missing new bytecode");

        if hash(prev_bytecode) != hash(new_bytecode) {
            fs::copy(&new_abi, &prev_abi).unwrap();
        }
    });

    println!("cargo:rerun-if-changed=contracts");

    Ok(())
}

fn hash(value: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}
