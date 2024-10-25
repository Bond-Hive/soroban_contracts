#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, Env, String, token,
};

pub(crate) const DAY_IN_LEDGERS: u32 = 17280;
pub(crate) const MAX_TTL: u32 = 3110400;
pub(crate) const DECIMALS: u32 = 7;

#[derive(Clone, Copy)]
#[contracttype]
pub enum DataKey {
    Admin = 0,
    RewardedToken1 = 1,
    RewardedToken2 = 2,
    AllocatedRewards1 = 3, // Global allocated rewards for token 1
    AllocatedRewards2 = 4, // Global allocated rewards for token 2
    PoolCounter = 5,       // DataKey for pool counter
    Maturity = 6,          // DataKey for Maturity
    Initialized = 7,       // DataKey to track if the contract is initialized
    PoolData = 8,          // Prefix for pool data
    UserData = 9,          // Prefix for user data
    PoolToken = 10,        // Global pool token
    Stopped = 11,          // For stop switch
    MaxRewardRatio1 = 12,
    MaxRewardRatio2 = 13,
}

#[contracterror]
#[repr(u32)]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub enum FarmError {
    InvalidAmount = 1,
    NotInitialized = 2,
    NotAuthorized = 3,
    PoolNotActive = 4,
    WithdrawError = 5,
    InsufficientRewards = 6,
    PoolNotFound = 7,
    UserNotFound = 8,
    SameRewardTokens = 9,
    TokenConflict = 10,
    AlreadyInitialized = 11,
    ContractStopped = 12,
}

#[derive(Clone)]
#[contracttype]
pub struct Pool {
    pub start_time: u64,
    pub reward_ratio1: i128,
    pub reward_ratio2: i128,
}

#[derive(Clone)]
#[contracttype]
pub struct UserData {
    pub deposited: i128,
    pub deposit_time: u64,
    pub accrued_rewards1: i128,
    pub accrued_rewards2: i128,
}

#[contract]
pub struct Farm;

/// Helper function to generate unique keys for each pool.
fn pool_data_key(pool_id: u32) -> (u32, u32) {
    (DataKey::PoolData as u32, pool_id)
}

/// Helper function to generate unique keys for each user's data.
fn user_data_key(user: Address, pool_id: u32) -> (Address, u32) {
    (user, pool_id)
}

