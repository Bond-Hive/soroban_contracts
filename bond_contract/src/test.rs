#![cfg(test)]
extern crate std;

use crate::{token, VaultClient};

use soroban_sdk::{
    testutils::{Address as _, Ledger},
    Address, BytesN, Env, String,
};

fn create_token_contract<'a>(e: &Env, admin: &Address) -> token::Client<'a> {
    token::Client::new(e, &e.register_stellar_asset_contract(admin.clone()))
}

fn install_token_wasm(e: &Env) -> BytesN<32> {
    // Ensure the path is correct relative to the current file
    soroban_sdk::contractimport!(file = "../soroban_token_contract.wasm");

    // Upload the WASM contract to the environment
    e.deployer().upload_contract_wasm(WASM)
}

#[test]
fn test() {
    let e = Env::default();
    e.mock_all_auths();

    let admin = Address::generate(&e);
    let user = Address::generate(&e);

    let token = create_token_contract(&e, &admin);
    let token_client = token::Client::new(&e, &token.address);
    let vault = VaultClient::new(&e, &e.register_contract(None, crate::Vault {}));

    // Create and initialize the vault contract
    let vault_result = vault.initialize(
        &install_token_wasm(&e),
        &token.address,
        &admin,
        &(e.ledger().timestamp()),
         // end_time 10 minutes from now
        &(e.ledger().timestamp() + 600),
        &300,
        &admin,
        &100,
        &String::from_str(&e, "BOND"),
    );

    let expected = String::from_str(&e, "Ok");

    // Ensure the vault initialization returned "Ok"
    assert_eq!(vault_result, expected);

    // Set the quote
    let set_quote_result = vault.set_quote(&10000000);
    assert_eq!(set_quote_result, 10000000);

    token_client.mint(&user, &1000);

    // Deposit an amount into the vault
    let deposit_amount = 200;
    let deposit_result: i128 = vault.deposit(&user, &deposit_amount);
    // ensure the returned number is greater than 0
    assert!(deposit_result > 0);

    // Move time forward to simulate end time and set total redemption
    e.ledger().set_timestamp(e.ledger().timestamp() + 601);

    // Mint tokens to the vault to simulate yield
    token_client.mint(&admin, &1000);

    // Set total redemption value
    let set_redemption_result = vault.set_total_redemption(&300);
    assert_eq!(set_redemption_result, 300);

    // Withdraw funds by burning shares and getting back principal + yield
    let withdraw_result = vault.withdraw(&user, &deposit_amount);
    assert!(withdraw_result > 0);
}
