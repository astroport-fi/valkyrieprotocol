use cosmwasm_std::{Addr, attr, CosmosMsg, Decimal, DepsMut, Env, MessageInfo, Reply, ReplyOn, Response, StdError, StdResult, SubMsg, to_binary, Uint128, WasmMsg};

use valkyrie::common::{ContractResult, Execution, ExecutionMsg};
use valkyrie::errors::ContractError;
use valkyrie::governance::enumerations::{PollStatus, VoteOption};
use valkyrie::governance::execute_msgs::{ExecuteMsg, PollConfigInitMsg};
use valkyrie::message_factories;

use crate::common::states::{ContractConfig, load_available_balance};
use crate::poll::states::{PollExecutionContext, PollResult};
use crate::staking::states::StakerState;

use super::states::{get_poll_id, Poll, PollConfig, PollState};

const MIN_TITLE_LENGTH: usize = 4;
const MAX_TITLE_LENGTH: usize = 64;
const MIN_DESC_LENGTH: usize = 4;
const MAX_DESC_LENGTH: usize = 1024;
const MIN_LINK_LENGTH: usize = 12;
const MAX_LINK_LENGTH: usize = 128;

pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: PollConfigInitMsg,
) -> ContractResult<Response> {
    // Validate
    validate_quorum(msg.quorum)?;
    validate_threshold(msg.threshold)?;

    // Execute
    let poll_config = PollConfig {
        quorum: msg.quorum,
        threshold: msg.threshold,
        voting_period: msg.voting_period,
        execution_delay_period: msg.execution_delay_period,
        proposal_deposit: msg.proposal_deposit,
        snapshot_period: msg.snapshot_period,
    };

    let poll_state = PollState {
        poll_count: 0,
        total_deposit: Uint128::zero(),
    };

    poll_config.save(deps.storage)?;
    poll_state.save(deps.storage)?;

    // Response
    Ok(Response {
        submessages: vec![],
        messages: vec![],
        attributes: vec![
            attr("action", "instantiate"),
        ],
        data: None,
    })
}

#[allow(clippy::too_many_arguments)]
pub fn update_poll_config(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    quorum: Option<Decimal>,
    threshold: Option<Decimal>,
    voting_period: Option<u64>,
    execution_delay_period: Option<u64>,
    proposal_deposit: Option<Uint128>,
    snapshot_period: Option<u64>,
) -> ContractResult<Response> {
    // Validate
    if env.contract.address != info.sender {
        return Err(ContractError::Unauthorized {});
    }

    // Execute
    let mut poll_config = PollConfig::load(deps.storage)?;

    if let Some(quorum) = quorum {
        validate_quorum(quorum)?;
        poll_config.quorum = quorum;
    }

    if let Some(threshold) = threshold {
        validate_threshold(threshold)?;
        poll_config.threshold = threshold;
    }

    if let Some(voting_period) = voting_period {
        poll_config.voting_period = voting_period;
    }

    if let Some(execution_delay_period) = execution_delay_period {
        poll_config.execution_delay_period = execution_delay_period;
    }

    if let Some(proposal_deposit) = proposal_deposit {
        poll_config.proposal_deposit = proposal_deposit;
    }

    if let Some(period) = snapshot_period {
        poll_config.snapshot_period = period;
    }

    poll_config.save(deps.storage)?;

    // Response
    Ok(Response {
        submessages: vec![],
        messages: vec![],
        attributes: vec![
            attr("action", "update_poll_config"),
            attr("is_updated_quorum", quorum.is_some().to_string()),
            attr("is_updated_threshold", threshold.is_some().to_string()),
            attr("is_updated_voting_period", voting_period.is_some().to_string()),
            attr("is_updated_execution_delay_period", execution_delay_period.is_some().to_string()),
            attr("is_updated_proposal_deposit", proposal_deposit.is_some().to_string()),
            attr("is_updated_snapshot_period", snapshot_period.is_some().to_string()),
        ],
        data: None,
    })
}

#[allow(clippy::too_many_arguments)]
pub fn create_poll(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    proposer: Addr,
    deposit_amount: Uint128,
    title: String,
    description: String,
    link: Option<String>,
    executions: Vec<ExecutionMsg>,
) -> ContractResult<Response> {
    // Validate
    validate_title(&title)?;
    validate_description(&description)?;
    validate_link(&link)?;

    let config = ContractConfig::load(deps.storage)?;
    if !config.is_governance_token(&info.sender) {
        return Err(ContractError::Unauthorized {});
    }

    let poll_config = PollConfig::load(deps.storage)?;
    if deposit_amount < poll_config.proposal_deposit {
        return Err(ContractError::Std(StdError::generic_err(
            format!("Must deposit more than {} token", poll_config.proposal_deposit)
        )));
    }

    // Execute
    let executions = executions.iter()
        .map(|execution| Execution::from(deps.api, execution))
        .collect();

    let mut poll = Poll {
        id: get_poll_id(deps.storage, &deposit_amount)?,
        creator: proposer.clone(),
        status: PollStatus::InProgress,
        yes_votes: Uint128::zero(),
        no_votes: Uint128::zero(),
        abstain_votes: Uint128::zero(),
        end_height: env.block.height + poll_config.voting_period,
        title,
        description,
        link,
        executions,
        deposit_amount,
        total_balance_at_end_poll: None,
        snapped_staked_amount: None,
        _status: None,
    };

    poll.save_with_index(deps.storage)?;

    // Response
    Ok(Response {
        submessages: vec![],
        messages: vec![],
        attributes: vec![
            attr("action", "create_poll"),
            attr("creator", proposer.as_str()),
            attr("poll_id", poll.id.to_string()),
            attr("end_height", poll.end_height),
        ],
        data: None,
    })
}

