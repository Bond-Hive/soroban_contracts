#![no_std]

mod token;

use soroban_sdk::{
    contract, contracterror, contractimpl, symbol_short, Address, BytesN, ConversionError, Env,
    IntoVal, TryFromVal, Val,
};

use token::create_contract;

pub(crate) const DAY_IN_LEDGERS: u32 = 17280;
pub(crate) const MAX_TTL: u32 = 3110400;
pub(crate) const DECIMALS: u32 = 7;

#[derive(Clone, Copy)]
#[repr(u32)]
pub enum DataKey {
    Token = 0,
    TokenShare = 1,
    Admin = 2,
    StartTime = 3,
    EndTime = 4,
    TotalShares = 5,
    TotalDeposit = 6,
    AvailableRedemption = 7,
    CurrentQuote = 8,
    QuoteExpiration = 9,
    QuotePeriod = 10,
    Treasury = 11,
    MinDeposit = 12,
}

impl TryFromVal<Env, DataKey> for Val {
    type Error = ConversionError;

    fn try_from_val(_env: &Env, v: &DataKey) -> Result<Self, Self::Error> {
        Ok((*v as u32).into())
    }
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum VaultError {
    InvalidAmount = 1,
    NotInitialized = 2,
    AlreadyInitialized = 3,
    MaturityReached = 4,
    MaturityNotReached = 5,
    NotOpenYet = 6,
    QuoteRequired = 7,
    AvailableRedemptionNotSet = 8,
    AvailableRedemptionAlreadySet = 9,
}

fn get_token(e: &Env) -> Result<Address, VaultError> {
    e.storage()
        .instance()
        .get(&DataKey::Token)
        .ok_or(VaultError::NotInitialized)
}

fn get_token_share(e: &Env) -> Result<Address, VaultError> {
    e.storage()
        .instance()
        .get(&DataKey::TokenShare)
        .ok_or(VaultError::NotInitialized)
}

fn get_admin(e: &Env) -> Result<Address, VaultError> {
    e.storage()
        .instance()
        .get(&DataKey::Admin)
        .ok_or(VaultError::NotInitialized)
}

fn get_start_time(e: &Env) -> Result<u64, VaultError> {
    e.storage()
        .instance()
        .get(&DataKey::StartTime)
        .ok_or(VaultError::NotInitialized)
}

fn get_end_time(e: &Env) -> Result<u64, VaultError> {
    e.storage()
        .instance()
        .get(&DataKey::EndTime)
        .ok_or(VaultError::NotInitialized)
}

fn get_total_shares(e: &Env) -> Result<i128, VaultError> {
    e.storage()
        .instance()
        .get(&DataKey::TotalShares)
        .ok_or(VaultError::NotInitialized)
}

fn get_total_deposit(e: &Env) -> Result<i128, VaultError> {
    e.storage()
        .instance()
        .get(&DataKey::TotalDeposit)
        .ok_or(VaultError::NotInitialized)
}

fn get_available_redemption(e: &Env) -> Result<i128, VaultError> {
    e.storage()
        .instance()
        .get(&DataKey::AvailableRedemption)
        .ok_or(VaultError::NotInitialized)
}

fn get_min_deposit(e: &Env) -> Result<u128, VaultError> {
    e.storage()
        .instance()
        .get(&DataKey::MinDeposit)
        .ok_or(VaultError::NotInitialized)
}

fn get_current_quote(e: &Env) -> Result<i128, VaultError> {
    let current_quote = e
        .storage()
        .instance()
        .get(&DataKey::CurrentQuote)
        .ok_or(VaultError::NotInitialized)?;
    let quote_expiration = e
        .storage()
        .instance()
        .get(&DataKey::QuoteExpiration)
        .ok_or(VaultError::NotInitialized)?;

    // Check they are non-zero
    if current_quote != 0 && quote_expiration != 0 {
        if time(&e) <= quote_expiration {
            Ok(current_quote)
        } else {
            return Err(VaultError::QuoteRequired);
        }
    } else {
        return Err(VaultError::QuoteRequired);
    }
}

fn get_quote_period(e: &Env) -> Result<u64, VaultError> {
    e.storage()
        .instance()
        .get(&DataKey::QuotePeriod)
        .ok_or(VaultError::NotInitialized)
}

fn get_treasury(e: &Env) -> Result<Address, VaultError> {
    e.storage()
        .instance()
        .get(&DataKey::Treasury)
        .ok_or(VaultError::NotInitialized)
}

fn time(e: &Env) -> u64 {
    e.ledger().timestamp()
}

fn extend_instance_ttl(e: &Env) {
    e.storage()
        .instance()
        .extend_ttl(MAX_TTL - DAY_IN_LEDGERS, MAX_TTL)
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

fn put_quote_expiration(e: &Env) -> Result<(), VaultError> {
    let time = time(e) + get_quote_period(e)?;
    e.storage().instance().set(&DataKey::QuoteExpiration, &time);
    Ok(())
}

fn put_quote_period(e: &Env, period: u64) {
    e.storage().instance().set(&DataKey::QuotePeriod, &period)
}

fn put_total_shares(e: &Env, amount: i128) {
    e.storage().instance().set(&DataKey::TotalShares, &amount)
}

fn put_total_deposit(e: &Env, amount: i128) {
    e.storage().instance().set(&DataKey::TotalDeposit, &amount)
}

fn put_available_redemption(e: &Env, amount: i128) {
    e.storage().instance().set(&DataKey::AvailableRedemption, &amount)
}

fn put_treasury(e: &Env, treasury: Address) {
    e.storage().instance().set(&DataKey::Treasury, &treasury)
}

fn put_min_deposit(e: &Env, amount: u128) {
    e.storage().instance().set(&DataKey::MinDeposit, &amount)
}

fn burn_shares(e: &Env, amount: i128) -> Result<(), VaultError> {
    let total = get_total_shares(e)?;
    let share_contract_id = get_token_share(e)?;

    token::Client::new(e, &share_contract_id).burn(&e.current_contract_address(), &amount);
    put_total_shares(e, total - amount);

    e.events()
        .publish((symbol_short!("SHARES"), symbol_short!("burned")), amount);

    Ok(())
}

fn mint_shares(e: &Env, to: Address, amount: i128) -> Result<(), VaultError> {
    let total = get_total_shares(e)?;
    let share_contract_id = get_token_share(e)?;

    token::Client::new(e, &share_contract_id).mint(&to, &amount);

    put_total_shares(e, total + amount);

    e.events().publish(
        (symbol_short!("SHARES"), symbol_short!("minted")),
        (to, amount),
    );

    Ok(())
}

fn check_nonnegative_amount(amount: i128) -> Result<(), VaultError> {
    if amount < 0 {
        Err(VaultError::InvalidAmount)
    } else {
        Ok(())
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
        min_deposit: u128,
    );

    // Returns the token contract address for the vault share token
    fn bond_id(e: Env) -> Result<Address, VaultError>;

    // Deposits token. Also mints vault shares for the `from` Identifier. The amount minted
    // is determined based on the difference between the reserves stored by this contract, and
    // the actual balance of token for this contract.
    fn deposit(e: Env, from: Address, amount: i128) -> Result<i128, VaultError>;

    // transfers `amount` of vault share tokens to this contract, burns all pools share tokens in this contracts, and sends the
    // corresponding amount of token to `to`.
    // Returns amount of token withdrawn
    fn withdraw(e: Env, to: Address, amount: i128) -> Result<i128, VaultError>;

    fn total_deposit(e: Env) -> Result<i128, VaultError>;

    fn available_redemption(e: Env) -> Result<i128, VaultError>;

    fn admin(e: Env) -> Result<Address, VaultError>;

    fn maturity(e: Env) -> Result<u64, VaultError>;

    fn total_bonds(e: Env) -> Result<i128, VaultError>;

    fn treasury_account(e: Env) -> Result<Address, VaultError>;

    fn quote(e: Env) -> Result<i128, VaultError>;

    fn set_quote(e: Env, amount: i128) -> Result<(), VaultError>;

    fn set_total_redemption(e: Env, amount: i128) -> Result<(), VaultError>;

    fn set_treasury(e: Env, treasury: Address) -> Result<(), VaultError>;

    fn set_admin(e: Env, new_admin: Address) -> Result<(), VaultError>;
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
        min_deposit: u128,
    ) {
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
        put_total_deposit(&e, 0);
        put_available_redemption(&e, 0);
        put_current_quote(&e, 0);
        put_quote_period(&e, quote_period);
        put_treasury(&e, treasury);
        put_min_deposit(&e, min_deposit);

        e.events().publish(
            (symbol_short!("VAULT"), symbol_short!("init")),
            (e.current_contract_address(), start_time, end_time),
        );
    }

    fn quote(e: Env) -> Result<i128, VaultError> {
        extend_instance_ttl(&e);
        match get_current_quote(&e) {
            Ok(quote) => Ok(quote),
            Err(_) => Ok(0),
        }
    }

    fn set_quote(e: Env, amount: i128) -> Result<(), VaultError> {
        let admin = get_admin(&e)?;
        admin.require_auth();

        check_nonnegative_amount(amount)?;
        extend_instance_ttl(&e);
        put_current_quote(&e, amount);
        put_quote_expiration(&e)?;

        e.events()
            .publish((symbol_short!("QUOTE"), symbol_short!("set")), amount);

        Ok(())
    }

    fn bond_id(e: Env) -> Result<Address, VaultError> {
        extend_instance_ttl(&e);
        get_token_share(&e)
    }

    fn deposit(e: Env, from: Address, amount: i128) -> Result<i128, VaultError> {
        // Depositor needs to authorize the deposit
        from.require_auth();

        check_nonnegative_amount(amount)?;
        extend_instance_ttl(&e);

        if time(&e) > get_end_time(&e)? {
            return Err(VaultError::MaturityReached);
        }

        if time(&e) < get_start_time(&e)? {
            return Err(VaultError::NotOpenYet);
        }
        if amount < get_min_deposit(&e)? as i128 {
            return Err(VaultError::InvalidAmount);
        }

        let quote = get_current_quote(&e)?;
        let quantity = amount * quote / 10i128.pow(DECIMALS);
        let token_client = token::Client::new(&e, &get_token(&e)?);
        token_client.transfer(&from, &get_treasury(&e)?, &amount);

        mint_shares(&e, from, quantity)?;
        put_total_deposit(&e, get_total_deposit(&e)? + amount);

        Ok(quantity)
    }

    fn withdraw(e: Env, to: Address, amount: i128) -> Result<i128, VaultError> {
        to.require_auth();

        check_nonnegative_amount(amount)?;
        extend_instance_ttl(&e);

        if time(&e) < get_end_time(&e)? {
            return Err(VaultError::MaturityNotReached);
        }

        let available_redemption = get_available_redemption(&e)?;
        if available_redemption == 0 {
            return Err(VaultError::AvailableRedemptionNotSet);
        }

        // First transfer the vault shares that need to be redeemed
        let share_token_client = token::Client::new(&e, &get_token_share(&e)?);
        share_token_client.transfer(&to, &e.current_contract_address(), &amount);

        // Calculate total amount including yield
        let asset_amount = available_redemption * amount / get_total_shares(&e)?;

        let token_client = token::Client::new(&e, &get_token(&e)?);
        token_client.transfer(&e.current_contract_address(), &to, &asset_amount);

        burn_shares(&e, amount)?; // Only burn the original amount of shares
        put_available_redemption(&e, available_redemption - asset_amount);

        Ok(asset_amount)
    }

    fn total_deposit(e: Env) -> Result<i128, VaultError> {
        extend_instance_ttl(&e);
        get_total_deposit(&e)
    }

    fn available_redemption(e: Env) -> Result<i128, VaultError> {
        extend_instance_ttl(&e);
        get_available_redemption(&e)
    }

    fn set_total_redemption(e: Env, amount: i128) -> Result<(), VaultError> {
        check_nonnegative_amount(amount)?;
        extend_instance_ttl(&e);

        if time(&e) < get_end_time(&e)? {
            return Err(VaultError::MaturityNotReached);
        }
        if get_available_redemption(&e)? > 0 {
            return Err(VaultError::AvailableRedemptionAlreadySet);
        }
        let admin = get_admin(&e)?;
        admin.require_auth();

        let token_client = token::Client::new(&e, &get_token(&e)?);
        token_client.transfer(&admin, &e.current_contract_address(), &amount);

        put_available_redemption(&e, amount);
        Ok(())
    }

    fn set_treasury(e: Env, treasury: Address) -> Result<(), VaultError> {
        let admin = get_admin(&e)?;
        admin.require_auth();
        extend_instance_ttl(&e);
        e.events().publish(
            (symbol_short!("TREASURY"), symbol_short!("set")),
            treasury.clone(),
        );
        put_treasury(&e, treasury);

        Ok(())
    }

    fn admin(e: Env) -> Result<Address, VaultError> {
        extend_instance_ttl(&e);
        get_admin(&e)
    }

    fn set_admin(e: Env, new_admin: Address) -> Result<(), VaultError> {
        let admin = get_admin(&e)?;
        admin.require_auth();
        extend_instance_ttl(&e);
        e.events().publish(
            (symbol_short!("ADMIN"), symbol_short!("changed")),
            new_admin.clone(),
        );
        put_admin(&e, new_admin);

        Ok(())
    }

    fn maturity(e: Env) -> Result<u64, VaultError> {
        extend_instance_ttl(&e);
        get_end_time(&e)
    }

    fn total_bonds(e: Env) -> Result<i128, VaultError> {
        extend_instance_ttl(&e);
        get_total_shares(&e)
    }

    fn treasury_account(e: Env) -> Result<Address, VaultError> {
        extend_instance_ttl(&e);
        get_treasury(&e)
    }
}