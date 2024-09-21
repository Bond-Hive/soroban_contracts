#![cfg(test)]
extern crate std;

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger, MockAuth, MockAuthInvoke},
    Address, BytesN, Env, String, IntoVal
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
#[should_panic(expected = "HostError: Error(Contract, #3)")]
fn test_not_double_initialization() {
    let e = Env::default();
    e.mock_all_auths();

    let admin = Address::generate(&e);

    let token = create_token_contract(&e, &admin);
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

    // Test that the contract cannot be initialized a second time
    let token2 = create_token_contract(&e, &admin);
    vault.initialize(
        &install_token_wasm(&e),
        &token2.address,
        &admin,
        &(e.ledger().timestamp()),
         // end_time 10 minutes from now
        &(e.ledger().timestamp() + 600),
        &300,
        &admin,
        &100,
        &String::from_str(&e, "BOND"),
    );
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #2)")]
fn try_deposit_without_quote() {
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

    // Mint tokens to the user to deposit
    token_client.mint(&user, &1000);

    let deposit_amount = 200;

    // Try to deposit before setting quote
    vault.deposit(&user, &deposit_amount);
}

/// Test withdrawing funds before the contract reaches maturity
#[test]
#[should_panic(expected = "HostError: Error(Contract, #5)")]
fn test_withdraw_before_maturity() {
    let e = Env::default();
    e.mock_all_auths();

    let admin = Address::generate(&e);
    let user = Address::generate(&e);

    let token = create_token_contract(&e, &admin);
    let token_client = token::Client::new(&e, &token.address);
    let vault = VaultClient::new(&e, &e.register_contract(None, crate::Vault {}));

    // Initialize the vault
    let _ = vault.initialize(
        &install_token_wasm(&e),
        &token.address,
        &admin,
        &(e.ledger().timestamp()),
        &(e.ledger().timestamp() + 600),
        &300,
        &admin,
        &100,
        &String::from_str(&e, "BOND"),
    );

    // Mint tokens to the user
    token_client.mint(&user, &1000);

    // Set the quote and deposit
    vault.set_quote(&10000000);
    vault.deposit(&user, &200);

    // Attempt to withdraw before maturity, which should fail
    vault.withdraw(&user, &200);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #10)")]
fn test_contract_pause_deposit() {
    let e = Env::default();
    e.mock_all_auths();
 
    let admin = Address::generate(&e);
    let user = Address::generate(&e);

    let token = create_token_contract(&e, &admin);
    let token_client = token::Client::new(&e, &token.address);
    let vault = VaultClient::new(&e, &e.register_contract(None, crate::Vault {}));

    // Initialize the vault
    let _ = vault.initialize(
        &install_token_wasm(&e),
        &token.address,
        &admin,
        &(e.ledger().timestamp()),
        &(e.ledger().timestamp() + 600),
        &300,
        &admin,
        &100,
        &String::from_str(&e, "BOND"),
    );

    // Mint tokens to the user
    token_client.mint(&user, &1000);

    // Set the quote and deposit
    vault.set_quote(&10000000);
    vault.deposit(&user, &200);
    vault.set_contract_stopped(&true);
    vault.deposit(&user, &200);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #11)")]
fn test_override_quote() {
    let e = Env::default();
    e.mock_all_auths();

    let admin = Address::generate(&e);
    let user = Address::generate(&e);

    let token = create_token_contract(&e, &admin);
    let token_client = token::Client::new(&e, &token.address);
    let vault = VaultClient::new(&e, &e.register_contract(None, crate::Vault {}));

    // Initialize the vault
    let _ = vault.initialize(
        &install_token_wasm(&e),
        &token.address,
        &admin,
        &(e.ledger().timestamp()),
        &(e.ledger().timestamp() + 600),
        &300,
        &admin,
        &100,
        &String::from_str(&e, "BOND"),
    );

    // Mint tokens to the user
    token_client.mint(&user, &1000);

    // Set the quote and deposit
    vault.set_quote(&10000000);
    vault.set_quote(&20000000);
}

#[test]
fn test_full() {
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

    // Mint tokens to the user to deposit
    token_client.mint(&user, &1000);

        // Set the quote
        let set_quote_result = vault.set_quote(&10000000);
        assert_eq!(set_quote_result, 10000000);

    let deposit_amount = 200;

    let share_address = vault.bond_id();
    let share_client = token::Client::new(&e, &share_address);

    let share_balance = share_client.balance(&user);
    assert_eq!(share_balance, 0);
    
    // Try to deposit before setting quote
    let deposit_result = vault.deposit(&user, &deposit_amount);
    // ensure the returned number is greater than 0
    assert_eq!(deposit_result, 200);

    let total_deposit = vault.total_deposit();
    assert_eq!(total_deposit, 200);

    let total_bonds = vault.total_bonds();
    assert_eq!(total_bonds, 200);

    // Move time forward to simulate end time and set total redemption
    e.ledger().set_timestamp(e.ledger().timestamp() + 601);

    // Mint tokens to the treasury to simulate yield
    token_client.mint(&admin, &1000);

    // Set total redemption value
    let set_redemption_result = vault.set_total_redemption(&300);
    assert_eq!(set_redemption_result, 300);

    let share_balance = share_client.balance(&user);
    assert_eq!(share_balance, 200);
    
    // Withdraw funds by burning shares and getting back principal + yield
    let withdraw_result = vault.withdraw(&user, &share_balance);
    assert_eq!(withdraw_result, 300);
}

#[test]
#[should_panic(expected = "HostError: Error(Auth, InvalidAction)")]
fn try_mint_bonds_directly() {
    let e = Env::default();

    let admin = Address::generate(&e);
    let token = create_token_contract(&e, &admin);
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

    // try to mint the bonds directly
    let bond_id = vault.bond_id();
    let bond_client = token::Client::new(&e, &bond_id);

    bond_client
        .mock_auths(&[MockAuth {
            address: &admin,
            invoke: &MockAuthInvoke {
                contract: &bond_id,
                fn_name: "mint",
                args: (&admin, 123i128).into_val(&e),
                sub_invokes: &[],
            },
        }])
        .mint(&admin,  &123);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #3)")]
fn try_reinitialize_after_maturity() {
    let e = Env::default();
    e.mock_all_auths();

    let admin = Address::generate(&e);

    let token = create_token_contract(&e, &admin);
    let vault = VaultClient::new(&e, &e.register_contract(None, crate::Vault {}));

    // Initialize the vault with an end time of 10 seconds from now
    let _ = vault.initialize(
        &install_token_wasm(&e),
        &token.address,
        &admin,
        &(e.ledger().timestamp()),
        &(e.ledger().timestamp() + 10),
        &300,
        &admin,
        &100,
        &String::from_str(&e, "BOND"),
    );

    // Simulate the time passing beyond maturity
    e.ledger().set_timestamp(e.ledger().timestamp() + 11);

    // Attempt to reinitialize the contract, which should panic
    vault.initialize(
        &install_token_wasm(&e),
        &token.address,
        &admin,
        &(e.ledger().timestamp()),
        &(e.ledger().timestamp() + 600),
        &300,
        &admin,
        &100,
        &String::from_str(&e, "BOND"),
    );
}

#[test]
fn send_bonds_to_another_wallet_and_withdraw() {
    let e = Env::default();
    e.mock_all_auths();

    let admin = Address::generate(&e);
    let user1 = Address::generate(&e);
    let user2 = Address::generate(&e);

    let token = create_token_contract(&e, &admin);
    let token_client = token::Client::new(&e, &token.address);
    let vault = VaultClient::new(&e, &e.register_contract(None, crate::Vault {}));

    // Initialize the vault
    let _ = vault.initialize(
        &install_token_wasm(&e),
        &token.address,
        &admin,
        &(e.ledger().timestamp()),
        &(e.ledger().timestamp() + 600),
        &300,
        &admin,
        &100,
        &String::from_str(&e, "BOND"),
    );

    // Mint tokens to the first user
    token_client.mint(&user1, &1000);

    // Set the quote and deposit from user1
    vault.set_quote(&10000000);
    vault.deposit(&user1, &200);

    // Transfer bonds from user1 to user2
    let bond_id = vault.bond_id();
    let bond_client = token::Client::new(&e, &bond_id);
    bond_client.transfer(&user1, &user2, &200);

    // Move time forward to simulate end time and set total redemption
    e.ledger().set_timestamp(e.ledger().timestamp() + 601);

    // Mint tokens to the treasury to simulate yield
    token_client.mint(&admin, &1000);

    // Set total redemption value
    vault.set_total_redemption(&300);

    // Withdraw funds by user2, who now holds the bonds
    let share_balance = bond_client.balance(&user2);
    assert_eq!(share_balance, 200);

    let withdraw_result = vault.withdraw(&user2, &share_balance);
    assert_eq!(withdraw_result, 300);
}
