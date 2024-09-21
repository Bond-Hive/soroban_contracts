#![cfg(test)]
extern crate std;

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    Address, Env, String
};

fn create_token_contract<'a>(
    e: &Env,
    admin: &Address,
) -> (token::Client<'a>, token::StellarAssetClient<'a>) {
    let sac = e.register_stellar_asset_contract(admin.clone());
    (
        token::Client::new(e, &sac),
        token::StellarAssetClient::new(e, &sac),
    )
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #11)")]
fn test_not_double_initialization() {
    let e = Env::default();
    e.mock_all_auths();

    let admin = Address::generate(&e);

    let rewarded_token1 = create_token_contract(&e, &admin);
    let rewarded_token2 = create_token_contract(&e, &admin);
    let token = create_token_contract(&e, &admin);

    let farm = FarmClient::new(&e, &e.register_contract(None, crate::Farm {}));

    // Initialize the farm contract
    let result = farm.initialize(
        &admin,
        &rewarded_token1.0.address,
        &Some(rewarded_token2.0.address.clone()),
        &token.0.address,
        &(e.ledger().timestamp() + 10000),
        &10,
        &Some(10),
    );
    let expected = String::from_str(&e, "Ok");
    // Ensure the vault initialization returned "Ok"
    assert_eq!(result, expected);

    let rewarded_token3 = create_token_contract(&e, &admin);
    let rewarded_token4 = create_token_contract(&e, &admin);
    let token_to_farm2 = create_token_contract(&e, &admin);


    // Test that the contract cannot be initialized a second time
    farm.initialize(
        &admin,
        &rewarded_token3.0.address,
        &Some(rewarded_token4.0.address.clone()),
        &token_to_farm2.0.address,
        &(e.ledger().timestamp() + 10000),
        &10,
        &Some(10),
    );
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #6)")]
fn deposit_without_rewards() {
    let e = Env::default();
    e.mock_all_auths();

    let admin = Address::generate(&e);
    let user = Address::generate(&e);

    let rewarded_token1 = create_token_contract(&e, &admin);
    let rewarded_token2 = create_token_contract(&e, &admin);
    let token = create_token_contract(&e, &admin);
    let token_admin_client = token.1;

    token_admin_client.mint(&user, &1000);

    let farm = FarmClient::new(&e, &e.register_contract(None, crate::Farm {}));

    // Initialize the farm contract
    let result = farm.initialize(
        &admin,
        &rewarded_token1.0.address,
        &Some(rewarded_token2.0.address.clone()),
        &token.0.address,
        &(e.ledger().timestamp() + 10000),
        &100000000,
        &Some(100000000),
    );
    let expected = String::from_str(&e, "Ok");

    // Ensure the vault initialization returned "Ok"
    assert_eq!(result, expected);

    // Create a new pool
    let pool_id = farm.create_pool(
        &(e.ledger().timestamp()),
        &10000000,
        &Some(10000000),
    );
    assert_eq!(pool_id, 0, "Pool creation failed");

    // Deposit tokens into the pool
    let deposit_amount = 10;
    let deposit_result = farm.deposit(&user, &deposit_amount, &pool_id);
    assert!(deposit_result > 0);
}

#[test]
fn test_withdraw_before_and_after_maturity() {
    let e = Env::default();
    e.mock_all_auths();

    let admin = Address::generate(&e);
    let user = Address::generate(&e);

    // Create token contracts for the reward tokens and the pool token
    let (rewarded_token1_client, rewarded_token1_admin) = create_token_contract(&e, &admin);
    let (rewarded_token2_client, rewarded_token2_admin) = create_token_contract(&e, &admin);
    let (pool_token_client, pool_token_admin) = create_token_contract(&e, &admin);

    // Mint tokens for the user to deposit
    pool_token_admin.mint(&user, &1000);

    // Initialize the farm contract
    let farm = FarmClient::new(&e, &e.register_contract(None, crate::Farm {}));
    let maturity = e.ledger().timestamp() + 10_000; // Maturity in 10,000 seconds
    let max_reward_ratio1 = 100000000; // Set max reward ratio to 1e6
    let max_reward_ratio2 = Some(100000000);

    let result = farm.initialize(
        &admin,
        &rewarded_token1_client.address,
        &Some(rewarded_token2_client.address.clone()),
        &pool_token_client.address,
        &maturity,
        &max_reward_ratio1,
        &max_reward_ratio2,
    );
    assert_eq!(result, String::from_str(&e, "Ok"));

    let total_reward_amount = 50000000000; // The total amount of reward tokens to allocate
    rewarded_token1_admin.mint(&farm.address, &total_reward_amount);    
    rewarded_token2_admin.mint(&farm.address, &total_reward_amount);    

    let reward_ratio1 = 10000000;
    let reward_ratio2 = Some(10000000);
    let pool_id = farm.create_pool(
        &e.ledger().timestamp(), // Start now
        &reward_ratio1,
        &reward_ratio2,
    );
    assert_eq!(pool_id, 0, "Pool creation failed");

    // User deposits tokens into the pool
    let deposit_amount = 100; // User deposits 100 tokens
    let deposit_result = farm.deposit(&user, &deposit_amount, &pool_id);
    assert_eq!(deposit_result, deposit_amount);

    // check that global allocated rewards are correct
    let global_allocated_rewards = farm.get_global_allocated_rewards();
    let current_time: u64 = e.ledger().timestamp();
    let qty = deposit_amount * (maturity as i128 - current_time as i128);

    assert_eq!(global_allocated_rewards.0, qty);
    assert_eq!(global_allocated_rewards.1, qty);

    // Move time forward to before maturity
    let time_elapsed_before_withdraw = 5000; // 5,000 seconds
    e.ledger().set_timestamp(e.ledger().timestamp() + time_elapsed_before_withdraw);

    // User withdraws part of the deposit before maturity
    let withdraw_amount_before_maturity = 50; // Withdraw 50 tokens
    let withdraw_result = farm.withdraw(&user, &withdraw_amount_before_maturity, &pool_id);
    assert_eq!(withdraw_result, withdraw_amount_before_maturity);

    // Check user's balances and accrued rewards
    // Get user data
    let user_data = farm.get_user_info(&user, &pool_id);
    assert_eq!(user_data.deposited, deposit_amount - withdraw_amount_before_maturity);

    // Calculate expected accrued rewards
    let time_elapsed = time_elapsed_before_withdraw;
    let expected_accrued_rewards1 = (deposit_amount as i128 * reward_ratio1 as i128 * time_elapsed as i128) / 10i128.pow(DECIMALS);
    let expected_accrued_rewards2 = (deposit_amount as i128 * reward_ratio2.unwrap() as i128 * time_elapsed as i128) / 10i128.pow(DECIMALS);

    // Check the user's reward balances
    let user_reward_token1_balance = rewarded_token1_client.balance(&user);
    let user_reward_token2_balance = rewarded_token2_client.balance(&user);

    // The user should have received the accrued rewards up to the withdrawal time
    assert_eq!(user_reward_token1_balance, expected_accrued_rewards1);
    assert_eq!(user_reward_token2_balance, expected_accrued_rewards2);

    // Move time forward to after maturity
    let time_to_maturity = maturity - e.ledger().timestamp();
    e.ledger().set_timestamp(e.ledger().timestamp() + time_to_maturity + 1);

    // User withdraws the remaining amount after maturity
    let withdraw_amount_after_maturity = deposit_amount - withdraw_amount_before_maturity;
    let withdraw_result2 = farm.withdraw(&user, &withdraw_amount_after_maturity, &pool_id);
    assert_eq!(withdraw_result2, withdraw_amount_after_maturity);
}

#[test]
fn test_full_withdraw_before_maturity() {
    let e = Env::default();
    e.mock_all_auths();

    let admin = Address::generate(&e);
    let user = Address::generate(&e);

    let (rewarded_token1_client, rewarded_token1_admin) = create_token_contract(&e, &admin);
    let (pool_token_client, pool_token_admin) = create_token_contract(&e, &admin);

    pool_token_admin.mint(&user, &1000);

    let farm = FarmClient::new(&e, &e.register_contract(None, crate::Farm {}));
    let maturity = e.ledger().timestamp() + 10000;
    let max_reward_ratio1 = 100000000;

    let result = farm.initialize(
        &admin,
        &rewarded_token1_client.address,
        &None,
        &pool_token_client.address,
        &maturity,
        &max_reward_ratio1,
        &None,
    );
    assert_eq!(result, String::from_str(&e, "Ok"));

    rewarded_token1_admin.mint(&farm.address, &50000000);

    let reward_ratio1 = 10000000;
    let pool_id = farm.create_pool(
        &e.ledger().timestamp(),
        &reward_ratio1,
        &None,
    );
    assert_eq!(pool_id, 0);

    let deposit_amount = 1;
    let deposit_result = farm.deposit(&user, &deposit_amount, &pool_id);
    assert_eq!(deposit_result, deposit_amount);

    let time_elapsed = 5000;
    e.ledger().set_timestamp(e.ledger().timestamp() + time_elapsed);

    // Withdraw full amount before maturity
    let withdraw_result = farm.withdraw(&user, &deposit_amount, &pool_id);
    assert_eq!(withdraw_result, deposit_amount);

    // Check user's pool token balance
    let user_pool_token_balance = pool_token_client.balance(&user);
    assert_eq!(user_pool_token_balance, 1000);

    // Check user's reward token balance
    let expected_rewards = (deposit_amount as i128 * reward_ratio1 as i128 * time_elapsed as i128) / 10i128.pow(DECIMALS);
    let user_reward_balance = rewarded_token1_client.balance(&user);
    assert_eq!(user_reward_balance, expected_rewards);
}
