use cosmwasm_std::Order;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::errors::ContractError;

pub type ContractResult<T> = core::result::Result<T, ContractError>;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum OrderBy {
    Asc,
    Desc,
}

impl Into<Order> for OrderBy {
    fn into(self) -> Order {
        if self == OrderBy::Asc {
            Order::Ascending
        } else {
            Order::Descending
        }
    }
}
