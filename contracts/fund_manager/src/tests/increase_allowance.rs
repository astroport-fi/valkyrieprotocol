use cosmwasm_std::{Addr, Env, MessageInfo, Response, Uint128};

use valkyrie::common::ContractResult;
use valkyrie::mock_querier::{custom_deps, CustomDeps};
use valkyrie::test_constants::campaign_manager::campaign_manager_sender;
use valkyrie::test_constants::default_sender;
use valkyrie::test_constants::fund_manager::{FUND_MANAGER, fund_manager_env, MANAGING_TOKEN};
use valkyrie::test_utils::{expect_generic_err, expect_invalid_zero_amount_err, expect_unauthorized_err};

use crate::executions::increase_allowance;
use crate::states::{Allowance, ContractState};

pub fn exec(
    deps: &mut CustomDeps,
    env: Env,
    info: MessageInfo,
    address: String,
    amount: Uint128,
) -> ContractResult<Response> {
    increase_allowance(
        deps.as_mut(),
        env,
        info,
        address,
        amount,
    )
}

pub fn will_success(
    deps: &mut CustomDeps,
    address: String,
    amount: Uint128,
) -> (Env, MessageInfo, Response) {
    let env = fund_manager_env();
    let info = campaign_manager_sender();

    let response = exec(
        deps,
        env.clone(),
        info.clone(),
        address,
        amount,
    ).unwrap();

    (env, info, response)
}

#[test]
fn succeed() {
    let mut deps = custom_deps();
    deps.querier.with_token_balances(&[(
        MANAGING_TOKEN,
        &[(FUND_MANAGER, &Uint128::new(10000))],
    )]);

    super::instantiate::default(&mut deps);

    let address = Addr::unchecked("Addr1");
    let amount = Uint128::new(100);
    will_success(&mut deps, address.to_string(), amount.clone());

    let allowance = Allowance::load(&deps.storage, &address).unwrap();
    assert_eq!(allowance, Allowance {
        address: address.clone(),
        allowed_amount: amount.clone(),
        remain_amount: amount.clone(),
    });

    let state = ContractState::load(&deps.storage).unwrap();
    assert_eq!(state.remain_allowance_amount, amount.clone());

    will_success(&mut deps, address.to_string(), amount.clone());

    let allowance = Allowance::load(&deps.storage, &address).unwrap();
    assert_eq!(allowance, Allowance {
        address: address.clone(),
        allowed_amount: amount + amount,
        remain_amount: amount + amount,
    });

    let state = ContractState::load(&deps.storage).unwrap();
    assert_eq!(state.remain_allowance_amount, amount + amount);
}

#[test]
fn failed_invalid_permission() {
    let mut deps = custom_deps();

    super::instantiate::default(&mut deps);

    let result = exec(
        &mut deps,
        fund_manager_env(),
        default_sender(),
        "Address".to_string(),
        Uint128::new(100),
    );
    expect_unauthorized_err(&result);
}

#[test]
fn failed_overflow_free_balance() {
    let mut deps = custom_deps();
    deps.querier.with_token_balances(&[(
        MANAGING_TOKEN,
        &[(FUND_MANAGER, &Uint128::new(1000))],
    )]);

    super::instantiate::default(&mut deps);

    will_success(&mut deps, "Address1".to_string(), Uint128::new(1000));

    let result = exec(
        &mut deps,
        fund_manager_env(),
        campaign_manager_sender(),
        "Address2".to_string(),
        Uint128::new(1),
    );
    expect_generic_err(&result, "Insufficient balance");
}

#[test]
fn failed_zero_amount() {
    let mut deps = custom_deps();

    super::instantiate::default(&mut deps);

    let result = exec(
        &mut deps,
        fund_manager_env(),
        campaign_manager_sender(),
        "Address".to_string(),
        Uint128::zero(),
    );

    expect_invalid_zero_amount_err(&result);
}
