use cosmwasm_std::{Timestamp, Uint128, Uint64};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::campaign::enumerations::{Denom, Referrer};
use crate::common::OrderBy;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    CampaignInfo {},
    DistributionConfig {},
    CampaignState {},
    ShareUrl {
        address: String,
    },
    GetAddressFromReferrer {
        referrer: Referrer,
    },
    Participation {
        address: String,
    },
    Participations {
        start_after: Option<String>,
        limit: Option<u32>,
        order_by: Option<OrderBy>,
    },
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, JsonSchema)]
pub struct CampaignInfoResponse {
    pub title: String,
    pub description: String,
    pub url: String,
    pub parameter_key: String,
    pub creator: String,
    pub created_at: Timestamp,
    pub created_block: Uint64,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, JsonSchema)]
pub struct DistributionConfigResponse {
    pub denom: Denom,
    pub amounts: Vec<Uint128>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, JsonSchema)]
pub struct CampaignStateResponse {
    pub participation_count: Uint64,
    pub cumulative_distribution_amount: Uint128,
    pub locked_balance: Uint128,
    pub balance: Uint128,
    pub is_active: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, JsonSchema)]
pub struct ShareUrlResponse {
    pub address: String,
    pub compressed: String,
    pub url: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, JsonSchema)]
pub struct GetAddressFromReferrerResponse {
    pub address: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, JsonSchema)]
pub struct ParticipationResponse {
    pub actor_address: String,
    pub referrer_address: Option<String>,
    pub rewards: Vec<(Denom, Uint128)>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, JsonSchema)]
pub struct ParticipationsResponse {
    pub participations: Vec<ParticipationResponse>,
}