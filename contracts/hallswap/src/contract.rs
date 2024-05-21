#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    from_json, to_json_binary, Addr, Binary, Coin, CosmosMsg, Decimal, Deps, DepsMut, Env,
    MessageInfo, Response, StdResult, Uint128, WasmMsg,
};
use cw2::set_contract_version;
use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg};
use models::asset::Asset;
use models::asset_info::AssetInfo;
use querier::querier::{
    query_astrovault_hybrid_simulation, query_astrovault_pool_info,
    query_astrovault_stable_simulation, query_balance, query_contract_info, query_helix_simulation,
    query_simulation,
};

use crate::error::ContractError;
use crate::msg::{
    AstrovaultHybridExecuteMsg, AstrovaultStableExecuteMsg, AstrovaultXykExecuteMsg,
    Cw20AstrovaultXykExecuteMsg, ExecuteMsg, HelixExecuteMsg, InstantiateMsg, Interface,
    PairCw20HookMsg, PairExecuteMsg, PairType, QueryMsg, QuerySimulationResult, RouteInfo,
    RouteInfoV2, SwapInterface, SwapOperation,
};
use crate::state::{Config, CONFIG, FEES_COLLECTED};

const CONTRACT_NAME: &str = "crates.io:hallswap";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

// Use max allowed values to bypass all slippage calculations on the pool contract
const BELIEF_PRICE: Decimal = Decimal::MAX;
const MAX_SLIPPAGE: Decimal = Decimal::raw(500_000_000_000_000_000u128); // 0.5 = 50%

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    let config = Config {
        owner: match msg.owner {
            Some(owner) => owner,
            None => info.sender.clone(),
        },
        fee_address: match msg.fee_address {
            Some(fee_address) => fee_address,
            None => info.sender.clone(),
        },
        fee_bps: msg.fee_bps.unwrap_or(0),
        fee_assets: match msg.fee_assets {
            Some(fee_assets) => fee_assets,
            None => vec![],
        },
    };

    CONFIG.save(deps.storage, &config)?;
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Receive(cw20_msg) => receive_cw20(deps, env, cw20_msg),
        ExecuteMsg::ExecuteRoutes {
            offer_asset_info,
            routes,
            minimum_receive,
            to,
        } => swap_deprec(
            deps,
            env,
            info.sender,
            offer_asset_info,
            routes,
            minimum_receive,
            to,
        ),
        ExecuteMsg::ExecuteRoutesV2 {
            routes,
            minimum_receive,
            to,
        } => swap(deps, env, info.sender, routes, minimum_receive, to),
        ExecuteMsg::ExecuteSwapOp { operation, amount } => {
            swap_pool(deps, env, info.sender, operation, amount)
        }
        ExecuteMsg::ExecutePostSwap {
            offer_asset_info,
            offer_amount,
            return_asset_info,
            to,
        } => post_swap(
            deps,
            env,
            info.sender,
            offer_asset_info,
            offer_amount,
            return_asset_info,
            to,
        ),
        ExecuteMsg::AssertMinimumReceive {
            receiver,
            asset_info,
            prev_balance,
            minimum_receive,
        } => assert_minimum_receive(
            deps,
            env,
            info.sender,
            receiver,
            asset_info,
            prev_balance,
            minimum_receive,
        ),
        ExecuteMsg::UpdateConfig(config) => update_config(deps, info, config),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> Result<Binary, ContractError> {
    match msg {
        QueryMsg::Simulation { routes } => Ok(to_json_binary(&simulation(deps, routes)?)?),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, env: Env, msg: InstantiateMsg) -> Result<Response, ContractError> {
    let contract_info = query_contract_info(&deps.querier, &env.contract.address)?;
    let fallback_owner = deps
        .api
        .addr_validate(&contract_info.admin.unwrap_or(contract_info.creator))?;
    let config = Config {
        owner: match msg.owner {
            Some(owner) => owner,
            None => fallback_owner.clone(),
        },
        fee_address: match msg.fee_address {
            Some(fee_address) => fee_address,
            None => fallback_owner.clone(),
        },
        fee_bps: msg.fee_bps.unwrap_or(0),
        fee_assets: match msg.fee_assets {
            Some(fee_assets) => fee_assets,
            None => vec![],
        },
    };

    CONFIG.save(deps.storage, &config)?;
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    Ok(Response::default())
}

/// See `swap` function for where fees are charged
fn simulation(
    deps: Deps,
    routes: Vec<RouteInfoV2>,
) -> Result<QuerySimulationResult, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    let (offer_asset_info, return_asset_info) = get_offer_return_asset(&routes)?;
    let mut fee_asset_info = offer_asset_info.clone();
    let mut fee_asset_amount = Uint128::zero();
    let mut return_asset_amount = Uint128::zero();

    // Execute every route
    for route_info in routes {
        let (route, mut offer_amount) = (route_info.route, route_info.offer_amount);

        // Case 1: Charge starting offer asset
        if config.fee_bps > 0 && config.fee_assets.contains(&offer_asset_info.id()) {
            let fee_amount = calc_fee(offer_amount, config.fee_bps)?;
            fee_asset_amount = fee_asset_amount.checked_add(fee_amount)?;
            offer_amount = offer_amount.checked_sub(fee_amount)?;
        }

        // Execute the swap, sending all return asset back to this contract
        let mut mut_offer_asset = Asset {
            info: offer_asset_info.clone(),
            amount: offer_amount,
        };
        for swap_operation in &route {
            let pair_info = swap_operation.pair_info(&deps.querier)?;
            let offer_asset_index = u32::try_from(
                pair_info
                    .asset_infos
                    .iter()
                    .position(|info| info.id() == swap_operation.offer_asset.id())
                    .ok_or(ContractError::InvalidRoute {})?,
            )
            .unwrap();
            let return_asset_index = u32::try_from(
                pair_info
                    .asset_infos
                    .iter()
                    .position(|info| info.id() == swap_operation.return_asset.id())
                    .ok_or(ContractError::InvalidRoute {})?,
            )
            .unwrap();

            let return_amount = match swap_operation.interface()? {
                SwapInterface::Astroport {} | SwapInterface::OraiDexV2 {} => query_simulation(
                    &deps.querier,
                    &swap_operation.contract_addr,
                    mut_offer_asset.clone(),
                )?,
                SwapInterface::Helix { market_id } => query_helix_simulation(
                    &deps.querier,
                    &swap_operation.contract_addr,
                    mut_offer_asset.clone(),
                    market_id,
                )?,
                SwapInterface::Astrovault {
                    pair_type: PairType::Xyk {},
                } => query_simulation(
                    &deps.querier,
                    &swap_operation.contract_addr,
                    mut_offer_asset.clone(),
                )?,
                SwapInterface::Astrovault {
                    pair_type: PairType::Stable {},
                } => query_astrovault_stable_simulation(
                    &deps.querier,
                    &swap_operation.contract_addr,
                    mut_offer_asset.amount,
                    offer_asset_index,
                    return_asset_index,
                )?,
                SwapInterface::Astrovault {
                    pair_type: PairType::Hybrid {},
                } => query_astrovault_hybrid_simulation(
                    &deps.querier,
                    &swap_operation.contract_addr,
                    mut_offer_asset.amount,
                    offer_asset_index,
                )?,
            };
            if Uint128::is_zero(&return_amount) {
                return Err(ContractError::InvalidRoute {});
            }

            let return_asset_info = pair_info
                .asset_infos
                .iter()
                .find(|info| info.id() == swap_operation.return_asset.id())
                .ok_or(ContractError::InvalidRoute {})?;

            mut_offer_asset = Asset {
                info: return_asset_info.clone(),
                amount: return_amount,
            };
        }
        return_asset_amount = return_asset_amount.checked_add(mut_offer_asset.amount)?;
    }

    if config.fee_bps > 0 && fee_asset_amount.is_zero() {
        let fee_amount = calc_fee(return_asset_amount, config.fee_bps)?;
        fee_asset_amount = fee_amount;
        fee_asset_info = return_asset_info.clone();
        return_asset_amount = return_asset_amount.checked_sub(fee_amount)?;
    }

    Ok(QuerySimulationResult {
        return_asset: Asset {
            info: return_asset_info,
            amount: return_asset_amount,
        },
        fee_asset: if fee_asset_amount.is_zero() {
            None
        } else {
            Some(Asset {
                info: fee_asset_info,
                amount: fee_asset_amount,
            })
        },
    })
}

