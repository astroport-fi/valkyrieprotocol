use std::cmp::max;

use cosmwasm_std::{Addr, attr, DepsMut, Env, MessageInfo, Response, StdError, Uint128, Uint64};

use valkyrie::common::ContractResult;
use valkyrie::cw20::create_send_msg_response;
use valkyrie::errors::ContractError;
use valkyrie::governance::execute_msgs::StakingConfigInitMsg;

use crate::common::states::{ContractConfig, is_admin, load_contract_available_balance};
use crate::staking::states::StakingConfig;

use super::states::{StakerState, StakingState};

pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: StakingConfigInitMsg,
) -> ContractResult<Response> {
    // Execute
    StakingConfig {
        withdraw_delay: msg.withdraw_delay.u64(),
    }.save(deps.storage)?;

    StakingState {
        total_share: Uint128::zero(),
    }.save(deps.storage)?;

    // Response
    Ok(Response::default())
}

pub fn update_config(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    withdraw_delay: Option<Uint64>,
) -> ContractResult<Response> {
    // Validate
    if !is_admin(deps.storage, env, &info.sender) {
        return Err(ContractError::Unauthorized {});
    }

    // Execute
    let mut config = StakingConfig::load(deps.storage)?;

    if let Some(withdraw_delay) = withdraw_delay {
        config.withdraw_delay = withdraw_delay.u64();
    }

    config.save(deps.storage)?;

    // Response
    Ok(Response::default())
}

pub fn stake_voting_token(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    sender: Addr,
    amount: Uint128,
) -> ContractResult<Response> {
    // Validate
    if amount.is_zero() {
        return Err(ContractError::Std(StdError::generic_err("Insufficient funds sent")));
    }

    // Execute
    let mut staking_state = StakingState::load(deps.storage)?;
    let mut staker_state = StakerState::load_safe(deps.storage, &sender)?;

    let contract_available_balance = load_contract_available_balance(deps.as_ref())?
        .checked_sub(amount)?;

    let share = if contract_available_balance.is_zero() || staking_state.total_share.is_zero() {
        amount
    } else {
        amount.multiply_ratio(staking_state.total_share, contract_available_balance)
    };

    staking_state.total_share += share;
    staker_state.save(deps.storage)?;

    staker_state.share += share;
    staker_state.save(deps.storage)?;

    // Response
    Ok(
        Response {
            submessages: vec![],
            messages: vec![],
            attributes: vec![
                attr("action", "staking"),
                attr("sender", sender.as_str()),
                attr("share", share.to_string()),
                attr("amount", amount.to_string()),
            ],
            data: None,
        }
    )
}

// Withdraw amount if not staked. By default all funds will be withdrawn.
pub fn unstake_voting_token(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    amount: Option<Uint128>,
) -> ContractResult<Response> {
    let staker_state = StakerState::may_load(deps.storage, &info.sender)?;

    if staker_state.is_none() {
        return Err(ContractError::Std(StdError::generic_err("Nothing staked")));
    }

    let mut staker_state = staker_state.unwrap();
    let mut staking_state = StakingState::load(deps.storage)?;

    staker_state.clean_votes(deps.storage);

    let contract_available_balance = load_contract_available_balance(deps.as_ref())?;
    let total_share = staking_state.total_share;
    let locked_balance = staker_state.get_locked_balance();
    let locked_share = locked_balance.multiply_ratio(
        total_share,
        contract_available_balance,
    );
    let user_share = staker_state.share;
    let withdraw_share = amount.map(|v| {
        max(
            v.multiply_ratio(total_share, contract_available_balance),
            Uint128::new(1u128),
        )
    }).unwrap_or_else(|| user_share.checked_sub(locked_share).unwrap());
    let withdraw_amount = amount.unwrap_or_else(|| {
        withdraw_share.multiply_ratio(contract_available_balance, total_share)
    });

    if locked_share + withdraw_share > user_share {
        return Err(ContractError::Std(StdError::generic_err(
            "User is trying to withdraw too many tokens.",
        )));
    }

    staker_state.share = user_share.checked_sub(withdraw_share)?;
    staker_state.withdraw_unstaked_amounts.push((env.block.height, withdraw_amount));
    staker_state.save(deps.storage)?;

    staking_state.total_share = total_share.checked_sub(withdraw_share)?;
    staking_state.save(deps.storage)?;

    // Response
    Ok(Response::default())
}

pub fn withdraw_voting_token(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> ContractResult<Response> {
    // Execute
    let mut staker_state = StakerState::load(deps.storage, &info.sender)?;

    let withdraw_amount = staker_state.withdraw_unstaked(deps.storage, env.block.height)
        .iter()
        .map(|(_, amount)| amount)
        .sum();

    staker_state.save(deps.storage)?;

    // Response
    let contract_config = ContractConfig::load(deps.storage)?;
    Ok(
        create_send_msg_response(
            &contract_config.token_contract,
            &info.sender,
            withdraw_amount,
            "claim_pending_withdrawal",
        )
    )
}