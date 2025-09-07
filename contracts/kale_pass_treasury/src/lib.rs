#![no_std]
use soroban_sdk::{contract, contracterror, contractimpl, contracttype, vec, Address, Env, Vec};

#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    Admin,
    Tiers,
    Stake(Address),
}

#[derive(Clone)]
#[contracttype]
pub struct Tier {
    pub threshold: u128,
    pub discount_bps: u32, // 0..=10_000 (100%)
}

#[contracterror]
#[derive(Copy, Clone, PartialEq, Eq)]
pub enum Err {
    NotInitialized = 1,
    AlreadyInitialized = 2,
    NotAdmin = 3,
    BadInput = 4,
}

fn read_admin(env: &Env) -> Address {
    env.storage()
        .instance()
        .get::<_, Address>(&DataKey::Admin)
        .unwrap_or_else(|| soroban_sdk::Env::panic_with_error(env, Err::NotInitialized))
}
fn require_admin(env: &Env) {
    read_admin(env).require_auth();
}

#[contract]
pub struct KalePassTreasury;

#[contractimpl]
impl KalePassTreasury {
    /// One-time init with default tiers: 0, 100, 500 KALE -> 0%, 20%, 40%
    pub fn init(env: Env, admin: Address) {
        if env.storage().instance().has(&DataKey::Admin) {
            soroban_sdk::Env::panic_with_error(&env, Err::AlreadyInitialized);
        }
        env.storage().instance().set(&DataKey::Admin, &admin);

        let tiers: Vec<Tier> = vec![
            &env,
            Tier { threshold: 0, discount_bps: 0 },
            Tier { threshold: 100, discount_bps: 2000 },
            Tier { threshold: 500, discount_bps: 4000 },
        ];
        env.storage().instance().set(&DataKey::Tiers, &tiers);
    }

    /// Replace discount tiers (admin only). Validates monotonic thresholds and bps<=10000.
    pub fn set_tiers(env: Env, tiers: Vec<Tier>) {
        require_admin(&env);
        // basic validation
        let mut prev: u128 = 0;
        for i in 0..tiers.len() {
            let t = tiers.get_unchecked(i);
            if t.discount_bps > 10_000 {
                soroban_sdk::Env::panic_with_error(&env, Err::BadInput);
            }
            if i > 0 && t.threshold < prev {
                soroban_sdk::Env::panic_with_error(&env, Err::BadInput);
            }
            prev = t.threshold;
        }
        env.storage().instance().set(&DataKey::Tiers, &tiers);
    }

    /// Prototype: admin sets a user’s staked amount (MVP; in prod this would be token-hook driven).
    pub fn admin_set_stake(env: Env, user: Address, amount: u128) {
        require_admin(&env);
        env.storage()
            .instance()
            .set(&DataKey::Stake(user), &amount);
    }

    /// Read the effective discount (bps) for `user` given current tiers & their stake.
    pub fn get_discount_bps(env: Env, user: Address) -> u32 {
        let tiers = env
            .storage()
            .instance()
            .get::<_, Vec<Tier>>(&DataKey::Tiers)
            .unwrap_or(vec![&env]); // empty vec if not initialized (defensive)

        let bal = env
            .storage()
            .instance()
            .get::<_, u128>(&DataKey::Stake(user))
            .unwrap_or(0);

        let mut best: u32 = 0;
        // Use index-based access to avoid iterator Option confusion on Soroban Vec
        for i in 0..tiers.len() {
            let t = tiers.get_unchecked(i);
            if bal >= t.threshold {
                // max across matching tiers
                if t.discount_bps > best {
                    best = t.discount_bps;
                }
            }
        }
        best
    }
}


#[cfg(test)]
mod test;