fn receive_cw20(
    deps: DepsMut,
    env: Env,
    cw20_msg: Cw20ReceiveMsg,
) -> Result<Response, ContractError> {
    match from_json(&cw20_msg.msg)? {
        ExecuteMsg::ExecuteRoutes {
            offer_asset_info,
            routes,
            minimum_receive,
            to,
        } => swap_deprec(
            deps,
            env,
            Addr::unchecked(cw20_msg.sender),
            offer_asset_info,
            routes,
            minimum_receive,
            to,
        ),
        ExecuteMsg::ExecuteRoutesV2 {
            routes,
            minimum_receive,
            to,
        } => swap(
            deps,
            env,
            Addr::unchecked(cw20_msg.sender),
            routes,
            minimum_receive,
            to,
        ),
        _ => Err(ContractError::InvalidCw20HookMessage {}),
    }
}

fn swap_deprec(
    deps: DepsMut,
    env: Env,
    sender: Addr,
    offer_asset_info: AssetInfo,
    routes: Vec<RouteInfo>,
    minimum_receive: Uint128,
    to: Option<Addr>,
) -> Result<Response, ContractError> {
    let mut routes_v2: Vec<RouteInfoV2> = vec![];
    for route_info in routes {
        let mut mut_offer_asset_info = offer_asset_info.clone();

        let mut new_route: Vec<SwapOperation> = vec![];
        for contract_info in route_info.route {
            let pair_info = contract_info.pair_info(&deps.querier)?;
            let return_asset_info = pair_info
                .asset_infos
                .iter()
                .find(|info| info.id() != mut_offer_asset_info.id())
                .ok_or(ContractError::InvalidRoute {})?;
            new_route.push(SwapOperation {
                contract_addr: contract_info.contract_addr,
                offer_asset: mut_offer_asset_info.clone(),
                return_asset: return_asset_info.clone(),
                interface: contract_info.interface.map(Interface::Struct),
            });
            mut_offer_asset_info = return_asset_info.clone();
        }

        routes_v2.push(RouteInfoV2 {
            route: new_route,
            offer_amount: route_info.offer_amount,
        });
    }
    swap(deps, env, sender, routes_v2, minimum_receive, to)
}

