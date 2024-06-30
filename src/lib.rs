#![no_std]

mod token;

use soroban_sdk::{
    contract, contractimpl, Address, BytesN, ConversionError, Env, IntoVal, TryFromVal, Val,
};
use token::create_contract;

#[derive(Clone, Copy)]
#[repr(u32)]
pub enum DataKey {
    Token = 0,
    TokenShare = 1,
    Admin = 2,
    StartTime = 3,
    EndTime = 4,
    TotalShares = 5,
    Reserve = 6,
    TotalReserve = 7,
    CurrentQuote = 8,
    QuoteExpiration = 9,
    QuotePeriod = 10,
    Treasury = 11,
}

impl TryFromVal<Env, DataKey> for Val {
    type Error = ConversionError;

    fn try_from_val(_env: &Env, v: &DataKey) -> Result<Self, Self::Error> {
        Ok((*v as u32).into())
    }
}

fn get_token(e: &Env) -> Address {
    e.storage().instance().get(&DataKey::Token).unwrap()
}

fn get_token_share(e: &Env) -> Address {
    e.storage().instance().get(&DataKey::TokenShare).unwrap()
}

fn get_admin(e: &Env) -> Address {
    e.storage().instance().get(&DataKey::Admin).unwrap()
}

fn get_start_time(e: &Env) -> u64 {
    e.storage().instance().get(&DataKey::StartTime).unwrap()
}

fn get_end_time(e: &Env) -> u64 {
    e.storage().instance().get(&DataKey::EndTime).unwrap()
}

fn get_total_shares(e: &Env) -> i128 {
    e.storage().instance().get(&DataKey::TotalShares).unwrap()
}

fn get_reserve(e: &Env) -> i128 {
    e.storage().instance().get(&DataKey::Reserve).unwrap()
}

fn get_total_reserve(e: &Env) -> i128 {
    e.storage().instance().get(&DataKey::TotalReserve).unwrap()
}

fn get_current_quote(e: &Env) -> i128 {
    let current_quote = e.storage().instance().get(&DataKey::CurrentQuote).unwrap();
    let quote_expiration = e.storage()
        .instance()
        .get(&DataKey::QuoteExpiration)
        .unwrap();

    // Check they are non-zero
    if current_quote != 0 && quote_expiration != 0 {
        if time(&e) <= quote_expiration {
            current_quote
        } else {
            0
        }
    } else {
        0
    }
}

fn get_quote_period(e: &Env) -> u64 {
    e.storage().instance().get(&DataKey::QuotePeriod).unwrap()
}

fn get_treasury(e: &Env) -> Address {
    e.storage().instance().get(&DataKey::Treasury).unwrap()
}

fn time(e: &Env) -> u64 {
    e.ledger().timestamp()
}

fn put_token(e: &Env, contract: Address) {
    e.storage().instance().set(&DataKey::Token, &contract);
}

fn put_token_share(e: &Env, contract: Address) {
    e.storage().instance().set(&DataKey::TokenShare, &contract);
}

fn put_admin(e: &Env, admin: Address) {
    e.storage().instance().set(&DataKey::Admin, &admin)
}

fn put_start_time(e: &Env, time: u64) {
    e.storage().instance().set(&DataKey::StartTime, &time)
}

fn put_end_time(e: &Env, time: u64) {
    e.storage().instance().set(&DataKey::EndTime, &time)
}

fn put_current_quote(e: &Env, amount: i128) {
    e.storage().instance().set(&DataKey::CurrentQuote, &amount)
}

fn put_quote_expiration(e: &Env) {
    let time = time(e) + get_quote_period(e);
    e.storage().instance().set(&DataKey::QuoteExpiration, &time)
}

fn put_quote_period(e: &Env, period: u64) {
    e.storage().instance().set(&DataKey::QuotePeriod, &period)
}

fn put_total_shares(e: &Env, amount: i128) {
    e.storage().instance().set(&DataKey::TotalShares, &amount)
}

fn put_reserve(e: &Env, amount: i128) {
    e.storage().instance().set(&DataKey::Reserve, &amount)
}

fn put_total_reserve(e: &Env, amount: i128) {
    e.storage().instance().set(&DataKey::TotalReserve, &amount)
}

fn put_treasury(e: &Env, treasury: Address) {
    e.storage().instance().set(&DataKey::Treasury, &treasury)
}

fn burn_shares(e: &Env, amount: i128) {
    let total = get_total_shares(e);
    let share_contract_id = get_token_share(e);

    token::Client::new(e, &share_contract_id).burn(&e.current_contract_address(), &amount);
    put_total_shares(e, total - amount);
}

fn mint_shares(e: &Env, to: Address, amount: i128) {
    let total = get_total_shares(e);
    let share_contract_id = get_token_share(e);

    token::Client::new(e, &share_contract_id).mint(&to, &amount);

    put_total_shares(e, total + amount);
}

fn check_nonnegative_amount(amount: i128) {
    if amount < 0 {
        panic!("negative amount is not allowed: {}", amount)
    }
}

pub trait VaultTrait {
    // Sets the token contract addresses for this vault
    fn initialize(
        e: Env,
        token_wasm_hash: BytesN<32>,
        token: Address,
        admin: Address,
        start_time: u64,
        end_time: u64,
        quote_period: u64,
        treasury: Address,
    );

    // Returns the token contract address for the vault share token
    fn bond_id(e: Env) -> Address;

