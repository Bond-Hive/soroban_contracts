#![cfg(test)]
extern crate std;

use crate::{token, FarmClient};
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    Address, BytesN, Env, String,
};

fn create_token_contract<'a>(e: &Env, admin: &Address) -> token::Client<'a> {
    token::Client::new(e, &e.register_stellar_asset_contract(admin.clone()))
}

fn install_token_wasm(e: &Env) -> BytesN<32> {
    soroban_sdk::contractimport!(file = "../soroban_token_contract.wasm");
    e.deployer().upload_contract_wasm(WASM)
}

#[test]
fn test_farm() {
    let e = Env::default();
    e.mock_all_auths();

    let admin = Address::generate(&e);
    let user = Address::generate(&e);

    let rewarded_token1 = create_token_contract(&e, &admin);
    let rewarded_token2 = create_token_contract(&e, &admin);
    let token_to_farm = create_token_contract(&e, &admin);

    let burn_wallet = Address::generate(&e); // Simulate a burn wallet address

    let farm = FarmClient::new(&e, &e.register_contract(None, crate::Farm {}));

    // Initialize the farm contract
    let result = farm.initialize(
        &admin,
        &rewarded_token1.address,
        &Some(rewarded_token2.address.clone()),  // Wrapping the Address in Some()
        &install_token_wasm(&e),
        &(e.ledger().timestamp() + 10000),
        &burn_wallet, // Pass burn wallet address
    );
    let expected = String::from_str(&e, "Ok");

    // Ensure the vault initialization returned "Ok"
    assert_eq!(result, expected);

    // Create a new pool
    let pool_id = farm.create_pool(
        &token_to_farm.address,
        &(e.ledger().timestamp()),
        &1, // Reward ratio 1
        &1, // Reward ratio 2
    );
    assert_eq!(pool_id, 0, "Pool creation failed");

    // Mint some tokens to the user to deposit
    token_to_farm.mint(&user, &1000);
    rewarded_token1.mint(&farm.address, &1000);
    rewarded_token2.mint(&farm.address, &1000);

    // Deposit tokens into the pool
    let deposit_amount = 10;
    let deposit_result = farm.deposit(&user, &deposit_amount, &pool_id);
    assert!(deposit_result > 0);

    // Move time forward to simulate the passage of time
    e.ledger().set_timestamp(e.ledger().timestamp() + 5000);

    // Withdraw tokens from the pool
    let withdraw_amount = 10;
    let withdraw_result = farm.withdraw(&user, &withdraw_amount, &pool_id);
    assert!(withdraw_result > 0);
}
