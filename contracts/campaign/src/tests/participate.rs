use cosmwasm_std::{Addr, coin, Decimal, Env, MessageInfo, Response, Uint128, CosmosMsg, WasmMsg, to_binary, SubMsg};
use cosmwasm_std::testing::{mock_info, MOCK_CONTRACT_ADDR};

use valkyrie::campaign::enumerations::Referrer;
use valkyrie::common::ContractResult;
use valkyrie::mock_querier::{custom_deps, CustomDeps};
use valkyrie::test_utils::{contract_env, default_sender, expect_generic_err, DEFAULT_SENDER, expect_unauthorized_err};

use crate::executions::participate;
use crate::states::{CampaignState, Participation};
use crate::tests::{CAMPAIGN_DISTRIBUTION_AMOUNTS, CAMPAIGN_DISTRIBUTION_DENOM_NATIVE, TOKEN_CONTRACT, CAMPAIGN_ADMIN, CAMPAIGN_MANAGER};
use crate::tests::enable_booster::{DROP_BOOSTER_AMOUNT, PLUS_BOOSTER_AMOUNT};
use valkyrie::campaign_manager::execute_msgs::ExecuteMsg;

pub fn exec(
    deps: &mut CustomDeps,
    env: Env,
    info: MessageInfo,
    actor: String,
    referrer: Option<Referrer>,
) -> ContractResult<Response> {
    participate(deps.as_mut(), env, info, actor, referrer)
}

pub fn will_success(
    deps: &mut CustomDeps,
    participator: &str,
    referrer: Option<Referrer>,
) -> (Env, MessageInfo, Response) {
    let env = contract_env();
    let info = mock_info(participator, &[]);

    let response = exec(
        deps,
        env.clone(),
        info.clone(),
        participator.to_string(),
        referrer,
    ).unwrap();

    (env, info, response)
}

#[test]
fn succeed_without_referrer() {
    let mut deps = custom_deps(&[
        coin(100000000000u128, CAMPAIGN_DISTRIBUTION_DENOM_NATIVE),
    ]);

    super::instantiate::default(&mut deps);
    super::update_activation::will_success(&mut deps, true);

    let participator = Addr::unchecked("Participator");

    let (env, _, _) = will_success(&mut deps, participator.as_str(), None);

    let campaign_state = CampaignState::load(&deps.storage).unwrap();
    assert_eq!(campaign_state.participation_count, 1);
    assert_eq!(campaign_state.last_active_height, Some(env.block.height));
    assert_eq!(campaign_state.locked_balance, CAMPAIGN_DISTRIBUTION_AMOUNTS[0]);

    let participation = Participation::load(&deps.storage, &participator).unwrap();
    assert_eq!(participation, Participation {
        actor_address: participator.clone(),
        referrer_address: None,
        reward_amount: CAMPAIGN_DISTRIBUTION_AMOUNTS[0],
        participated_at: env.block.time,
        drop_booster_claimable: vec![(1, true)],
        drop_booster_distance_counts: vec![(1, vec![(0, 1)])],
        activity_booster_reward_amount: Uint128::zero(),
        plus_booster_reward_amount: Uint128::zero(),
    });
}

#[test]
fn succeed_with_referrer() {
    let mut deps = custom_deps(&[
        coin(100000000000u128, CAMPAIGN_DISTRIBUTION_DENOM_NATIVE),
    ]);

    super::instantiate::default(&mut deps);
    super::update_activation::will_success(&mut deps, true);

    let participator = Addr::unchecked("Participator");
    let referrer = Addr::unchecked("Referrer");

    let (referrer_env, _, _) = will_success(
        &mut deps,
        referrer.as_str(),
        Some(Referrer::Address("InvalidReferrer".to_string())),
    );

    let (env, _, _) = will_success(
        &mut deps,
        participator.as_str(),
        Some(Referrer::Address(referrer.to_string())),
    );

    let campaign_state = CampaignState::load(&deps.storage).unwrap();
    assert_eq!(campaign_state.participation_count, 2);
    assert_eq!(campaign_state.last_active_height, Some(env.block.height));
    assert_eq!(campaign_state.locked_balance, CAMPAIGN_DISTRIBUTION_AMOUNTS[0] + CAMPAIGN_DISTRIBUTION_AMOUNTS[0] + CAMPAIGN_DISTRIBUTION_AMOUNTS[1]);

    let participation = Participation::load(&deps.storage, &participator).unwrap();
    assert_eq!(participation, Participation {
        actor_address: participator.clone(),
        referrer_address: Some(referrer.clone()),
        reward_amount: CAMPAIGN_DISTRIBUTION_AMOUNTS[0],
        participated_at: env.block.time,
        drop_booster_claimable: vec![(1, true)],
        drop_booster_distance_counts: vec![(1, vec![(0, 1)])],
        activity_booster_reward_amount: Uint128::zero(),
        plus_booster_reward_amount: Uint128::zero(),
    });

    let referrer_participation = Participation::load(&deps.storage, &referrer).unwrap();
    assert_eq!(referrer_participation, Participation {
        actor_address: referrer.clone(),
        referrer_address: None,
        reward_amount: CAMPAIGN_DISTRIBUTION_AMOUNTS[0] + CAMPAIGN_DISTRIBUTION_AMOUNTS[1],
        participated_at: referrer_env.block.time,
        drop_booster_claimable: vec![(1, true)],
        drop_booster_distance_counts: vec![(1, vec![(0, 1), (1, 1)])],
        activity_booster_reward_amount: Uint128::zero(),
        plus_booster_reward_amount: Uint128::zero(),
    });
}

