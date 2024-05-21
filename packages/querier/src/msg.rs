use cosmwasm_std::Uint128;
use models::asset::Asset;
use models::asset_info::AssetInfo;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

// ********** LP ************* //

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PairQueryMsg {
    Pair {},
    Simulation { offer_asset: Asset },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct PairInfo {
    pub asset_infos: Vec<AssetInfo>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct PairSimulationResponse {
    pub return_amount: Uint128,
}

// ********** CW20 ************* //

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Cw20QueryMsg {
    Balance { address: String },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct Cw20BalanceResponse {
    pub balance: Uint128,
}

// ********** Helix ************* //
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryHelix {
    Market {
        market_id: String,
    },
    Simulation {
        offer_asset: Asset,
        market_id: String,
    },
}

// ********** Astrovault ************* //
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryAstrovault {
    PoolInfo {},
    Pair {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryAstrovaultStable {
    SwapSimulation {
        amount: Uint128,
        swap_from_asset_index: u32,
        swap_to_asset_index: u32,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryAstrovaultHybrid {
    SwapSimulation {
        amount: Uint128,
        swap_from_asset_index: u32,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct QueryAstrovaultResponse {
    pub asset_infos: Vec<AssetInfo>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct QueryAstrovaultStableSimulationResponse {
    pub swap_to_assets_amount: Vec<Uint128>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct QueryAstrovaultHybridSimulationResponse {
    pub to_amount_minus_fee: Uint128,
}

// ********** OraiDex ************* //
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryOraiDexV2 {
    Pair {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct QueryOraiDexV2PairResponse {
    pub info: PairInfo,
}
