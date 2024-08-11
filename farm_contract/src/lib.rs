#![no_std]

mod token;

use soroban_sdk::{
    contract, contracterror, contractimpl, symbol_short, Address, BytesN, ConversionError, Env,
    IntoVal, TryFromVal, TryIntoVal, Val, Map,
};

use token::create_contract;

pub(crate) const DAY_IN_LEDGERS: u32 = 17280;
pub(crate) const MAX_TTL: u32 = 3110400;
pub(crate) const DECIMALS: u32 = 7;

#[derive(Clone, Copy)]
#[repr(u32)]
pub enum DataKey {
    Pools = 0,
    Admin = 1,
    AllocatedRewards1 = 2,
    AllocatedRewards2 = 3,
    RewardedToken1 = 4,
    RewardedToken2 = 5,
    PoolCounter = 6,
    TokenShare = 7,
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
pub enum FarmError {
    InvalidAmount = 1,
    NotInitialized = 2,
    NotAuthorized = 3,
    PoolNotFound = 4,
    WithdrawError = 5,
    InsufficientRewards = 6,
    ConversionError = 7,
    PoolNotActive = 8,
}

#[derive(Clone)]
pub struct Pool {
    pub token: Address,
    pub start_time: u64,
    pub expiration_date: u64,
    pub reward_ratio1: i128,
    pub reward_ratio2: i128,
    pub pool_id: u32,
}

#[derive(Clone)]
pub struct UserData {
    pub deposited: i128,
    pub deposit_time: u64,
    pub accrued_rewards1: i128,
    pub accrued_rewards2: i128,
}

#[derive(Clone)]
pub struct FarmState {
    pub allocated_rewards1: i128,
    pub allocated_rewards2: i128,
    pub pools: Map<u32, Pool>,
    pub user_data: Map<(Address, u32), UserData>,
}

impl TryFromVal<Env, Val> for Pool {
    type Error = ConversionError;

    fn try_from_val(env: &Env, val: &Val) -> Result<Self, Self::Error> {
        let data: (Address, u64, u64, i128, i128, u32) = val.clone().try_into_val(env)?;
        Ok(Pool {
            token: data.0,
            start_time: data.1,
            expiration_date: data.2,
            reward_ratio1: data.3,
            reward_ratio2: data.4,
            pool_id: data.5,
        })
    }
}

impl IntoVal<Env, Val> for Pool {
    fn into_val(&self, env: &Env) -> Val {
        (
            self.token.clone(),
            self.start_time,
            self.expiration_date,
            self.reward_ratio1,
            self.reward_ratio2,
            self.pool_id,
        )
        .into_val(env)
    }
}

impl TryFromVal<Env, Val> for UserData {
    type Error = ConversionError;

    fn try_from_val(env: &Env, val: &Val) -> Result<Self, Self::Error> {
        let data: (i128, u64, i128, i128) = val.clone().try_into_val(env)?;
        Ok(UserData {
            deposited: data.0,
            deposit_time: data.1,
            accrued_rewards1: data.2,
            accrued_rewards2: data.3,
        })
    }
}

impl IntoVal<Env, Val> for UserData {
    fn into_val(&self, env: &Env) -> Val {
        (
            self.deposited,
            self.deposit_time,
            self.accrued_rewards1,
            self.accrued_rewards2,
        )
        .into_val(env)
    }
}

#[contract]
struct Farm;

#[contractimpl]
impl Farm {
    pub fn initialize(
        e: Env,
        admin: Address,
        rewarded_token1: Address,
        rewarded_token2: Address,
        token_wasm_hash: BytesN<32>,
    ) -> Result<(), FarmError> {
        // Create the receipt token contract and initialize it
        let receipt_token_id = create_contract(&e, token_wasm_hash, &e.current_contract_address());
        token::Client::new(&e, &receipt_token_id).initialize(
            &e.current_contract_address(),
            &DECIMALS,
            &"bondHive".into_val(&e),
            &"BHFARM".into_val(&e),
        );
    
        // Store the admin, receipt token, and rewarded tokens in the contract's storage
        put_admin(&e, &admin);  // Pass `admin` as a reference
        put_token_share(&e, receipt_token_id);
        put_rewarded_tokens(&e, rewarded_token1, rewarded_token2);
        put_pool_counter(&e, 0); // Initialize the pool counter
        put_farm_state(
            &e,
            FarmState {
                allocated_rewards1: 0,
                allocated_rewards2: 0,
                pools: Map::new(&e),
                user_data: Map::new(&e),
            },
        );
    
        Ok(())
    }
    