/// Two cases where fees are charged depending on where and if we find a valid fee asset
/// Case 1: valid fee offer asset -> charge at `swap`
/// Case 2: return asset -> charge at `post_swap`
fn swap(
    deps: DepsMut,
    env: Env,
    sender: Addr,
    routes: Vec<RouteInfoV2>,
    minimum_receive: Uint128,
    to: Option<Addr>,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    let mut msgs: Vec<CosmosMsg> = vec![];
    let mut total_offer_amount = Uint128::zero();
    let mut total_fee_amount = Uint128::zero();

    let (offer_asset_info, return_asset_info) = get_offer_return_asset(&routes)?;

    // Execute every route
    for route_info in routes {
        let (route, mut offer_amount) = (route_info.route, route_info.offer_amount);
        total_offer_amount = total_offer_amount.checked_add(offer_amount)?;

        // Case 1: Charge starting offer asset
        if config.fee_bps > 0 && config.fee_assets.contains(&offer_asset_info.id()) {
            let fee_amount = calc_fee(offer_amount, config.fee_bps)?;
            total_fee_amount = total_fee_amount.checked_add(fee_amount)?;
            offer_amount = offer_amount.checked_sub(fee_amount)?;
        }

        // Execute the swap, sending all return asset back to this contract
        for (idx, swap_operation) in route.iter().enumerate() {
            msgs.push(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: env.contract.address.to_string(),
                funds: vec![],
                msg: to_json_binary(&ExecuteMsg::ExecuteSwapOp {
                    operation: swap_operation.clone(),
                    amount: if idx == 0 { Some(offer_amount) } else { None },
                })?,
            }));
        }
    }

    // Send to fee collector for Case 1
    if !total_fee_amount.is_zero() {
        msgs.push(offer_asset_info.to_send_msg(config.fee_address.to_string(), total_fee_amount));
        FEES_COLLECTED.save(
            deps.storage,
            &Asset {
                info: offer_asset_info.clone(),
                amount: total_fee_amount,
            },
        )?;
    } else {
        FEES_COLLECTED.remove(deps.storage);
    }

    // Send the return asset back to the user/to and emit all event logs
    let receiver = to.unwrap_or(sender.clone());
    msgs.push(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: env.contract.address.to_string(),
        funds: vec![],
        msg: to_json_binary(&ExecuteMsg::ExecutePostSwap {
            offer_asset_info,
            offer_amount: total_offer_amount,
            return_asset_info: return_asset_info.clone(),
            to: receiver.clone(),
        })?,
    }));

    // Assert minimum received by the user
    let receiver_balance = query_balance(&deps.querier, &receiver, &return_asset_info)?;
    msgs.push(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: env.contract.address.to_string(),
        funds: vec![],
        msg: to_json_binary(&ExecuteMsg::AssertMinimumReceive {
            receiver,
            asset_info: return_asset_info,
            prev_balance: receiver_balance,
            minimum_receive,
        })?,
    }));

    Ok(Response::new().add_messages(msgs))
}

