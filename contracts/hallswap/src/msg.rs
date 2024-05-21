use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{from_json, Addr, Binary, Decimal, QuerierWrapper, StdResult, Uint128};
use cw20::Cw20ReceiveMsg;
use models::asset::Asset;
use models::asset_info::AssetInfo;
use querier::msg::PairInfo;
use querier::querier::{
    query_astrovault_pair, query_astrovault_pool_info, query_market_info, query_orai_dex_v2_pair,
    query_pair_info,
};

#[cw_serde]
pub struct InstantiateMsg {
    pub owner: Option<Addr>,
    pub fee_address: Option<Addr>,
    pub fee_bps: Option<u16>,
    pub fee_assets: Option<Vec<String>>,
}

#[cw_serde]
pub struct RouteInfo {
    pub route: Vec<ContractInfo>,
    pub offer_amount: Uint128,
}

#[cw_serde]
pub struct ContractInfo {
    pub contract_addr: Addr,
    pub interface: Option<SwapInterface>,
}

#[cw_serde]
pub struct RouteInfoV2 {
    pub route: Vec<SwapOperation>,
    pub offer_amount: Uint128,
}

#[cw_serde]
pub struct SwapOperation {
    pub contract_addr: Addr,
    pub offer_asset: AssetInfo,
    pub return_asset: AssetInfo,
    pub interface: Option<Interface>,
}

#[cw_serde]
pub enum Interface {
    Binary(Binary),
    Struct(SwapInterface),
}

#[cw_serde]
pub enum PairType {
    Stable {},
    Xyk {},
    Hybrid {},
}

#[cw_serde]
pub enum SwapInterface {
    Astroport {},
    Astrovault { pair_type: PairType },
    Helix { market_id: String },
    OraiDexV2 {},
}

impl ContractInfo {
    // Defaults to Astroport
    pub fn interface(&self) -> SwapInterface {
        self.interface
            .clone()
            .unwrap_or(SwapInterface::Astroport {})
    }

    pub fn pair_info(&self, querier: &QuerierWrapper) -> StdResult<PairInfo> {
        let interface = self.interface();
        let pair_info = match interface {
            SwapInterface::Astroport {} => query_pair_info(querier, &self.contract_addr)?,
            SwapInterface::Helix { market_id } => {
                query_market_info(querier, &self.contract_addr, market_id)?
            }
            SwapInterface::Astrovault {
                pair_type: PairType::Stable {},
            }
            | SwapInterface::Astrovault {
                pair_type: PairType::Hybrid {},
            } => PairInfo {
                asset_infos: query_astrovault_pool_info(querier, &self.contract_addr)?.asset_infos,
            },
            SwapInterface::Astrovault {
                pair_type: PairType::Xyk {},
            } => PairInfo {
                asset_infos: query_astrovault_pair(querier, &self.contract_addr)?.asset_infos,
            },
            SwapInterface::OraiDexV2 {} => {
                query_orai_dex_v2_pair(querier, &self.contract_addr)?.info
            }
        };
        Ok(pair_info)
    }
}

impl SwapOperation {
    // Defaults to Astroport
    pub fn interface(&self) -> StdResult<SwapInterface> {
        if let Some(interface) = &self.interface {
            Ok(match interface {
                Interface::Binary(encoded) => from_json(encoded)?,
                Interface::Struct(operation) => operation.clone(),
            })
        } else {
            Ok(SwapInterface::Astroport {})
        }
    }

    pub fn pair_info(&self, querier: &QuerierWrapper) -> StdResult<PairInfo> {
        let interface = self.interface()?;
        let pair_info = match interface {
            SwapInterface::Astroport {} => query_pair_info(querier, &self.contract_addr)?,
            SwapInterface::Helix { market_id } => {
                query_market_info(querier, &self.contract_addr, market_id)?
            }
            SwapInterface::Astrovault {
                pair_type: PairType::Stable {},
            }
            | SwapInterface::Astrovault {
                pair_type: PairType::Hybrid {},
            } => PairInfo {
                asset_infos: query_astrovault_pool_info(querier, &self.contract_addr)?.asset_infos,
            },
            SwapInterface::Astrovault {
                pair_type: PairType::Xyk {},
            } => PairInfo {
                asset_infos: query_astrovault_pair(querier, &self.contract_addr)?.asset_infos,
            },
            SwapInterface::OraiDexV2 {} => {
                query_orai_dex_v2_pair(querier, &self.contract_addr)?.info
            }
        };
        Ok(pair_info)
    }
}

#[cw_serde]
pub enum ExecuteMsg {
    Receive(Cw20ReceiveMsg),
    ExecuteRoutes {
        offer_asset_info: AssetInfo,
        routes: Vec<RouteInfo>,
        minimum_receive: Uint128,
        to: Option<Addr>,
    },
    ExecuteSwapOp {
        operation: SwapOperation,
        amount: Option<Uint128>,
    },
    ExecuteRoutesV2 {
        routes: Vec<RouteInfoV2>,
        minimum_receive: Uint128,
        to: Option<Addr>,
    },
    ExecutePostSwap {
        offer_asset_info: AssetInfo,
        offer_amount: Uint128,
        return_asset_info: AssetInfo,
        to: Addr,
    },
    AssertMinimumReceive {
        receiver: Addr,
        asset_info: AssetInfo,
        prev_balance: Uint128,
        minimum_receive: Uint128,
    },
    UpdateConfig(InstantiateMsg),
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(QuerySimulationResult)]
    Simulation { routes: Vec<RouteInfoV2> },
}

#[cw_serde]
pub struct QuerySimulationResult {
    pub return_asset: Asset,
    pub fee_asset: Option<Asset>,
}

#[cw_serde]
pub enum PairExecuteMsg {
    Swap {
        offer_asset: Asset,
        belief_price: Option<Decimal>,
        max_spread: Option<Decimal>,
        to: Option<String>,
    },
}

#[cw_serde]
pub enum PairCw20HookMsg {
    Swap {
        belief_price: Option<Decimal>,
        max_spread: Option<Decimal>,
        to: Option<String>,
    },
}

#[cw_serde]
pub enum HelixExecuteMsg {
    Swap {
        market_id: String,
        minimum_receive: Option<Uint128>,
        to: Option<String>,
    },
}

#[cw_serde]
pub enum AstrovaultXykExecuteMsg {
    Swap { offer_asset: Asset },
}
#[cw_serde]
pub enum Cw20AstrovaultXykExecuteMsg {
    Swap {},
}

#[cw_serde]
pub enum AstrovaultStableExecuteMsg {
    Swap {
        swap_to_asset_index: u32,
        expected_return: Uint128,
    },
}

#[cw_serde]
pub enum AstrovaultHybridExecuteMsg {
    Swap {},
}