    pub fn create_pool(
        e: Env,
        token: Address,
        start_time: u64,
        expiration_date: u64,
        reward_ratio1: i128,
        reward_ratio2: i128,
    ) -> Result<u32, FarmError> {
        let admin = get_admin(&e)?;
        admin.require_auth();
        extend_instance_ttl(&e);

        let mut counter = get_pool_counter(&e)?;
        let mut state = get_farm_state(&e)?;

        let pool = Pool {
            token: token.clone(),
            start_time,
            expiration_date,
            reward_ratio1,
            reward_ratio2,
            pool_id: counter,
        };

        state.pools.set(counter, pool);
        put_farm_state(&e, state);

        counter += 1;
        put_pool_counter(&e, counter);

        e.events().publish(
            (symbol_short!("NewPool"), counter),
            (token, start_time, expiration_date, reward_ratio1, reward_ratio2),
        );

        Ok(counter - 1)
    }

    pub fn deposit(
        e: Env,
        depositor: Address,
        amount: i128,
        pool_id: u32,
    ) -> Result<(), FarmError> {
        depositor.require_auth();
        extend_instance_ttl(&e);

        check_nonnegative_amount(amount)?;
        check_nonzero_amount(amount)?;

        let mut state = get_farm_state(&e)?;
        let pool = state.pools.get(pool_id).ok_or(FarmError::PoolNotFound)?;
        let current_time = time(&e);

        if current_time < pool.start_time {
            return Err(FarmError::PoolNotActive);
        }

        // Get existing user data or initialize it
        let mut user_data = state
            .user_data
            .get((depositor.clone(), pool_id))
            .unwrap_or(UserData {
                deposited: 0,
                deposit_time: current_time,
                accrued_rewards1: 0,
                accrued_rewards2: 0,
            });

        let time_elapsed = core::cmp::min(
            current_time - user_data.deposit_time,
            pool.expiration_date - user_data.deposit_time,
        );
        let time_to_maturity = pool.expiration_date - current_time;

        let accrued_yield1 = if pool.reward_ratio1 > 0 {
            (user_data.deposited * pool.reward_ratio1 * time_elapsed as i128)
                / 10i128.pow(DECIMALS)
        } else {
            0
        };
        let accrued_yield2 = if pool.reward_ratio2 > 0 {
            (user_data.deposited * pool.reward_ratio2 * time_elapsed as i128)
                / 10i128.pow(DECIMALS)
        } else {
            0
        };

        // Update the user's accrued rewards
        user_data.accrued_rewards1 += accrued_yield1;
        user_data.accrued_rewards2 += accrued_yield2;

        // Add the new deposit to the existing deposit amount
        user_data.deposited += amount;
        user_data.deposit_time = current_time; // Reset deposit time to the time of the new deposit

        token::Client::new(&e, &pool.token).transfer(&depositor, &e.current_contract_address(), &amount);
        state.user_data.set((depositor.clone(), pool_id), user_data);

        mint_receipt_tokens(&e, &depositor, amount)?;

        // Allocate the new potential yield based on the new total deposit
        let potential_yield1 = if pool.reward_ratio1 > 0 {
            (amount * pool.reward_ratio1 * time_to_maturity as i128) / 10i128.pow(DECIMALS)
        } else {
            0
        };
        let potential_yield2 = if pool.reward_ratio2 > 0 {
            (amount * pool.reward_ratio2 * time_to_maturity as i128) / 10i128.pow(DECIMALS)
        } else {
            0
        };

        // Check if there is enough balance in the contract to cover these new yields
        if !self::has_sufficient_rewards(
            &e,
            &(state.allocated_rewards1 + potential_yield1),
            &(state.allocated_rewards2 + potential_yield2),
        )? {
            return Err(FarmError::InsufficientRewards);
        }

        // Allocate the new rewards
        state.allocated_rewards1 += potential_yield1;
        state.allocated_rewards2 += potential_yield2;
        put_farm_state(&e, state);

        e.events().publish(
            (symbol_short!("Deposit"), pool_id, depositor.clone()),
            amount,
        );

        Ok(())
    }