pub fn cast_vote(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    poll_id: u64,
    option: VoteOption,
    amount: Uint128,
) -> ContractResult<Response> {
    // Validate
    let poll_state = PollState::load(deps.storage)?;

    if poll_id == 0 || poll_state.poll_count < poll_id {
        return Err(ContractError::Std(StdError::generic_err("Poll does not exist")));
    }

    let mut poll = Poll::load(deps.storage, &poll_id)?;

    if !poll.in_progress(env.block.height) {
        return Err(ContractError::Std(StdError::generic_err("Poll is not in progress")));
    }

    // Check the voter already has a vote on the poll
    if poll.is_voted(deps.storage, &info.sender) {
        return Err(ContractError::Std(StdError::generic_err("User has already voted.")));
    }

    let contract_available_balance = load_available_balance(deps.as_ref())?;
    let mut staker_state = StakerState::load_safe(deps.storage, &info.sender)?;

    if !staker_state.can_vote(deps.storage, contract_available_balance, amount)? {
        return Err(ContractError::Std(StdError::generic_err("User does not have enough staked tokens.")));
    }

    // Execute
    poll.vote(deps.storage, &mut staker_state, option.clone(), amount)?;
    poll.snapshot_staked_amount(deps.storage, env.block.height, contract_available_balance).ok(); //snapshot 실패하더라도 무시

    poll.save(deps.storage)?;
    staker_state.save(deps.storage)?;


    // Response
    Ok(Response {
        submessages: vec![],
        messages: vec![],
        attributes: vec![
            attr("action", "cast_vote"),
            attr("poll_id", &poll_id.to_string()),
            attr("amount", &amount.to_string()),
            attr("voter", info.sender.as_str()),
            attr("voter_option", option.to_string()),
        ],
        data: None,
    })
}

pub fn end_poll(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    poll_id: u64,
) -> ContractResult<Response> {
    // Validate
    let mut poll = Poll::load(deps.storage, &poll_id)?;

    if poll.status != PollStatus::InProgress {
        return Err(ContractError::Std(StdError::generic_err("Poll is not in progress")));
    }

    if poll.end_height > env.block.height {
        return Err(ContractError::Std(StdError::generic_err("Voting period has not expired")));
    }

    // Execute
    let mut messages: Vec<CosmosMsg> = vec![];

    let contract_config = ContractConfig::load(deps.storage)?;
    let mut poll_state = PollState::load(deps.storage)?;

    let (poll_result, staked_amount) = poll.get_result(deps.as_ref())?;

    poll.status = if poll_result == PollResult::Passed {
        PollStatus::Passed
    } else {
        PollStatus::Rejected
    };

    // Refunds deposit only when quorum is reached
    if poll_result != PollResult::QuorumNotReached && !poll.deposit_amount.is_zero() {
        messages.push(
            message_factories::cw20_transfer(
                &contract_config.governance_token,
                &poll.creator,
                poll.deposit_amount,
            )
        )
    }

    // Update poll status
    poll.total_balance_at_end_poll = Some(staked_amount);
    poll.save_with_index(deps.storage)?;

    // Decrease total deposit amount
    poll_state.total_deposit = poll_state.total_deposit.checked_sub(poll.deposit_amount)?;
    poll_state.save(deps.storage)?;

    // Response
    Ok(Response {
        submessages: vec![],
        messages,
        attributes: vec![
            attr("action", "end_poll"),
            attr("poll_id", poll_id.to_string()),
            attr("result", poll_result.to_string()),
            attr("passed", (poll_result == PollResult::Passed).to_string()),
        ],
        data: None,
    })
}

pub const REPLY_EXECUTION: u64 = 1;

