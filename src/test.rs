#![cfg(test)]
extern crate std;

use crate::{token, VaultClient, VaultError};
use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, AuthorizedFunction, AuthorizedInvocation},
    Address, BytesN, Env, IntoVal,
};

fn create_token_contract<'a>(e: &Env, admin: &Address) -> token::Client<'a> {
    token::Client::new(e, &e.register_stellar_asset_contract(admin.clone()))
}

fn create_vault_contract<'a>(
    e: &Env,
    token_wasm_hash: &BytesN<32>,
    token: &Address,
    admin: &Address,
    start_time: u64,
    end_time: u64,
    quote_period: u64,
    treasury: &Address,
    min_deposit: u128,
) -> VaultClient<'a> {
    let vault = VaultClient::new(e, &e.register_contract(None, crate::Vault {}));
    vault.initialize(token_wasm_hash, token, admin, start_time, end_time, quote_period, treasury, min_deposit);
    vault
}

fn install_token_wasm(e: &Env) -> BytesN<32> {
    soroban_sdk::contractimport!(file = "./soroban_token_contract.wasm");
    e.deployer().upload_contract_wasm(WASM)
}

#[test]
fn test_vault_contract() {
    let e = Env::default();
    e.mock_all_auths();

    let admin1 = Address::generate(&e);
    let token = create_token_contract(&e, &admin1);

    let user1 = Address::generate(&e);
    let treasury = Address::generate(&e);

    let start_time = e.ledger().timestamp();
    let end_time = start_time + 100000;
    let quote_period = 600;
    let min_deposit = 100;

    let vault = create_vault_contract(&e, &install_token_wasm(&e), &token.address, &admin1, start_time, end_time, quote_period, &treasury, min_deposit);

    let contract_share = token::Client::new(&e, &vault.bond_id().unwrap());
    let token_share = token::Client::new(&e, &contract_share.address);

    token.mint(&user1, &1000);
    assert_eq!(token.balance(&user1), 1000);

    // Admin sets the quote
    vault.set_quote(1).unwrap();
    assert_eq!(vault.quote().unwrap(), 1);

    // User deposits to mint bonds
    vault.deposit(&user1, &200).unwrap();
    assert_eq!(
        e.auths(),
        std::vec![(
            user1.clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    vault.address.clone(),
                    symbol_short!("deposit"),
                    (&user1, 200_i128).into_val(&e)
                )),
                sub_invocations: std::vec![AuthorizedInvocation {
                    function: AuthorizedFunction::Contract((
                        token.address.clone(),
                        symbol_short!("transfer"),
                        (&user1, &treasury, 200_i128).into_val(&e)
                    )),
                    sub_invocations: std::vec![]
                }]
            }
        )]
    );

    assert_eq!(token_share.balance(&user1), 200);
    assert_eq!(token.balance(&user1), 800);
    assert_eq!(token.balance(&treasury), 200);

    // Fast forward time to after maturity
    e.ledger().set_timestamp(end_time + 1);

    // Admin sets the total redemption amount (principal + rewards)
    vault.set_total_redemption(300).unwrap();

    // User withdraws by burning bonds
    e.budget().reset_unlimited();
    vault.withdraw(&user1, &200).unwrap();
    assert_eq!(
        e.auths(),
        std::vec![(
            user1.clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    vault.address.clone(),
                    symbol_short!("withdraw"),
                    (&user1, 200_i128).into_val(&e)
                )),
                sub_invocations: std::vec![AuthorizedInvocation {
                    function: AuthorizedFunction::Contract((
                        token_share.address.clone(),
                        symbol_short!("transfer"),
                        (&user1, &vault.address, 200_i128).into_val(&e)
                    )),
                    sub_invocations: std::vec![]
                }]
            }
        )]
    );

    assert_eq!(token.balance(&user1), 1100); // 800 + 300 (principal + rewards)
    assert_eq!(token_share.balance(&user1), 0);
    assert_eq!(token.balance(&vault.address), 0);
}

#[test]
fn test_set_admin() {
    let e = Env::default();
    e.mock_all_auths();

    let admin1 = Address::generate(&e);
    let admin2 = Address::generate(&e);

    let token = create_token_contract(&e, &admin1);
    let treasury = Address::generate(&e);

    let start_time = e.ledger().timestamp();
    let end_time = start_time + 100000;
    let quote_period = 600;
    let min_deposit = 100;

    let vault = create_vault_contract(&e, &install_token_wasm(&e), &token.address, &admin1, start_time, end_time, quote_period, &treasury, min_deposit);

    // Test set_admin
    vault.set_admin(&admin2).unwrap();
    assert_eq!(vault.admin().unwrap(), admin2);
}

#[test]
fn test_error_cases() {
    let e = Env::default();
    e.mock_all_auths();

    let admin1 = Address::generate(&e);
    let token = create_token_contract(&e, &admin1);
    let user1 = Address::generate(&e);
    let treasury = Address::generate(&e);

    let start_time = e.ledger().timestamp();
    let end_time = start_time + 100000;
    let quote_period = 600;
    let min_deposit = 100;

    let vault = create_vault_contract(&e, &install_token_wasm(&e), &token.address, &admin1, start_time, end_time, quote_period, &treasury, min_deposit);

    // Test depositing without a quote
    let result = vault.deposit(&user1, &100);
    assert_eq!(result, Err(VaultError::QuoteRequired));

    // Admin sets the quote
    vault.set_quote(1).unwrap();

    // Test depositing less than minimum deposit
    let result = vault.deposit(&user1, &99);
    assert_eq!(result, Err(VaultError::InvalidAmount));

    // Test withdrawing before maturity
    let result = vault.withdraw(&user1, &100);
    assert_eq!(result, Err(VaultError::MaturityNotReached));

    // Fast forward time to after maturity
    e.ledger().set_timestamp(end_time + 1);

    // Test withdrawing before setting total redemption
    let result = vault.withdraw(&user1, &100);
    assert_eq!(result, Err(VaultError::AvailableRedemptionNotSet));
}