fn swap_pool(
    deps: DepsMut,
    env: Env,
    sender: Addr,
    operation: SwapOperation,
    amount: Option<Uint128>,
) -> Result<Response, ContractError> {
    // This is an internal function that's not meant to be executed by users
    if env.contract.address != sender {
        return Err(ContractError::Unauthorized {});
    }

    let (offer_asset_info, return_asset_info, swap_addr) = (
        operation.clone().offer_asset,
        operation.clone().return_asset,
        operation.clone().contract_addr,
    );

    let mut msgs: Vec<CosmosMsg> = vec![];

    let offer_amount = amount.unwrap_or(query_balance(
        &deps.querier,
        &env.contract.address,
        &offer_asset_info,
    )?);
    let offer_asset = Asset {
        info: offer_asset_info.clone(),
        amount: offer_amount,
    };

    msgs.push(match operation.interface()? {
        SwapInterface::Astroport {} | SwapInterface::OraiDexV2 {} => match &offer_asset.info {
            AssetInfo::NativeToken { denom } => CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: swap_addr.to_string(),
                funds: vec![Coin {
                    denom: denom.to_string(),
                    amount: offer_asset.amount,
                }],
                msg: to_json_binary(&PairExecuteMsg::Swap {
                    offer_asset,
                    belief_price: Some(BELIEF_PRICE),
                    max_spread: Some(MAX_SLIPPAGE),
                    to: None,
                })?,
            }),
            AssetInfo::Token { contract_addr } => CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: contract_addr.to_string(),
                funds: vec![],
                msg: to_json_binary(&Cw20ExecuteMsg::Send {
                    contract: swap_addr.to_string(),
                    amount: offer_asset.amount,
                    msg: to_json_binary(&PairCw20HookMsg::Swap {
                        belief_price: Some(BELIEF_PRICE),
                        max_spread: Some(MAX_SLIPPAGE),
                        to: None,
                    })?,
                })?,
            }),
        },
        SwapInterface::Helix { market_id } => CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: swap_addr.to_string(),
            funds: vec![Coin {
                denom: offer_asset_info.id(), // assume to be definitely a denom as Helix only supports native assets
                amount: offer_asset.amount,
            }],
            msg: to_json_binary(&HelixExecuteMsg::Swap {
                market_id,
                minimum_receive: None,
                to: None,
            })?,
        }),
        SwapInterface::Astrovault {
            pair_type: PairType::Xyk {},
        } => match &offer_asset.info {
            AssetInfo::NativeToken { denom } => CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: swap_addr.to_string(),
                funds: vec![Coin {
                    denom: denom.to_string(),
                    amount: offer_asset.amount,
                }],
                msg: to_json_binary(&AstrovaultXykExecuteMsg::Swap { offer_asset })?,
            }),
            AssetInfo::Token { contract_addr } => CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: contract_addr.to_string(),
                funds: vec![],
                msg: to_json_binary(&Cw20ExecuteMsg::Send {
                    contract: swap_addr.to_string(),
                    amount: offer_asset.amount,
                    msg: to_json_binary(&Cw20AstrovaultXykExecuteMsg::Swap {})?,
                })?,
            }),
        },
        SwapInterface::Astrovault {
            pair_type: PairType::Hybrid {},
        } => match &offer_asset.info {
            AssetInfo::NativeToken { denom } => CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: swap_addr.to_string(),
                funds: vec![Coin {
                    denom: denom.to_string(),
                    amount: offer_asset.amount,
                }],
                msg: to_json_binary(&AstrovaultHybridExecuteMsg::Swap {})?,
            }),
            AssetInfo::Token { contract_addr } => CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: contract_addr.to_string(),
                funds: vec![],
                msg: to_json_binary(&Cw20ExecuteMsg::Send {
                    contract: swap_addr.to_string(),
                    amount: offer_asset.amount,
                    msg: to_json_binary(&AstrovaultHybridExecuteMsg::Swap {})?,
                })?,
            }),
        },
        SwapInterface::Astrovault {
            pair_type: PairType::Stable {},
        } => {
            let pool_info = query_astrovault_pool_info(&deps.querier, &swap_addr)?;
            let swap_to_asset_index = pool_info
                .asset_infos
                .iter()
                .position(|info| info.id() == return_asset_info.id())
                .ok_or(ContractError::InvalidRoute {})?
                as u32;
            let expected_return = Uint128::zero();
            match &offer_asset.info {
                AssetInfo::NativeToken { denom } => CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: swap_addr.to_string(),
                    funds: vec![Coin {
                        denom: denom.to_string(),
                        amount: offer_asset.amount,
                    }],
                    msg: to_json_binary(&AstrovaultStableExecuteMsg::Swap {
                        swap_to_asset_index,
                        expected_return,
                    })?,
                }),
                AssetInfo::Token { contract_addr } => CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: contract_addr.to_string(),
                    funds: vec![],
                    msg: to_json_binary(&Cw20ExecuteMsg::Send {
                        contract: swap_addr.to_string(),
                        amount: offer_asset.amount,
                        msg: to_json_binary(&AstrovaultStableExecuteMsg::Swap {
                            swap_to_asset_index,
                            expected_return,
                        })?,
                    })?,
                }),
            }
        }
    });

    Ok(Response::new().add_messages(msgs))
}