pub fn execute_poll(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    poll_id: u64,
) -> ContractResult<Response> {
    // Validate
    let poll_config = PollConfig::load(deps.storage)?;
    let poll = Poll::load(deps.storage, &poll_id)?;

    if poll.status != PollStatus::Passed {
        return Err(ContractError::Std(StdError::generic_err("Poll is not in passed status")));
    }

    if poll.end_height + poll_config.execution_delay_period > env.block.height {
        return Err(ContractError::Std(StdError::generic_err("Execution delay period has not expired")));
    }

    let mut executions = poll.executions;
    if executions.is_empty() {
        return Err(ContractError::Std(StdError::generic_err("The poll does not have executions")));
    }

    // Execute
    executions.sort();

    // Response
    PollExecutionContext {
        poll_id: poll.id,
        execution_count: executions.len() as u64,
    }.save(deps.storage)?;

    let run_execution = SubMsg {
        id: REPLY_EXECUTION,
        msg: CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: env.contract.address.to_string(),
            send: vec![],
            msg: to_binary(&ExecuteMsg::RunExecution {
                executions: executions.iter().map(|e| ExecutionMsg::from(e)).collect(),
            })?,
        }),
        gas_limit: None,
        reply_on: ReplyOn::Always,
    };

    Ok(Response {
        submessages: vec![run_execution],
        messages: vec![],
        attributes: vec![
            attr("action", "execute_poll"),
            attr("poll_id", poll_id.to_string()),
        ],
        data: None,
    })
}

pub fn run_execution(
    _deps: DepsMut,
    env: Env,
    info: MessageInfo,
    mut executions: Vec<ExecutionMsg>,
) -> ContractResult<Response> {
    if env.contract.address != info.sender {
        return Err(ContractError::Unauthorized {});
    }

    executions.sort_by_key(|e| e.order);

    let messages: Vec<CosmosMsg> = executions.iter().map(|e| {
        CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: e.contract.to_string(),
            send: vec![],
            msg: e.msg.clone(),
        })
    }).collect();

    Ok(Response {
        submessages: vec![],
        messages,
        attributes: vec![
            attr("action", "execution_runner"),
            attr("execution_count", executions.len()),
        ],
        data: None,
    })
}

pub fn reply_execution(
    deps: DepsMut,
    _env: Env,
    msg: Reply,
) -> ContractResult<Response> {
    let poll_execution_context = PollExecutionContext::load(deps.storage)?;
    let mut poll = Poll::load(deps.storage, &poll_execution_context.poll_id)?;

    if poll.status == PollStatus::Failed || poll.status == PollStatus::Executed {
        return Err(ContractError::Std(StdError::generic_err("Already executed")));
    }

    poll.status = if msg.result.is_ok() {
        PollStatus::Executed
    } else {
        PollStatus::Failed
    };

    poll.save_with_index(deps.storage)?;
    PollExecutionContext::clear(deps.storage);

    Ok(Response {
        submessages: vec![],
        messages: vec![],
        attributes: vec![
            attr("action", "reply_execution"),
            attr("poll_status", poll.status.to_string()),
        ],
        data: None,
    })
}

pub fn snapshot_poll(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    poll_id: u64,
) -> ContractResult<Response> {
    // Validate
    let mut poll = Poll::load(deps.storage, &poll_id)?;

    if poll.status != PollStatus::InProgress {
        return Err(ContractError::Std(StdError::generic_err("Poll is not in progress")));
    }

    // Execute
    let contract_available_balance = load_available_balance(deps.as_ref())?;
    let staked_amount = poll.snapshot_staked_amount(
        deps.storage,
        env.block.height,
        contract_available_balance,
    )?;
    poll.save(deps.storage)?;

    // Response
    Ok(Response {
        submessages: vec![],
        messages: vec![],
        attributes: vec![
            attr("action", "snapshot_poll"),
            attr("poll_id", poll_id.to_string()),
            attr("staked_amount", staked_amount),
        ],
        data: None,
    })
}

// Validate_quorum returns an error if the quorum is invalid
/// (we require 0-1)
fn validate_quorum(quorum: Decimal) -> StdResult<()> {
    if quorum > Decimal::one() {
        Err(StdError::generic_err("quorum must be 0 to 1"))
    } else {
        Ok(())
    }
}

// Validate_threshold returns an error if the threshold is invalid
/// (we require 0-1)
fn validate_threshold(threshold: Decimal) -> StdResult<()> {
    if threshold > Decimal::one() {
        Err(StdError::generic_err("threshold must be 0 to 1"))
    } else {
        Ok(())
    }
}

// Validate_title returns an error if the title is invalid
fn validate_title(title: &str) -> StdResult<()> {
    if title.len() < MIN_TITLE_LENGTH {
        Err(StdError::generic_err("Title too short"))
    } else if title.len() > MAX_TITLE_LENGTH {
        Err(StdError::generic_err("Title too long"))
    } else {
        Ok(())
    }
}

// Validate_description returns an error if the description is invalid
fn validate_description(description: &str) -> StdResult<()> {
    if description.len() < MIN_DESC_LENGTH {
        Err(StdError::generic_err("Description too short"))
    } else if description.len() > MAX_DESC_LENGTH {
        Err(StdError::generic_err("Description too long"))
    } else {
        Ok(())
    }
}

// Validate_link returns an error if the link is invalid
fn validate_link(link: &Option<String>) -> StdResult<()> {
    if let Some(link) = link {
        if link.len() < MIN_LINK_LENGTH {
            Err(StdError::generic_err("Link too short"))
        } else if link.len() > MAX_LINK_LENGTH {
            Err(StdError::generic_err("Link too long"))
        } else {
            Ok(())
        }
    } else {
        Ok(())
    }
}