#[test]
fn succeed_with_booster() {
    let mut deps = custom_deps(&[
        coin(1000, CAMPAIGN_DISTRIBUTION_DENOM_NATIVE),
    ]);
    let activity_booster_multiplier = Decimal::percent(80);
    let min_participation_count = 10u64;
    deps.querier.with_booster_config(
        TOKEN_CONTRACT.to_string(),
        Decimal::percent(10),
        Decimal::percent(80),
        Decimal::percent(10),
        activity_booster_multiplier.clone(),
        min_participation_count,
    );

    super::instantiate::default(&mut deps);
    super::update_activation::will_success(&mut deps, true);

    //minimum participation for boosting
    for i in 1..min_participation_count {
        will_success(&mut deps, format!("participator{}", i).as_str(), None);
    }

    let referrer = "Referrer";
    will_success(&mut deps,  referrer, None);

    super::enable_booster::default(&mut deps);

    let participator = "Participator";
    will_success(
        &mut deps,
        participator,
        Some(Referrer::Address(referrer.to_string())),
    );

    let drop_booster = DROP_BOOSTER_AMOUNT.checked_div(Uint128::new(10)).unwrap();
    let activity_booster = activity_booster_multiplier * drop_booster;

    let referrer_participation = Participation::load(
        &deps.storage,
        &Addr::unchecked(referrer),
    ).unwrap();
    let reward_rate = Decimal::from_ratio(
        CAMPAIGN_DISTRIBUTION_AMOUNTS[1],
        CAMPAIGN_DISTRIBUTION_AMOUNTS.iter().sum::<Uint128>(),
    );
    assert_eq!(referrer_participation.activity_booster_reward_amount, reward_rate * activity_booster);
    assert_eq!(referrer_participation.plus_booster_reward_amount, Uint128::zero());

    let participation = Participation::load(
        &deps.storage,
        &Addr::unchecked(participator),
    ).unwrap();
    let reward_rate = Decimal::from_ratio(
        CAMPAIGN_DISTRIBUTION_AMOUNTS[0],
        CAMPAIGN_DISTRIBUTION_AMOUNTS.iter().sum::<Uint128>(),
    );
    assert_eq!(participation.activity_booster_reward_amount, reward_rate * activity_booster);
    assert_eq!(participation.plus_booster_reward_amount, Uint128::zero());

    let participator = "StakingParticipator";
    let voting_power = Decimal::percent(1);
    deps.querier.with_voting_powers(&[
        (&participator.to_string(), &voting_power),
    ]);

    will_success(&mut deps, participator, None);

    let participation = Participation::load(
        &deps.storage,
        &Addr::unchecked(participator),
    ).unwrap();
    let reward_rate = Decimal::from_ratio(
        CAMPAIGN_DISTRIBUTION_AMOUNTS[0],
        CAMPAIGN_DISTRIBUTION_AMOUNTS.iter().sum::<Uint128>(),
    );
    let plus_booster_amount = voting_power * PLUS_BOOSTER_AMOUNT;
    assert_eq!(participation.activity_booster_reward_amount, reward_rate * activity_booster);
    assert_eq!(participation.plus_booster_reward_amount, plus_booster_amount);
}