/// Sends the correct return amount back to the user/to and emits all event logs.
fn post_swap(
    deps: DepsMut,
    env: Env,
    sender: Addr,
    offer_asset_info: AssetInfo,
    offer_amount: Uint128,
    return_asset_info: AssetInfo,
    to: Addr,
) -> Result<Response, ContractError> {
    // This is an internal function that's not meant to be executed by users
    if env.contract.address != sender {
        return Err(ContractError::Unauthorized {});
    }

    let config = CONFIG.load(deps.storage)?;

    let offer_asset_id = offer_asset_info.id();
    let return_asset_id = return_asset_info.id();
    let mut return_amount =
        query_balance(&deps.querier, &env.contract.address, &return_asset_info)?;

    let mut fee: Vec<(String, String)> = vec![];
    let mut msgs: Vec<CosmosMsg> = vec![];

    let fees_collected = FEES_COLLECTED.may_load(deps.storage)?;
    if let Some(fees_collected) = fees_collected {
        // Case 1 which we charged in `swap` function
        fee.push(("fee_asset".to_owned(), fees_collected.info.id()));
        fee.push(("fee_amount".to_owned(), fees_collected.amount.to_string()));
    } else if config.fee_bps > 0 {
        // Case 2 which we charged at the end, `post_swap` function
        let fee_amount = calc_fee(return_amount, config.fee_bps)?;
        return_amount = return_amount.checked_sub(fee_amount)?;
        if !fee_amount.is_zero() {
            msgs.push(return_asset_info.to_send_msg(config.fee_address.to_string(), fee_amount));
            fee.push(("fee_asset".to_owned(), return_asset_id.clone()));
            fee.push(("fee_amount".to_owned(), fee_amount.to_string()));
        }
    }

    msgs.push(return_asset_info.to_send_msg(to.to_string(), return_amount));
    FEES_COLLECTED.remove(deps.storage);
    Ok(Response::new()
        .add_messages(msgs)
        .add_attribute("hallswap", "v1")
        .add_attribute("offer_asset", offer_asset_id)
        .add_attribute("offer_amount", offer_amount)
        .add_attribute("return_asset", return_asset_id)
        .add_attribute("return_amount", return_amount)
        .add_attribute("receiver", to)
        .add_attributes(fee))
}

