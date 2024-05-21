use cosmwasm_schema::cw_serde;
use cosmwasm_std::{to_json_binary, Addr, BankMsg, Coin, CosmosMsg, Uint128, WasmMsg};
use cw20::Cw20ExecuteMsg;

#[cw_serde]
#[derive(Hash, Eq)]
pub enum AssetInfo {
    Token { contract_addr: Addr },
    NativeToken { denom: String },
}

impl AssetInfo {
    /// Returns the address or denom of the asset.
    pub fn id(&self) -> String {
        match self {
            AssetInfo::Token { contract_addr } => contract_addr.to_string(),
            AssetInfo::NativeToken { denom } => denom.to_string(),
        }
    }

    /// Returns a CW20 transfer or bank send message.
    pub fn to_send_msg(&self, recipient: String, amount: Uint128) -> CosmosMsg {
        match self {
            AssetInfo::Token { contract_addr } => CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: contract_addr.to_string(),
                msg: to_json_binary(&Cw20ExecuteMsg::Transfer { recipient, amount }).unwrap(),
                funds: vec![],
            }),
            AssetInfo::NativeToken { denom } => CosmosMsg::Bank(BankMsg::Send {
                to_address: recipient,
                amount: vec![Coin {
                    denom: denom.clone(),
                    amount,
                }],
            }),
        }
    }
}
