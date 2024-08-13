#![no_std]

mod token;

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, String, Address, BytesN, Env, Map,
};

use token::create_contract;

pub(crate) const DAY_IN_LEDGERS: u32 = 17280;
pub(crate) const MAX_TTL: u32 = 3110400;
pub(crate) const DECIMALS: u32 = 7;

#[derive(Clone, Copy)]
#[contracttype]
pub enum DataKey {
    Admin = 0,
    TokenShare = 1,
    RewardedToken1 = 2,
    RewardedToken2 = 3,
    AllocatedRewards1 = 4, // Global allocated rewards for token 1
    AllocatedRewards2 = 5, // Global allocated rewards for token 2
    PoolMap = 6,           // DataKey for Pool Map
    PoolCounter = 7,       // DataKey for pool counter
    UserMap = 8,           // DataKey for User Data Map
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
}

#[derive(Clone)]
#[contracttype]
pub struct Pool {
    pub token: Address,
    pub start_time: u64,
    pub expiration_date: u64,
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

fn has_sufficient_rewards(e: &Env, required1: i128, required2: i128) -> Result<bool, FarmError> {
    let rewarded_token1 = get_rewarded_token1(e)?;
    let rewarded_token2 = get_rewarded_token2(e)?;

    let available1 = token::Client::new(e, &rewarded_token1).balance(&e.current_contract_address());
    let available2 = token::Client::new(e, &rewarded_token2).balance(&e.current_contract_address());

    Ok(available1 >= required1 && available2 >= required2)
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

fn put_allocated_rewards(e: &Env, allocated1: i128, allocated2: i128) {
    e.storage().instance().set(&DataKey::AllocatedRewards1, &allocated1);
    e.storage().instance().set(&DataKey::AllocatedRewards2, &allocated2);
}

fn get_allocated_rewards(e: &Env) -> Result<(i128, i128), FarmError> {
    let allocated1: i128 = e.storage().instance().get(&DataKey::AllocatedRewards1).unwrap_or(Ok(0))?;
    let allocated2: i128 = e.storage().instance().get(&DataKey::AllocatedRewards2).unwrap_or(Ok(0))?;
    Ok((allocated1, allocated2))
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

fn put_pool_data(e: &Env, pool_id: u32, pool: Pool) {
    let mut pool_map: Map<u32, Pool> = e
        .storage()
        .instance()
        .get(&DataKey::PoolMap)
        .unwrap_or(Map::new(&e));

    pool_map.set(pool_id, pool);
    e.storage().instance().set(&DataKey::PoolMap, &pool_map);
}

fn get_pool_data(e: &Env, pool_id: u32) -> Result<Pool, FarmError> {
    let pool_map: Map<u32, Pool> = e
        .storage()
        .instance()
        .get(&DataKey::PoolMap)
        .unwrap_or(Map::new(&e));

    pool_map.get(pool_id).ok_or(FarmError::PoolNotFound)
}

fn put_user_data(e: &Env, user: Address, pool_id: u32, user_data: UserData) {
    let mut user_map: Map<(Address, u32), UserData> = e
        .storage()
        .instance()
        .get(&DataKey::UserMap)
        .unwrap_or(Map::new(&e));

    user_map.set((user, pool_id), user_data);
    e.storage().instance().set(&DataKey::UserMap, &user_map);
}

fn get_user_data(e: &Env, user: Address, pool_id: u32) -> Result<UserData, FarmError> {
    let user_map: Map<(Address, u32), UserData> = e
        .storage()
        .instance()
        .get(&DataKey::UserMap)
        .unwrap_or(Map::new(&e));

    user_map.get((user, pool_id)).ok_or(FarmError::UserNotFound)
}

fn remove_user_data(e: &Env, withdrawer: &Address, pool_id: u32) -> Result<(), FarmError> {
    let mut user_map: Map<(Address, u32), UserData> = e
        .storage()
        .instance()
        .get(&DataKey::UserMap)
        .unwrap_or(Map::new(&e));
    
    user_map.remove((withdrawer.clone(), pool_id));
    e.storage().instance().set(&DataKey::UserMap, &user_map);

    Ok(())
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

fn put_pool_counter(e: &Env, counter: u32) {
    e.storage().instance().set(&DataKey::PoolCounter, &counter);
}

fn get_pool_counter(e: &Env) -> Result<u32, FarmError> {
    e.storage()
        .instance()
        .get(&DataKey::PoolCounter)
        .unwrap_or(Ok(0))
}

#[contractimpl]
impl Farm {
    pub fn initialize(
        e: Env,
        admin: Address,
        rewarded_token1: Address,
        rewarded_token2: Address,
        token_wasm_hash: BytesN<32>,
    ) -> Result<String, FarmError> {
        // Create the receipt token contract and initialize it
        let receipt_token_id = create_contract(&e, token_wasm_hash, &e.current_contract_address());
        token::Client::new(&e, &receipt_token_id).initialize(
            &e.current_contract_address(),
            &7u32,
            &String::from_str(&e, "bondHive"),
            &String::from_str(&e, "BHFARM"),
        );

        // Store the admin, receipt token, and rewarded tokens in the contract's storage
        put_admin(&e, &admin);
        put_token_share(&e, receipt_token_id);
        put_rewarded_tokens(&e, rewarded_token1, rewarded_token2);
        put_allocated_rewards(&e, 0, 0); // Initialize global allocated rewards
        put_pool_counter(&e, 0); // Initialize pool counter

        Ok(String::from_str(&e, "Ok"))
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
        let pool = Pool {
            token,
            start_time,
            expiration_date,
            reward_ratio1,
            reward_ratio2,
        };

        put_pool_data(&e, counter, pool);

        counter += 1;
        put_pool_counter(&e, counter);

        e.events().publish(
            (symbol_short!("NewPool"), admin.clone()),
            counter - 1,
        );

        Ok(counter - 1)
    }

    pub fn deposit(
        e: Env,
        depositor: Address,
        amount: i128,
        pool_id: u32,
    ) -> Result<i128, FarmError> {
        depositor.require_auth();
        extend_instance_ttl(&e);

        check_nonnegative_amount(amount)?;
        check_nonzero_amount(amount)?;

        let pool = get_pool_data(&e, pool_id)?;
        let current_time = time(&e);

        if current_time < pool.start_time {
            return Err(FarmError::PoolNotActive);
        }

        // Get existing user data or initialize it
        let mut user_data = get_user_data(&e, depositor.clone(), pool_id).unwrap_or(UserData {
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
        put_user_data(&e, depositor.clone(), pool_id, user_data);

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

        // Get current allocated rewards and update them
        let (mut allocated_rewards1, mut allocated_rewards2) = get_allocated_rewards(&e)?;

        // Check if there is enough balance in the contract to cover these new yields
        if !has_sufficient_rewards(
            &e,
            allocated_rewards1 + potential_yield1,
            allocated_rewards2 + potential_yield2,
        )? {
            return Err(FarmError::InsufficientRewards);
        }

        // Allocate the new rewards globally
        allocated_rewards1 += potential_yield1;
        allocated_rewards2 += potential_yield2;
        put_allocated_rewards(&e, allocated_rewards1, allocated_rewards2);

        e.events().publish(
            (symbol_short!("Deposit"), depositor.clone()),
            amount,
        );

        Ok(amount)
    }

    pub fn withdraw(
        e: Env,
        withdrawer: Address,
        amount: i128,
        pool_id: u32,
    ) -> Result<i128, FarmError> {
        withdrawer.require_auth();
        extend_instance_ttl(&e);

        check_nonnegative_amount(amount)?;

        let pool = get_pool_data(&e, pool_id)?;
        let current_time = time(&e);

        let mut user_data = get_user_data(&e, withdrawer.clone(), pool_id)?;

        if amount > user_data.deposited {
            return Err(FarmError::InvalidAmount);
        }

        if current_time < pool.start_time {
            return Err(FarmError::PoolNotActive);
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

        let (mut allocated_rewards1, mut allocated_rewards2) = get_allocated_rewards(&e)?;

        // Reduce the global allocated rewards
        allocated_rewards1 -= full_yield1;
        allocated_rewards2 -= full_yield2;
        put_allocated_rewards(&e, allocated_rewards1, allocated_rewards2);

        // Update the user's deposited balance and accrued rewards
        user_data.deposited -= amount;
        user_data.accrued_rewards1 = 0;
        user_data.accrued_rewards2 = 0;
        user_data.deposit_time = current_time;

        if user_data.deposited > 0 {
            put_user_data(&e, withdrawer.clone(), pool_id, user_data);
        } else {
            // Remove user data if all funds are withdrawn
            remove_user_data(&e, &withdrawer, pool_id)?;
        }

        e.events().publish(
            (symbol_short!("Withdraw"), withdrawer.clone()),
            amount,
        );

        Ok(amount)
    }

    pub fn set_admin(e: Env, new_admin: Address) -> Result<String, FarmError> {
        let admin = get_admin(&e)?;
        admin.require_auth();
        extend_instance_ttl(&e);

        put_admin(&e, &new_admin);

        e.events().publish(
            (symbol_short!("AdminChg"), new_admin.clone()),
            new_admin,
        );

        Ok(String::from_str(&e, "Ok"))
    }

    pub fn get_receipt_token_id(e: Env) -> Result<Address, FarmError> {
        extend_instance_ttl(&e);
        get_receipt_token_id_internal(&e)
    }
}

mod test;