/// Asserts that `receiver` will receive at least `min_output` of `asset_info`.
fn assert_minimum_receive(
    deps: DepsMut,
    env: Env,
    sender: Addr,
    receiver: Addr,
    asset_info: AssetInfo,
    prev_balance: Uint128,
    minimum_receive: Uint128,
) -> Result<Response, ContractError> {
    // This is an internal function that's not meant to be executed by users
    if env.contract.address != sender {
        return Err(ContractError::Unauthorized {});
    }

    let current_balance = query_balance(&deps.querier, &receiver, &asset_info)?;
    let swap_amount = current_balance.checked_sub(prev_balance)?;
    if swap_amount < minimum_receive {
        Err(ContractError::AssertionMinimumReceive {
            receive: minimum_receive,
            amount: swap_amount,
        })
    } else {
        Ok(Response::default())
    }
}

fn update_config(
    deps: DepsMut,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;

    if info.sender != config.owner {
        return Err(ContractError::Unauthorized {});
    }

    if let Some(owner) = msg.owner {
        config.owner = owner;
    }
    if let Some(fee_address) = msg.fee_address {
        config.fee_address = fee_address;
    }
    if let Some(fee_bps) = msg.fee_bps {
        config.fee_bps = fee_bps;
    }
    if let Some(fee_assets) = msg.fee_assets {
        config.fee_assets = fee_assets;
    }

    CONFIG.save(deps.storage, &config)?;
    Ok(Response::new().add_attribute("action", "update_config"))
}

/// Returns the basis points value of `amount`.
fn calc_fee(amount: Uint128, fee_bps: u16) -> StdResult<Uint128> {
    Ok(amount
        .checked_mul(Uint128::from(fee_bps))?
        .checked_div(Uint128::from(10000u16))?)
}

fn get_offer_return_asset(
    routes: &[RouteInfoV2],
) -> Result<(AssetInfo, AssetInfo), ContractError> {
    let offer_asset_info = if let Some(route_info) = routes.first() {
        if let Some(swap_operation) = route_info.route.first() {
            Ok(swap_operation.offer_asset.clone())
        } else {
            Err(ContractError::InvalidRoute {})
        }
    } else {
        Err(ContractError::InvalidRoute {})
    }?;
    let return_asset_info = if let Some(route_info) = routes.first() {
        if let Some(swap_operation) = route_info.route.last() {
            Ok(swap_operation.return_asset.clone())
        } else {
            Err(ContractError::InvalidRoute {})
        }
    } else {
        Err(ContractError::InvalidRoute {})
    }?;
    Ok((offer_asset_info, return_asset_info))
}
