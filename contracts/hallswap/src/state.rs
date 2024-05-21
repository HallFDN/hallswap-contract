use cosmwasm_schema::cw_serde;
use cosmwasm_std::Addr;
use cw_storage_plus::Item;
use models::asset::Asset;

/// Stores the contract config at the given key
pub const CONFIG: Item<Config> = Item::new("config");

/// This structure holds the main parameters for the router
#[cw_serde]
pub struct Config {
    /// Address allowed to change this config
    pub owner: Addr,
    /// Address where all fees will go to
    pub fee_address: Addr,
    /// Fee amount in basis points
    pub fee_bps: u16,
    /// Valid assets that could be used as fees
    pub fee_assets: Vec<String>,
}

/// Tracks if user has paid fees during the swap
pub const FEES_COLLECTED: Item<Asset> = Item::new("fees_collected");