#[test]
fn succeed_insufficient_booster_reward_balance() {
    let mut deps = custom_deps(&[
        coin(1000, CAMPAIGN_DISTRIBUTION_DENOM_NATIVE),
    ]);

    super::instantiate::default(&mut deps);
    super::update_activation::will_success(&mut deps, true);
    will_success(&mut deps, "Participator1", None);
    super::enable_booster::will_success(
        &mut deps,
        Uint128::from(10u64),
        Uint128::from(18u64),
        Uint128::from(10u64),
        Decimal::percent(80),
    );

    deps.querier.with_voting_powers(&[
        (&"Participator2".to_string(), &Decimal::percent(70)),
        (&"Participator3".to_string(), &Decimal::percent(50)),
    ]);
    will_success(&mut deps, "Participator2", Some(Referrer::Address("Participator1".to_string())));
    will_success(&mut deps, "Participator3", None);
    let (_, _, response) = will_success(&mut deps, "Participator4", None);

    let participation = Participation::load(&deps.storage, &Addr::unchecked("Participator3")).unwrap();
    assert_eq!(participation.activity_booster_reward_amount, Uint128::from(4u64));
    assert_eq!(participation.plus_booster_reward_amount, Uint128::from(3u64));

    let participation = Participation::load(&deps.storage, &Addr::unchecked("Participator4")).unwrap();
    assert_eq!(participation.activity_booster_reward_amount, Uint128::from(4u64));
    assert_eq!(participation.plus_booster_reward_amount, Uint128::zero());

    //finish boosting msg must be first.
    assert_eq!(response.messages.first(), Some(&SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: CAMPAIGN_MANAGER.to_string(),
        funds: vec![],
        msg: to_binary(&ExecuteMsg::FinishBoosting {
            campaign: MOCK_CONTRACT_ADDR.to_string(),
        }).unwrap(),
    }))));
}

#[test]
fn succeed_proxy() {
    let mut deps = custom_deps(&[
        coin(1000, CAMPAIGN_DISTRIBUTION_DENOM_NATIVE),
    ]);

    super::instantiate::default(&mut deps);
    super::update_activation::will_success(&mut deps, true);
    will_success(&mut deps, "Participator1", None);

    super::update_contract_config::will_success(
        &mut deps,
        None,
        Some(vec!["Proxy".to_string()]),
    );

    let result = exec(
        &mut deps,
        contract_env(),
        mock_info("Proxy", &[]),
        "Participator2".to_string(),
        None,
    );
    assert!(result.is_ok());

    let result = exec(
        &mut deps,
        contract_env(),
        mock_info(CAMPAIGN_ADMIN, &[]),
        "Participator3".to_string(),
        None,
    );
    assert!(result.is_ok());
}

#[test]
fn failed_proxy_not_in_whitelist() {
    let mut deps = custom_deps(&[
        coin(1000, CAMPAIGN_DISTRIBUTION_DENOM_NATIVE),
    ]);

    super::instantiate::default(&mut deps);
    super::update_activation::will_success(&mut deps, true);

    super::update_contract_config::will_success(
        &mut deps,
        None,
        Some(vec!["Proxy".to_string()]),
    );

    let result = exec(
        &mut deps,
        contract_env(),
        mock_info("Non-Proxy", &[]),
        "Participator".to_string(),
        None,
    );
    expect_unauthorized_err(&result);
}

#[test]
fn failed_inactive_campaign() {
    let mut deps = custom_deps(&[]);

    super::instantiate::default(&mut deps);

    let result = exec(
        &mut deps,
        contract_env(),
        default_sender(),
        DEFAULT_SENDER.to_string(),
        None,
    );

    expect_generic_err(&result, "Inactive campaign");
}

#[test]
fn failed_twice() {
    let mut deps = custom_deps(&[
        coin(1000, CAMPAIGN_DISTRIBUTION_DENOM_NATIVE),
    ]);

    super::instantiate::default(&mut deps);
    super::update_activation::will_success(&mut deps, true);

    let (_, info, _) = will_success(&mut deps, "Participator1", None);

    let result = exec(
        &mut deps,
        contract_env(),
        info,
        "Participator1".to_string(),
        None,
    );

    expect_generic_err(&result, "Already participated");
}

#[test]
fn failed_insufficient_balance() {
    let mut deps = custom_deps(&[
        coin(5, CAMPAIGN_DISTRIBUTION_DENOM_NATIVE),
    ]);

    super::instantiate::default(&mut deps);
    super::update_activation::will_success(&mut deps, true);
    will_success(&mut deps, "Participator1", None);

    let result = exec(
        &mut deps,
        contract_env(),
        mock_info("Participator2", &[]),
        "Participator2".to_string(),
        None,
    );
    expect_generic_err(&result, "Insufficient balance");
}