fn has_sufficient_rewards(e: &Env, required1: i128, required2: i128) -> Result<bool, FarmError> {
    let rewarded_token1 = get_rewarded_token1(e)?;
    let available1 = token::Client::new(e, &rewarded_token1).balance(&e.current_contract_address());
    if let Some(rewarded_token2) = get_rewarded_token2(e)? {
        let available2 = token::Client::new(e, &rewarded_token2).balance(&e.current_contract_address());
        Ok(available1 >= required1 && available2 >= required2)
    } else {
        Ok(available1 >= required1 && required2 == 0)
    }
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

fn put_rewarded_tokens(e: &Env, token1: Address, token2: Option<Address>) -> Result<(), FarmError> {
    if let Some(ref token2_addr) = token2 {
        if token1 == *token2_addr {
            return Err(FarmError::SameRewardTokens);
        }
    }
    e.storage()
        .instance()
        .set(&DataKey::RewardedToken1, &token1);
    if let Some(token2_addr) = token2 {
        e.storage().instance().set(&DataKey::RewardedToken2, &token2_addr);
    } else {
        e.storage().instance().remove(&DataKey::RewardedToken2);
    }
    Ok(())
}

fn put_maturity(e: &Env, maturity: u64) {
    e.storage().instance().set(&DataKey::Maturity, &maturity);
}

fn get_maturity(e: &Env) -> Result<u64, FarmError> {
    e.storage()
        .instance()
        .get(&DataKey::Maturity)
        .ok_or(FarmError::NotInitialized)
}

fn get_rewarded_token1(e: &Env) -> Result<Address, FarmError> {
    e.storage()
        .instance()
        .get(&DataKey::RewardedToken1)
        .ok_or(FarmError::NotInitialized)
}

fn get_rewarded_token2(e: &Env) -> Result<Option<Address>, FarmError> {
    Ok(e.storage().instance().get(&DataKey::RewardedToken2))
}

fn put_pool_token(e: &Env, pool_token: Address) {
    e.storage().instance().set(&DataKey::PoolToken, &pool_token);
}

fn get_pool_token(e: &Env) -> Result<Address, FarmError> {
    e.storage()
        .instance()
        .get(&DataKey::PoolToken)
        .ok_or(FarmError::NotInitialized)
}

fn put_allocated_rewards(e: &Env, allocated1: i128, allocated2: i128) {
    e.storage()
        .instance()
        .set(&DataKey::AllocatedRewards1, &allocated1);
    e.storage()
        .instance()
        .set(&DataKey::AllocatedRewards2, &allocated2);
}

fn get_allocated_rewards(e: &Env) -> Result<(i128, i128), FarmError> {
    let allocated1: i128 = e
        .storage()
        .instance()
        .get(&DataKey::AllocatedRewards1)
        .unwrap_or(Ok(0))?;
    let allocated2: i128 = e
        .storage()
        .instance()
        .get(&DataKey::AllocatedRewards2)
        .unwrap_or(Ok(0))?;
    Ok((allocated1, allocated2))
}

fn put_pool_data(e: &Env, pool_id: u32, pool: Pool) {
    let storage_key = pool_data_key(pool_id);
    e.storage().persistent().set(&storage_key, &pool);
}

fn get_pool_data(e: &Env, pool_id: u32) -> Result<Pool, FarmError> {
    let storage_key = pool_data_key(pool_id);
    e.storage()
        .persistent()
        .get(&storage_key)
        .ok_or(FarmError::PoolNotFound)
}

fn put_user_data(e: &Env, user: Address, pool_id: u32, user_data: UserData) {
    let storage_key = user_data_key(user, pool_id);
    e.storage().persistent().set(&storage_key, &user_data);
}

fn get_user_data(e: &Env, user: Address, pool_id: u32) -> Result<UserData, FarmError> {
    let storage_key = user_data_key(user, pool_id);
    e.storage()
        .persistent()
        .get(&storage_key)
        .ok_or(FarmError::UserNotFound)
}

/// Remove user data from storage.
fn remove_user_data(e: &Env, user: &Address, pool_id: u32) -> Result<(), FarmError> {
    let storage_key = user_data_key(user.clone(), pool_id);
    e.storage().persistent().remove(&storage_key);
    Ok(())
}

fn get_token_client2(e: &Env) -> Option<token::Client> {
    if let Ok(Some(rewarded_token2)) = get_rewarded_token2(e) {
        Some(token::Client::new(e, &rewarded_token2))
    } else {
        None
    }
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

fn put_pool_counter(e: &Env, counter: u32) {
    e.storage().instance().set(&DataKey::PoolCounter, &counter);
}

fn put_max_reward_ratios(
    e: &Env,
    ratio1: i128,
    ratio2: Option<i128>,
) -> Result<(), FarmError> {
    e.storage().instance().set(&DataKey::MaxRewardRatio1, &ratio1);
    if let Some(ratio2_value) = ratio2 {
        e.storage()
            .instance()
            .set(&DataKey::MaxRewardRatio2, &ratio2_value);
    } else {
        e.storage().instance().remove(&DataKey::MaxRewardRatio2);
    }
    Ok(())
}

fn get_max_reward_ratios(e: &Env) -> Result<(i128, Option<i128>), FarmError> {
    let ratio1: i128 = e
        .storage()
        .instance()
        .get(&DataKey::MaxRewardRatio1)
        .ok_or(FarmError::NotInitialized)?;
    let ratio2: Option<i128> = e.storage().instance().get(&DataKey::MaxRewardRatio2);
    Ok((ratio1, ratio2))
}

fn get_pool_counter(e: &Env) -> Result<u32, FarmError> {
    e.storage()
        .instance()
        .get(&DataKey::PoolCounter)
        .unwrap_or(Ok(0))
}

fn is_initialized(e: &Env) -> Result<bool, FarmError> {
    Ok(e.storage()
        .instance()
        .get(&DataKey::Initialized)
        .unwrap_or(0) == 1)
}

fn set_initialized(e: &Env) {
    e.storage().instance().set(&DataKey::Initialized, &1);
}

fn put_stopped(e: &Env, stopped: bool) {
    if stopped {
        e.storage().instance().set(&DataKey::Stopped, &1);
    } else {
        e.storage().instance().remove(&DataKey::Stopped);
    }
}

fn get_stopped(e: &Env) -> Result<bool, FarmError> {
    Ok(e.storage()
        .instance()
        .get(&DataKey::Stopped)
        .unwrap_or(0) == 1)
}

#[contractimpl]
impl Farm {
    pub fn initialize(
        e: &Env,
        admin: Address,
        rewarded_token1: Address,
        rewarded_token2: Option<Address>,
        pool_token: Address,
        maturity: u64,
        max_reward_ratio1: i128,
        max_reward_ratio2: Option<i128>,
    ) -> Result<String, FarmError> {
        // Check if the contract is already initialized
        if is_initialized(e)? {
            return Err(FarmError::AlreadyInitialized);
        }

        // Ensure that the reward tokens are not the same as the pool token
        if rewarded_token1 == pool_token {
            return Err(FarmError::TokenConflict);
        }
        if let Some(ref token2) = rewarded_token2 {
            if *token2 == pool_token {
                return Err(FarmError::TokenConflict);
            }
            if *token2 == rewarded_token1 {
                return Err(FarmError::SameRewardTokens);
            }
        }

        // Store the admin, reward tokens, pool token, and maturity in the contract's storage
        put_admin(e, &admin);
        put_rewarded_tokens(e, rewarded_token1.clone(), rewarded_token2.clone())?;
        put_pool_token(e, pool_token.clone());
        put_maturity(e, maturity);
        put_allocated_rewards(e, 0, 0); // Initialize global allocated rewards
        put_pool_counter(e, 0); // Initialize pool counter
        put_max_reward_ratios(e, max_reward_ratio1, max_reward_ratio2)?;

        set_initialized(e);

        e.events().publish(
            (symbol_short!("Init"), admin.clone()),
            (
                admin,
                rewarded_token1,
                rewarded_token2.clone(),
                pool_token,
                maturity,
                max_reward_ratio1,
                max_reward_ratio2,
            ),
        );

        Ok(String::from_str(e, "Ok"))
    }

    pub fn create_pool(
        e: &Env,
        start_time: u64,
        reward_ratio1: i128,
        reward_ratio2: Option<i128>,
    ) -> Result<u32, FarmError> {
        let admin = get_admin(e)?;
        admin.require_auth();
        extend_instance_ttl(e);

        // Get the global max reward ratios
        let (max_reward_ratio1, max_reward_ratio2) = get_max_reward_ratios(e)?;

        // Ensure the reward ratios are within the specified limits
        if reward_ratio1 > max_reward_ratio1 {
            return Err(FarmError::InvalidAmount);
        }
        if let Some(ratio2) = reward_ratio2 {
            if let Some(max_ratio2) = max_reward_ratio2 {
                if ratio2 > max_ratio2 {
                    return Err(FarmError::InvalidAmount);
                }
            } else {
                return Err(FarmError::InvalidAmount);
            }
        } else if max_reward_ratio2.is_some() {
            return Err(FarmError::InvalidAmount);
        }

        let mut counter = get_pool_counter(e)?;
        let pool = Pool {
            start_time,
            reward_ratio1,
            reward_ratio2: reward_ratio2.unwrap_or(0),
        };

        put_pool_data(e, counter, pool);

        counter += 1;
        put_pool_counter(e, counter);

        e.events()
            .publish((symbol_short!("NewPool"), admin.clone()), counter - 1);

        Ok(counter - 1)
    }

    pub fn deposit(
        e: &Env,
        depositor: Address,
        amount: i128,
        pool_id: u32,
    ) -> Result<i128, FarmError> {
        depositor.require_auth();
        extend_instance_ttl(e);

        if get_stopped(e)? {
            return Err(FarmError::ContractStopped);
        }

        check_nonnegative_amount(amount)?;
        check_nonzero_amount(amount)?;

        let pool = get_pool_data(e, pool_id)?;
        let pool_token = get_pool_token(e)?;
        let current_time = time(e);

        // Check if the current time has passed the maturity date
        let maturity = get_maturity(e)?;
        if current_time >= maturity {
            return Err(FarmError::PoolNotActive);
        }

        if current_time < pool.start_time {
            return Err(FarmError::PoolNotActive);
        }

        // Get existing user data or initialize it
        let mut user_data = get_user_data(e, depositor.clone(), pool_id).unwrap_or(UserData {
            deposited: 0,
            deposit_time: current_time,
            accrued_rewards1: 0,
            accrued_rewards2: 0,
        });

        let time_elapsed = core::cmp::min(
            current_time - user_data.deposit_time,
            maturity - user_data.deposit_time,
        );

        let accrued_yield1 = if pool.reward_ratio1 > 0 {
            (user_data.deposited * pool.reward_ratio1 * time_elapsed as i128) / 10i128.pow(DECIMALS)
        } else {
            0
        };

        let accrued_yield2 = if pool.reward_ratio2 > 0 && get_rewarded_token2(e)?.is_some() {
            (user_data.deposited * pool.reward_ratio2 * time_elapsed as i128) / 10i128.pow(DECIMALS)
        } else {
            0
        };

        let time_to_maturity = maturity - current_time;

        // Allocate the new potential yield based on the new total deposit
        let potential_yield1 = if pool.reward_ratio1 > 0 {
            (amount * pool.reward_ratio1 * time_to_maturity as i128) / 10i128.pow(DECIMALS)
        } else {
            0
        };
        let potential_yield2 = if pool.reward_ratio2 > 0 && get_rewarded_token2(e)?.is_some() {
            (amount * pool.reward_ratio2 * time_to_maturity as i128) / 10i128.pow(DECIMALS)
        } else {
            0
        };

        // Get current allocated rewards and update them
        let (mut allocated_rewards1, mut allocated_rewards2) = get_allocated_rewards(e)?;

        // Check if there is enough balance in the contract to cover these new yields
        if !has_sufficient_rewards(
            e,
            allocated_rewards1 + potential_yield1,
            allocated_rewards2 + potential_yield2,
        )? {
            return Err(FarmError::InsufficientRewards);
        }

        // Allocate the new rewards globally
        allocated_rewards1 += potential_yield1;
        allocated_rewards2 += potential_yield2;
        put_allocated_rewards(e, allocated_rewards1, allocated_rewards2);

        // Update the user's accrued rewards
        user_data.accrued_rewards1 += accrued_yield1;
        user_data.accrued_rewards2 += accrued_yield2;

        // Add the new deposit to the existing deposit amount
        user_data.deposited += amount;
        user_data.deposit_time = current_time; // Reset deposit time to the time of the new deposit

        token::Client::new(e, &pool_token).transfer(
            &depositor,
            &e.current_contract_address(),
            &amount,
        );
        put_user_data(e, depositor.clone(), pool_id, user_data);

        e.events()
            .publish((symbol_short!("Deposit"), depositor.clone()), amount);

        Ok(amount)
    }

    pub fn withdraw(
        e: &Env,
        withdrawer: Address,
        amount: i128,
        pool_id: u32,
    ) -> Result<i128, FarmError> {
        withdrawer.require_auth();
        extend_instance_ttl(e);

        check_nonnegative_amount(amount)?;

        let pool = get_pool_data(e, pool_id)?;
        let pool_token = get_pool_token(e)?;
        let current_time = time(e);

        let mut user_data = get_user_data(e, withdrawer.clone(), pool_id)?;

        if amount > user_data.deposited {
            return Err(FarmError::InvalidAmount);
        }

        if current_time < pool.start_time {
            return Err(FarmError::PoolNotActive);
        }

        let maturity = get_maturity(e)?;

        // Ensure that the time elapsed only considers up to the maturity date
        let time_elapsed = core::cmp::min(
            current_time - user_data.deposit_time,
            maturity - user_data.deposit_time,
        );

        let total_yield1 = if pool.reward_ratio1 > 0 {
            (user_data.deposited * pool.reward_ratio1 * time_elapsed as i128) / 10i128.pow(DECIMALS)
        } else {
            0
        };

        let total_yield2 = if pool.reward_ratio2 > 0 && get_rewarded_token2(e)?.is_some() {
            (user_data.deposited * pool.reward_ratio2 * time_elapsed as i128) / 10i128.pow(DECIMALS)
        } else {
            0
        };

        // Transfer the withdrawn amount back to the user
        if amount > 0 {
            token::Client::new(e, &pool_token).transfer(
                &e.current_contract_address(),
                &withdrawer,
                &amount,
            );
        }

        // Transfer accrued rewards up to the maturity date
        if user_data.accrued_rewards1 + total_yield1 > 0 {
            token::Client::new(e, &get_rewarded_token1(e)?).transfer(
                &e.current_contract_address(),
                &withdrawer,
                &(user_data.accrued_rewards1 + total_yield1),
            );
        }

        if user_data.accrued_rewards2 + total_yield2 > 0 {
            if let Some(rewarded_token2) = get_rewarded_token2(e)? {
                token::Client::new(e, &rewarded_token2).transfer(
                    &e.current_contract_address(),
                    &withdrawer,
                    &(user_data.accrued_rewards2 + total_yield2),
                );
            }
        }

        let (mut allocated_rewards1, mut allocated_rewards2) = get_allocated_rewards(e)?;
        allocated_rewards1 -= user_data.accrued_rewards1 + total_yield1;
        allocated_rewards2 -= user_data.accrued_rewards2 + total_yield2;

        // Adjust allocated rewards if the user withdraws early (i.e., before maturity)
        if current_time < maturity {
            let time_to_maturity = maturity - current_time;
            let full_yield1 = if pool.reward_ratio1 > 0 {
                (amount * pool.reward_ratio1 * time_to_maturity as i128) / 10i128.pow(DECIMALS)
            } else {
                0
            };
            let full_yield2 = if pool.reward_ratio2 > 0 && get_rewarded_token2(e)?.is_some() {
                (amount * pool.reward_ratio2 * time_to_maturity as i128) / 10i128.pow(DECIMALS)
            } else {
                0
            };

            // Reduce the global allocated rewards
            allocated_rewards1 -= full_yield1;
            allocated_rewards2 -= full_yield2;
            user_data.deposit_time = current_time;
        } else {
            user_data.deposit_time = maturity;
        }

        put_allocated_rewards(e, allocated_rewards1, allocated_rewards2);

        // Update the user's deposited balance and reset accrued rewards
        user_data.deposited -= amount;
        user_data.accrued_rewards1 = 0;
        user_data.accrued_rewards2 = 0;

        if user_data.deposited > 0 {
            put_user_data(e, withdrawer.clone(), pool_id, user_data);
        } else {
            // Remove user data if all funds are withdrawn
            remove_user_data(e, &withdrawer, pool_id)?;
        }

        e.events()
            .publish((symbol_short!("Withdraw"), withdrawer.clone()), amount);

        Ok(amount)
    }

    pub fn set_admin(e: &Env, new_admin: Address) -> Result<String, FarmError> {
        let admin = get_admin(e)?;
        admin.require_auth();
        extend_instance_ttl(e);

        put_admin(e, &new_admin);

        e.events()
            .publish((symbol_short!("AdminChg"), admin.clone()), new_admin);

        Ok(String::from_str(e, "Ok"))
    }

    pub fn withdraw_unallocated_rewards(
        e: &Env,
    ) -> Result<(i128, i128), FarmError> {
        let admin = get_admin(e)?;
        admin.require_auth();

        let current_time = time(e);
        let maturity = get_maturity(e)?;

        // Ensure that the current time is after the maturity date
        if current_time < maturity {
            return Err(FarmError::NotAuthorized);
        }

        let rewarded_token1 = get_rewarded_token1(e)?;

        // Get the total allocated rewards that should not be withdrawn
        let (allocated_rewards1, allocated_rewards2) = get_allocated_rewards(e)?;

        let token_client1 = token::Client::new(e, &rewarded_token1);
        let available_balance1: i128 = token_client1.balance(&e.current_contract_address());
        let unallocated_rewards1 = core::cmp::max(available_balance1 - allocated_rewards1, 0);

        let token_client2 = get_token_client2(e); // Get token client 2 if it exists

        // Get the current balance of the contract
        let available_balance2 = token_client2
            .as_ref()
            .map_or(0, |client| client.balance(&e.current_contract_address()));

        // Calculate unallocated rewards
        let unallocated_rewards2 = core::cmp::max(available_balance2 - allocated_rewards2, 0);

        // Transfer unallocated rewards to the admin
        if unallocated_rewards1 > 0 {
            token_client1.transfer(&e.current_contract_address(), &admin, &unallocated_rewards1);
        }

        if let Some(client) = token_client2 {
            if unallocated_rewards2 > 0 {
                client.transfer(&e.current_contract_address(), &admin, &unallocated_rewards2);
            }
        }

        e.events().publish(
            (symbol_short!("Withdraw"), admin.clone()),
            (unallocated_rewards1, unallocated_rewards2),
        );

        Ok((unallocated_rewards1, unallocated_rewards2))
    }

    pub fn set_contract_stopped(e: &Env, stopped: bool) -> Result<String, FarmError> {
        let current_admin = get_admin(e)?;
        current_admin.require_auth();

        put_stopped(e, stopped);

        e.events().publish((symbol_short!("Stopped"), current_admin.clone()), stopped);
        Ok(String::from_str(e, "Contract stopped"))
    }

    /// Public function to query the current pool counter.
    pub fn get_current_pool_counter(e: &Env) -> Result<u32, FarmError> {
        extend_instance_ttl(e);
        get_pool_counter(e)
    }

    /// Public function to query the maturity date.
    pub fn get_maturity_date(e: &Env) -> Result<u64, FarmError> {
        extend_instance_ttl(e);
        get_maturity(e)
    }

    /// Public function to query the allocated rewards.
    pub fn get_global_allocated_rewards(e: &Env) -> Result<(i128, i128), FarmError> {
        extend_instance_ttl(e);
        get_allocated_rewards(e)
    }

    /// Public function to query the admin address.
    pub fn get_admin_address(e: &Env) -> Result<Address, FarmError> {
        extend_instance_ttl(e);
        get_admin(e)
    }

    /// Public function to query a specific pool's data.
    pub fn get_pool_info(e: &Env, pool_id: u32) -> Result<Pool, FarmError> {
        extend_instance_ttl(e);
        get_pool_data(e, pool_id)
    }

    /// Public function to query a user's data for a specific pool.
    pub fn get_user_info(e: &Env, user: Address, pool_id: u32) -> Result<UserData, FarmError> {
        extend_instance_ttl(e);
        let mut user_data = get_user_data(e, user.clone(), pool_id)?;

        let pool = get_pool_data(e, pool_id)?;
        let current_time = time(e);

        // Calculate time elapsed since the last deposit or rewards update
        let maturity = get_maturity(e)?;
        let time_elapsed = core::cmp::min(
            current_time - user_data.deposit_time,
            maturity - user_data.deposit_time,
        );

        // Calculate current accrued rewards
        let accrued_yield1 = if pool.reward_ratio1 > 0 {
            (user_data.deposited * pool.reward_ratio1 * time_elapsed as i128) / 10i128.pow(DECIMALS)
        } else {
            0
        };

        let accrued_yield2 = if pool.reward_ratio2 > 0 && get_rewarded_token2(e)?.is_some() {
            (user_data.deposited * pool.reward_ratio2 * time_elapsed as i128) / 10i128.pow(DECIMALS)
        } else {
            0
        };

        // Update the user data with current accrued rewards
        user_data.accrued_rewards1 += accrued_yield1;
        user_data.accrued_rewards2 += accrued_yield2;

        Ok(user_data)
    }

    /// Public function to query the reward token addresses.
    pub fn get_reward_token_addresses(e: &Env) -> Result<(Address, Option<Address>), FarmError> {
        extend_instance_ttl(e);

        let rewarded_token1 = get_rewarded_token1(&e)?;
        let rewarded_token2 = get_rewarded_token2(&e)?;

        Ok((rewarded_token1, rewarded_token2))
    }
}

mod test;
