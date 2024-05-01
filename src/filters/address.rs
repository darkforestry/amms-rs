use crate::amm::{AutomatedMarketMaker, AMM};
use alloy::primitives::Address;
use std::collections::HashSet;

/// Filters out AMMs that contain a blacklisted token.
pub fn filter_blacklisted_tokens(amms: Vec<AMM>, blacklisted_addresses: Vec<Address>) -> Vec<AMM> {
    let mut filtered_pools = vec![];
    let blacklist: HashSet<Address> = blacklisted_addresses.into_iter().collect();

    for amm in amms {
        let mut blacklisted_token_in_amm = false;
        for token in amm.tokens() {
            if blacklist.contains(&token) {
                blacklisted_token_in_amm = true;
                break;
            }
        }

        if !blacklisted_token_in_amm {
            filtered_pools.push(amm);
        }
    }

    filtered_pools
}

/// Filters out AMMs where the AMM address is a blacklisted address.
pub fn filter_blacklisted_amms(amms: Vec<AMM>, blacklisted_addresses: Vec<Address>) -> Vec<AMM> {
    let mut filtered_amms = vec![];
    let blacklist: HashSet<Address> = blacklisted_addresses.into_iter().collect();

    for amm in amms {
        if !blacklist.contains(&amm.address()) {
            filtered_amms.push(amm);
        }
    }

    filtered_amms
}

/// Filters out AMMs where AMM address or any tokens in the AMM are in the blacklist.
pub fn filter_blacklisted_addresses(
    amms: Vec<AMM>,
    blacklisted_addresses: Vec<Address>,
) -> Vec<AMM> {
    let mut filtered_amms = vec![];
    let blacklist: HashSet<Address> = blacklisted_addresses.into_iter().collect();

    for amm in amms {
        let mut blacklisted_address_in_amm = false;

        //check if any of the tokens are on the blacklisted address
        for token in amm.tokens() {
            if blacklist.contains(&token) {
                blacklisted_address_in_amm = true;
            }
        }

        //Check if the amm address is blacklisted
        if blacklist.contains(&amm.address()) {
            blacklisted_address_in_amm = true;
        }

        //If there are no blacklisted addresses, add the amm to the filtered amms
        if !blacklisted_address_in_amm {
            filtered_amms.push(amm);
        }
    }

    filtered_amms
}
