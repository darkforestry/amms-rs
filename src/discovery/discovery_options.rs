use derive_builder::Builder;

/// Factories discovery options
#[derive(Debug, Builder, Default)]
pub struct FactoriesDiscoveryOptions {
    /// From block number, if None then the discovery start block number will be 0
    #[builder(default = "0")]
    pub from_block: u64,
    /// To block number, if None then the discovery end block number will be the current block number
    #[builder(default)]
    pub to_block: Option<u64>,
    /// Block number step
    #[builder(default = "1000")]
    pub step: u64,
    /// Filter factory that have to has at least a number of pairs
    #[builder(default = "10")]
    pub number_of_amms_threshold: u64,
}

/// ERC4626 vault discovery options
#[derive(Debug, Builder, Default)]
pub struct Erc4626DiscoveryOptions {
    /// From block number, if None then the discovery start block number will be 0
    #[builder(default = "0")]
    pub from_block: u64,
    /// To block number, if None then the discovery end block number will be the current block number
    #[builder(default)]
    pub to_block: Option<u64>,
    /// Block number step
    #[builder(default = "1000")]
    pub step: u64,
}