    // Deposits token. Also mints vault shares for the `from` Identifier. The amount minted
    // is determined based on the difference between the reserves stored by this contract, and
    // the actual balance of token for this contract.
    fn deposit(e: Env, from: Address, amount: i128) -> i128;

    // transfers `amount` of vault share tokens to this contract, burns all pools share tokens in this contracts, and sends the
    // corresponding amount of token to `to`.
    // Returns amount of token withdrawn
    fn withdraw(e: Env, to: Address, amount: i128) -> i128;

    fn reserves(e: Env) -> i128;

    fn admin(e: Env) -> Address;

    fn maturity(e: Env) -> u64;

    fn total_bonds(e: Env) -> i128;

    fn treasury_account(e: Env) -> Address;

    fn quote(e: Env) -> i128;

    fn set_quote(e: Env, amount: i128);

    fn set_total_reserve(e: Env, amount: i128);

    fn set_treasury(e: Env, treasury: Address);

    fn new_owner(e: Env) -> Address;
}

#[contract]
struct Vault;

#[contractimpl]
impl VaultTrait for Vault {
    fn initialize(
        e: Env,
        token_wasm_hash: BytesN<32>,
        token: Address,
        admin: Address,
        start_time: u64,
        end_time: u64,
        quote_period: u64,
        treasury: Address,
    ) {
        if get_start_time(&e) > 0 {
            panic!("already initialized")
        }
        let share_contract_id = create_contract(&e, token_wasm_hash, &token);
        token::Client::new(&e, &share_contract_id).initialize(
            &e.current_contract_address(),
            &7u32,
            &"Vault Share Token".into_val(&e),
            &"VST".into_val(&e),
        );

        put_token(&e, token);
        put_token_share(&e, share_contract_id.try_into().unwrap());
        put_admin(&e, admin);
        put_start_time(&e, start_time);
        put_end_time(&e, end_time);
        put_total_shares(&e, 0);
        put_reserve(&e, 0);
        put_total_reserve(&e, 0);
        put_current_quote(&e, 0);
        put_quote_period(&e, quote_period);
        put_treasury(&e, treasury);
    }

    fn quote(e: Env) -> i128 {
        get_current_quote(&e)
    }

    fn set_quote(e: Env, amount: i128) {
        let admin = get_admin(&e);
        admin.require_auth();

        check_nonnegative_amount(amount);
        put_current_quote(&e, amount);
        put_quote_expiration(&e);
    }

    fn bond_id(e: Env) -> Address {
        get_token_share(&e)
    }

    fn deposit(e: Env, from: Address, amount: i128) -> i128 {
        // Depositor needs to authorize the deposit
        from.require_auth();

        check_nonnegative_amount(amount);

        if time(&e) > get_end_time(&e) {
            panic!("maturity reached")
        }

        if time(&e) < get_start_time(&e) {
            panic!("not open yet")
        }

        let quote = get_current_quote(&e);
        if quote == 0 {
            panic!("request a new quote")
        }
        
        let quantity = amount * quote;

        let token_client = token::Client::new(&e, &get_token(&e));
        token_client.transfer(&from, &get_treasury(&e), &amount);

        mint_shares(&e, from, quantity);
        put_reserve(&e, get_reserve(&e) + amount);

        quantity
    }

    fn withdraw(e: Env, to: Address, amount: i128) -> i128 {
        to.require_auth();

        check_nonnegative_amount(amount);

        if time(&e) < get_end_time(&e) {
            panic!("maturity not reached")
        }

        let total_reserve = get_total_reserve(&e);
        if total_reserve == 0 {
            panic!("total reserve not set")
        }

        // First transfer the vault shares that need to be redeemed
        let share_token_client = token::Client::new(&e, &get_token_share(&e));
        share_token_client.transfer(&to, &e.current_contract_address(), &amount);

        // Calculate total amount including yield
        let asset_amount = total_reserve / get_total_shares(&e) * amount;

        let token_client = token::Client::new(&e, &get_token(&e));
        token_client.transfer(&e.current_contract_address(), &to, &asset_amount);

        burn_shares(&e, amount); // Only burn the original amount of shares
        put_total_reserve(&e, total_reserve - asset_amount);

        asset_amount
    }

    fn reserves(e: Env) -> i128 {
        get_reserve(&e)
    }

    fn set_total_reserve(e: Env, amount: i128) {
        check_nonnegative_amount(amount);
        
        if time(&e) < get_end_time(&e) {
            panic!("maturity not reached")
        }
        if get_total_reserve(&e) > 0 {
            panic!("already set")
        }
        let admin = get_admin(&e);
        admin.require_auth();

        let token_client = token::Client::new(&e, &get_token(&e));
        token_client.transfer(&admin, &e.current_contract_address(), &amount);

        put_total_reserve(&e, amount);
    }

    fn set_treasury(e: Env, treasury: Address) {
        let admin = get_admin(&e);
        admin.require_auth();
        put_treasury(&e, treasury);
    }

    fn admin(e: Env) -> Address {
        get_admin(&e)
    }

    fn new_owner(e: Env) -> Address {
        let admin = get_admin(&e);
        admin.require_auth();
        e.current_contract_address()
    }

    fn maturity(e: Env) -> u64 {
        get_end_time(&e)
    }

    fn total_bonds(e: Env) -> i128 {
        get_total_shares(&e)
    }

    fn treasury_account(e: Env) -> Address {
        get_treasury(&e)
    }
}
