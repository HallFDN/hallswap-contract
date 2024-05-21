use cosmwasm_std::{Addr, ContractInfoResponse, QuerierWrapper, StdResult, Uint128};
use models::asset::Asset;
use models::asset_info::AssetInfo;

use crate::msg::{
    QueryAstrovault, QueryAstrovaultHybrid, QueryAstrovaultHybridSimulationResponse, QueryAstrovaultResponse, QueryAstrovaultStable, QueryAstrovaultStableSimulationResponse, QueryHelix, QueryOraiDexV2, QueryOraiDexV2PairResponse
};

use super::msg::{
    Cw20BalanceResponse, Cw20QueryMsg, PairInfo, PairQueryMsg, PairSimulationResponse,
};

pub fn query_pair_info(querier: &QuerierWrapper, addr: &Addr) -> StdResult<PairInfo> {
    querier.query_wasm_smart(addr, &PairQueryMsg::Pair {})
}

pub fn query_astrovault_pool_info(
    querier: &QuerierWrapper,
    addr: &Addr,
) -> StdResult<QueryAstrovaultResponse> {
    querier.query_wasm_smart(addr, &QueryAstrovault::PoolInfo {})
}

pub fn query_astrovault_pair(
    querier: &QuerierWrapper,
    addr: &Addr,
) -> StdResult<QueryAstrovaultResponse> {
    querier.query_wasm_smart(addr, &QueryAstrovault::Pair {})
}

pub fn query_orai_dex_v2_pair(
    querier: &QuerierWrapper,
    addr: &Addr,
) -> StdResult<QueryOraiDexV2PairResponse> {
    querier.query_wasm_smart(addr, &QueryOraiDexV2::Pair {})
}

pub fn query_market_info(
    querier: &QuerierWrapper,
    addr: &Addr,
    market_id: String,
) -> StdResult<PairInfo> {
    querier.query_wasm_smart(addr, &QueryHelix::Market { market_id })
}

pub fn query_simulation(
    querier: &QuerierWrapper,
    addr: &Addr,
    offer_asset: Asset,
) -> StdResult<Uint128> {
    Ok(querier
        .query_wasm_smart(addr, &PairQueryMsg::Simulation { offer_asset })
        .map_or(Uint128::zero(), |res: PairSimulationResponse| {
            res.return_amount
        }))
}

pub fn query_helix_simulation(
    querier: &QuerierWrapper,
    addr: &Addr,
    offer_asset: Asset,
    market_id: String,
) -> StdResult<Uint128> {
    Ok(querier
        .query_wasm_smart(
            addr,
            &QueryHelix::Simulation {
                offer_asset,
                market_id,
            },
        )
        .map_or(Uint128::zero(), |res: PairSimulationResponse| {
            res.return_amount
        }))
}

pub fn query_astrovault_stable_simulation(
    querier: &QuerierWrapper,
    addr: &Addr,
    amount: Uint128,
    swap_from_asset_index: u32,
    swap_to_asset_index: u32,
) -> StdResult<Uint128> {
    Ok(querier
        .query_wasm_smart(
            addr,
            &QueryAstrovaultStable::SwapSimulation {
                amount,
                swap_from_asset_index,
                swap_to_asset_index,
            },
        )
        .map_or(
            Uint128::zero(),
            |res: QueryAstrovaultStableSimulationResponse| {
                let index = usize::try_from(swap_to_asset_index).unwrap();
                res.swap_to_assets_amount
                    .get(index)
                    .unwrap_or(&Uint128::zero())
                    .to_owned()
            },
        ))
}

pub fn query_astrovault_hybrid_simulation(
    querier: &QuerierWrapper,
    addr: &Addr,
    amount: Uint128,
    swap_from_asset_index: u32,
) -> StdResult<Uint128> {
    Ok(querier
        .query_wasm_smart(
            addr,
            &QueryAstrovaultHybrid::SwapSimulation {
                amount,
                swap_from_asset_index,
            },
        )
        .map_or(Uint128::zero(), |res: QueryAstrovaultHybridSimulationResponse| {
            res.to_amount_minus_fee
        }))
}

pub fn query_native_balance(
    querier: &QuerierWrapper,
    addr: &Addr,
    denom: &String,
) -> StdResult<Uint128> {
    Ok(querier
        .query_balance(addr, denom)
        .map_or(Uint128::zero(), |coin| coin.amount))
}

pub fn query_contract_info(
    querier: &QuerierWrapper,
    addr: &Addr,
) -> StdResult<ContractInfoResponse> {
    querier.query_wasm_contract_info(addr)
}

pub fn query_token_balance(
    querier: &QuerierWrapper,
    addr: &Addr,
    token: &String,
) -> StdResult<Uint128> {
    Ok(querier
        .query_wasm_smart(
            token,
            &Cw20QueryMsg::Balance {
                address: addr.into(),
            },
        )
        .map_or(Uint128::zero(), |res: Cw20BalanceResponse| res.balance))
}

pub fn query_balance(
    querier: &QuerierWrapper,
    addr: &Addr,
    asset_info: &AssetInfo,
) -> StdResult<Uint128> {
    match &asset_info {
        AssetInfo::NativeToken { denom } => query_native_balance(querier, addr, denom),
        AssetInfo::Token { contract_addr } => {
            query_token_balance(querier, addr, &contract_addr.to_string())
        }
    }
}