    pub fn withdraw(
        e: Env,
        withdrawer: Address,
        pool_id: u32,
        amount: i128,
    ) -> Result<(), FarmError> {
        withdrawer.require_auth();
        extend_instance_ttl(&e);

        check_nonnegative_amount(amount)?;

        let mut state = get_farm_state(&e)?;
        let current_time = time(&e);

        let pool = state.pools.get(pool_id).ok_or(FarmError::PoolNotFound)?;
        let mut user_data = state
            .user_data
            .get((withdrawer.clone(), pool_id))
            .ok_or(FarmError::WithdrawError)?;

        if amount > user_data.deposited {
            return Err(FarmError::InvalidAmount);
        }

        let time_elapsed = core::cmp::min(
            current_time - user_data.deposit_time,
            pool.expiration_date - user_data.deposit_time,
        );
        let total_yield1 = if pool.reward_ratio1 > 0 {
            (user_data.deposited * pool.reward_ratio1 * time_elapsed as i128)
                / 10i128.pow(DECIMALS)
        } else {
            0
        };
        let total_yield2 = if pool.reward_ratio2 > 0 {
            (user_data.deposited * pool.reward_ratio2 * time_elapsed as i128)
                / 10i128.pow(DECIMALS)
        } else {
            0
        };

        // Burn receipt tokens corresponding to the withdrawn amount
        if amount > 0 {
            burn_receipt_tokens(&e, &withdrawer, amount)?;
            token::Client::new(&e, &pool.token)
                .transfer(&e.current_contract_address(), &withdrawer, &amount);
        }

        if user_data.accrued_rewards1 + total_yield1 > 0 {
            token::Client::new(&e, &get_rewarded_token1(&e)?).transfer(
                &e.current_contract_address(),
                &withdrawer,
                &(user_data.accrued_rewards1 + total_yield1),
            );
        }

        if user_data.accrued_rewards2 + total_yield2 > 0 {
            token::Client::new(&e, &get_rewarded_token2(&e)?).transfer(
                &e.current_contract_address(),
                &withdrawer,
                &(user_data.accrued_rewards2 + total_yield2),
            );
        }

        // Adjust allocated rewards if the user withdraws early
        let time_to_maturity = pool.expiration_date - current_time;
        let full_yield1 = if pool.reward_ratio1 > 0 {
            (amount * pool.reward_ratio1 * time_to_maturity as i128) / 10i128.pow(DECIMALS)
        } else {
            0
        };
        let full_yield2 = if pool.reward_ratio2 > 0 {
            (amount * pool.reward_ratio2 * time_to_maturity as i128) / 10i128.pow(DECIMALS)
        } else {
            0
        };

        state.allocated_rewards1 -= full_yield1;
        state.allocated_rewards2 -= full_yield2;

        // Update the user's deposited balance and accrued rewards
        user_data.deposited -= amount;
        user_data.accrued_rewards1 = 0;
        user_data.accrued_rewards2 = 0;
        user_data.deposit_time = current_time;

        if user_data.deposited > 0 {
            state.user_data.set((withdrawer.clone(), pool_id), user_data);
        } else {
            state.user_data.remove((withdrawer.clone(), pool_id));
        }

        put_farm_state(&e, state);

        e.events().publish(
            (symbol_short!("Withdraw"), pool_id, withdrawer.clone()),
            amount,
        );

        Ok(())
    }

    pub fn set_admin(e: Env, new_admin: Address) -> Result<(), FarmError> {
        let admin = get_admin(&e)?;
        admin.require_auth();
        extend_instance_ttl(&e);
    
        put_admin(&e, &new_admin);
    
        e.events().publish(
            (symbol_short!("AdminChg"), new_admin.clone()),
            new_admin,
        );
    
        Ok(())
    }

