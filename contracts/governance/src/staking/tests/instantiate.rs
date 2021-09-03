use cosmwasm_std::{Env, MessageInfo, Response, Uint128};

use valkyrie::common::ContractResult;
use valkyrie::mock_querier::{custom_deps, CustomDeps};
use valkyrie::test_constants::default_sender;
use valkyrie::test_constants::governance::governance_env;

use crate::staking::executions::instantiate;
use crate::staking::states::StakingState;
use valkyrie::governance::models::DistributionPlan;
use valkyrie::governance::execute_msgs::DistributionConfigMsg;

pub fn exec(
    deps: &mut CustomDeps,
    env: Env,
    info: MessageInfo,
    plan: Vec<DistributionPlan>,
) -> ContractResult<Response> {
    let msg = DistributionConfigMsg {
        plan,
    };

    instantiate(deps.as_mut(), env, info, msg)
}

pub fn default(deps: &mut CustomDeps) -> (Env, MessageInfo, Response) {
    let env = governance_env();
    let info = default_sender();

    let response = exec(
        deps,
        env.clone(),
        info.clone(),
        vec![],
    ).unwrap();

    (env, info, response)
}

#[test]
fn succeed() {
    let mut deps = custom_deps();

    default(&mut deps);

    // Validate
    let staking_state = StakingState::load(&deps.storage).unwrap();
    assert_eq!(staking_state.total_share, Uint128::zero())
}