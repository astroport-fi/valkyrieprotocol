use valkyrie::mock_querier::{CustomDeps, custom_deps};
use cosmwasm_std::{Env, ContractResult as CwContractResult, Response, Reply, Uint128, SubMsgExecutionResponse};
use valkyrie::common::ContractResult;
use crate::poll::executions::{reply_execution, REPLY_EXECUTION};
use crate::tests::{init_default, POLL_PROPOSAL_DEPOSIT};
use crate::poll::tests::create_poll::{mock_exec_msg, PROPOSER1, POLL_TITLE, POLL_DESCRIPTION, POLL_LINK};
use crate::poll::tests::cast_vote::VOTER1;
use valkyrie::governance::enumerations::{VoteOption, PollStatus};
use crate::poll::states::{PollExecutionContext, Poll};

pub fn exec(
    deps: &mut CustomDeps,
    env: Env,
    result: CwContractResult<SubMsgExecutionResponse>,
) -> ContractResult<Response> {
    reply_execution(deps.as_mut(), env, Reply {
        id: REPLY_EXECUTION,
        result,
    })
}

#[test]
fn succeed_success_reply() {
    let mut deps = custom_deps(&[]);

    init_default(deps.as_mut());

    let execution_msgs = vec![
        mock_exec_msg(2),
        mock_exec_msg(1),
        mock_exec_msg(3),
    ];

    super::create_poll::will_success(
        &mut deps,
        PROPOSER1,
        POLL_PROPOSAL_DEPOSIT,
        POLL_TITLE,
        POLL_DESCRIPTION,
        Some(POLL_LINK),
        execution_msgs.clone(),
    );
    crate::staking::tests::stake_governance_token::will_success(&mut deps, VOTER1, Uint128::new(100));

    let poll_id = 1u64;

    super::cast_vote::will_success(&mut deps, VOTER1, poll_id, VoteOption::Yes, Uint128::new(100));
    super::end_poll::will_success(&mut deps, poll_id);

    let (env, _, _) = super::execute_poll::will_success(&mut deps, poll_id);
    let context = PollExecutionContext::load(&deps.storage).unwrap();

    exec(&mut deps, env.clone(), CwContractResult::Ok(mock_subcall_response())).unwrap();

    assert!(PollExecutionContext::may_load(&deps.storage).unwrap().is_none());

    let poll = Poll::load(&deps.storage, &context.poll_id).unwrap();
    assert_eq!(poll.status, PollStatus::Executed);
}

#[test]
fn succeed_failed_reply() {
    let mut deps = custom_deps(&[]);

    init_default(deps.as_mut());

    let execution_msgs = vec![
        mock_exec_msg(2),
        mock_exec_msg(1),
        mock_exec_msg(3),
    ];

    super::create_poll::will_success(
        &mut deps,
        PROPOSER1,
        POLL_PROPOSAL_DEPOSIT,
        POLL_TITLE,
        POLL_DESCRIPTION,
        Some(POLL_LINK),
        execution_msgs.clone(),
    );
    crate::staking::tests::stake_governance_token::will_success(&mut deps, VOTER1, Uint128::new(100));

    let poll_id = 1u64;

    super::cast_vote::will_success(&mut deps, VOTER1, poll_id, VoteOption::Yes, Uint128::new(100));
    super::end_poll::will_success(&mut deps, poll_id);

    let (env, _, _) = super::execute_poll::will_success(&mut deps, poll_id);
    let context = PollExecutionContext::load(&deps.storage).unwrap();

    exec(&mut deps, env.clone(), CwContractResult::Err("Mock err".to_string())).unwrap();

    assert!(PollExecutionContext::may_load(&deps.storage).unwrap().is_none());

    let poll = Poll::load(&deps.storage, &context.poll_id).unwrap();
    assert_eq!(poll.status, PollStatus::Failed);
}

pub fn mock_subcall_response() -> SubMsgExecutionResponse {
    SubMsgExecutionResponse {
        events: vec![],
        data: None,
    }
}