    pub fn get_receipt_token_id(e: Env) -> Result<Address, FarmError> {
        extend_instance_ttl(&e);
        get_receipt_token_id_internal(&e)
    }
}

fn has_sufficient_rewards(e: &Env, required1: &i128, required2: &i128) -> Result<bool, FarmError> {
    let rewarded_token1 = get_rewarded_token1(e)?;
    let rewarded_token2 = get_rewarded_token2(e)?;

    let available1 = token::Client::new(e, &rewarded_token1).balance(&e.current_contract_address());
    let available2 = token::Client::new(e, &rewarded_token2).balance(&e.current_contract_address());

    Ok(available1 >= *required1 && available2 >= *required2)
}

fn put_admin(e: &Env, admin: &Address) {
    e.storage().instance().set(&DataKey::Admin, admin);
}

fn get_admin(e: &Env) -> Result<Address, FarmError> {
    e.storage()
        .instance()
        .get(&DataKey::Admin)
        .ok_or(FarmError::NotInitialized)
}

fn put_rewarded_tokens(e: &Env, token1: Address, token2: Address) {
    e.storage().instance().set(&DataKey::RewardedToken1, &token1);
    e.storage().instance().set(&DataKey::RewardedToken2, &token2);
}

fn get_rewarded_token1(e: &Env) -> Result<Address, FarmError> {
    e.storage()
        .instance()
        .get(&DataKey::RewardedToken1)
        .ok_or(FarmError::NotInitialized)
}

fn get_rewarded_token2(e: &Env) -> Result<Address, FarmError> {
    e.storage()
        .instance()
        .get(&DataKey::RewardedToken2)
        .ok_or(FarmError::NotInitialized)
}

fn put_token_share(e: &Env, token_share: Address) {
    e.storage().instance().set(&DataKey::TokenShare, &token_share);
}

fn get_receipt_token_id_internal(e: &Env) -> Result<Address, FarmError> {
    e.storage()
        .instance()
        .get(&DataKey::TokenShare)
        .ok_or(FarmError::NotInitialized)
}

fn put_pool_counter(e: &Env, counter: u32) {
    e.storage().instance().set(&DataKey::PoolCounter, &counter);
}

fn get_pool_counter(e: &Env) -> Result<u32, FarmError> {
    e.storage()
        .instance()
        .get(&DataKey::PoolCounter)
        .ok_or(FarmError::NotInitialized)
}

fn put_farm_state(e: &Env, state: FarmState) {
    e.storage().instance().set(&DataKey::AllocatedRewards1, &state.allocated_rewards1);
    e.storage().instance().set(&DataKey::AllocatedRewards2, &state.allocated_rewards2);
    e.storage().instance().set(&DataKey::Pools, &state.pools);
    e.storage().instance().set(&DataKey::TokenShare, &state.user_data);
}

fn get_farm_state(e: &Env) -> Result<FarmState, FarmError> {
    let allocated_rewards1: i128 = e
        .storage()
        .instance()
        .get(&DataKey::AllocatedRewards1)
        .unwrap_or(Ok(0))?;
    let allocated_rewards2: i128 = e
        .storage()
        .instance()
        .get(&DataKey::AllocatedRewards2)
        .unwrap_or(Ok(0))?;
    let pools: Map<u32, Pool> = e
        .storage()
        .instance()
        .get(&DataKey::Pools)
        .unwrap_or(Ok(Map::new(&e)))?;
    let user_data: Map<(Address, u32), UserData> = e
        .storage()
        .instance()
        .get(&DataKey::TokenShare)
        .unwrap_or(Ok(Map::new(&e)))?;

    Ok(FarmState {
        allocated_rewards1: allocated_rewards1,
        allocated_rewards2: allocated_rewards2,
        pools: pools,
        user_data: user_data,
    })
}

fn mint_receipt_tokens(e: &Env, to: &Address, amount: i128) -> Result<(), FarmError> {
    let receipt_token_id = get_receipt_token_id_internal(&e)?;
    token::Client::new(&e, &receipt_token_id).mint(to, &amount);
    Ok(())
}

fn burn_receipt_tokens(e: &Env, from: &Address, amount: i128) -> Result<(), FarmError> {
    let receipt_token_id = get_receipt_token_id_internal(&e)?;
    token::Client::new(&e, &receipt_token_id).burn(from, &amount);
    Ok(())
}

fn check_nonnegative_amount(amount: i128) -> Result<(), FarmError> {
    if amount < 0 {
        Err(FarmError::InvalidAmount)
    } else {
        Ok(())
    }
}

fn check_nonzero_amount(amount: i128) -> Result<(), FarmError> {
    if amount == 0 {
        Err(FarmError::InvalidAmount)
    } else {
        Ok(())
    }
}

fn time(e: &Env) -> u64 {
    e.ledger().timestamp()
}

fn extend_instance_ttl(e: &Env) {
    e.storage()
        .instance()
        .extend_ttl(MAX_TTL - DAY_IN_LEDGERS, MAX_TTL)
}

