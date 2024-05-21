use cosmwasm_schema::cw_serde;
use cosmwasm_std::{CosmosMsg, Uint128};

use super::asset_info::AssetInfo;

#[cw_serde]
pub struct Asset {
    pub info: AssetInfo,
    pub amount: Uint128,
}

impl Asset {
    /// Returns the address or denom of the asset.
    pub fn id(&self) -> String {
        self.info.id()
    }

    /// Returns a CW20 transfer or bank send message.
    pub fn to_send_msg(&self, recipient: String) -> CosmosMsg {
        self.info.to_send_msg(recipient, self.amount)
    }
}
