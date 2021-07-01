use cosmwasm_std::{Env, MessageInfo, Response, Uint128};
use cosmwasm_std::testing::mock_env;

use valkyrie::common::ContractResult;
use valkyrie::governance::execute_msgs::StakingConfigInitMsg;
use valkyrie::mock_querier::{custom_deps, CustomDeps};
use valkyrie::test_utils::default_sender;

use crate::staking::executions::instantiate;
use crate::staking::states::{StakingConfig, StakingState};
use crate::tests::WITHDRAW_DELAY;

pub fn exec(deps: &mut CustomDeps, env: Env, info: MessageInfo, withdraw_delay: u64) -> ContractResult<Response> {
    let msg = StakingConfigInitMsg {
        withdraw_delay
    };

    instantiate(deps.as_mut(), env, info, msg)
}

pub fn default(deps: &mut CustomDeps) -> (Env, MessageInfo, Response) {
    let env = mock_env();
    let info = default_sender();

    let response = exec(
        deps,
        env.clone(),
        info.clone(),
        WITHDRAW_DELAY,
    ).unwrap();

    (env, info, response)
}

#[test]
fn succeed() {
    let mut deps = custom_deps(&[]);

    default(&mut deps);

    // Validate
    let staking_config = StakingConfig::load(&deps.storage).unwrap();
    assert_eq!(staking_config.withdraw_delay, WITHDRAW_DELAY);

    let staking_state = StakingState::load(&deps.storage).unwrap();
    assert_eq!(staking_state.total_share, Uint128::zero())
}
