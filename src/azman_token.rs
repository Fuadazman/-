#![no_std]
/**
 * © جميع الحقوق محفوظة 2026 - المطور: فؤاد يحيى عزمان
 * مشروع: ميثاق (Mithaq) - العقد الذكي الثالث: رمز Azman وصندوق الأمان (v1.7.1 - Governance Ready)
 * الملف: contracts/src/azman_token.rs
 * الوصف: يتوافق مع توقيع v7.0 للعقد الرئيسي ومع عقد الحوكمة v1.0.0.
 * يستخدم نسب رسوم قابلة للتعديل BPS مخزنة على السلسلة.
 * يدعم الخصم المباشر من قبل عقد الحوكمة (seize_for_governance).
 */
use soroban_sdk::{
    contract, contractimpl, contracttype, Address, Env, String, symbol_short,
};

const DECIMALS: u32 = 7;
const DECIMAL_FACTOR: i128 = 10_000_000;
const BPS_DENOMINATOR: i128 = 10000;

const PROPOSAL_PENALTY_AMOUNT: i128 = 10 * DECIMAL_FACTOR;
const DEVELOPER_SHARE: i128 = 5 * DECIMAL_FACTOR;
const SAFETY_FUND_SHARE: i128 = 5 * DECIMAL_FACTOR;

const DEFAULT_PLATFORM_FEE_RATE_BPS: i128 = 100;
const DEFAULT_INSURANCE_FEE_RATE_BPS: i128 = 100;

const DAY_IN_LEDGERS: u32 = 17280;
const INSTANCE_BUMP_AMOUNT: u32 = 30 * DAY_IN_LEDGERS;
const INSTANCE_LIFETIME_THRESHOLD: u32 = 15 * DAY_IN_LEDGERS;
const PERSISTENT_BUMP_AMOUNT: u32 = 30 * DAY_IN_LEDGERS;
const PERSISTENT_LIFETIME_THRESHOLD: u32 = 15 * DAY_IN_LEDGERS;

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AllowanceDataKey {
    pub from: Address,
    pub spender: Address,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AllowanceValue {
    pub amount: i128,
    pub expiration_ledger: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    TokenAdmin,
    MithaqContract,
    GovernanceContract,
    Treasury,
    SafetyFund,
    CirculatingSupply,
    PlatformFeeRateBPS,
    InsuranceFeeRateBPS,
    Balance(Address),
    Allowance(AllowanceDataKey),
}

fn extend_instance_ttl(env: &Env) {
    env.storage()
      .instance()
      .extend_ttl(INSTANCE_LIFETIME_THRESHOLD, INSTANCE_BUMP_AMOUNT);
}

fn extend_persistent_ttl(env: &Env, key: &DataKey) {
    env.storage()
      .persistent()
      .extend_ttl(key, PERSISTENT_LIFETIME_THRESHOLD, PERSISTENT_BUMP_AMOUNT);
}

fn read_balance(env: &Env, id: Address) -> i128 {
    let key = DataKey::Balance(id);
    let balance = env.storage().persistent().get(&key).unwrap_or(0i128);
    extend_persistent_ttl(env, &key);
    balance
}

fn write_balance(env: &Env, id: Address, amount: i128) {
    let key = DataKey::Balance(id);
    env.storage().persistent().set(&key, &amount);
    extend_persistent_ttl(env, &key);
}

#[contract]
pub struct AzmanToken;

#[contractimpl]
impl AzmanToken {

    pub fn initialize_token(
        env: Env,
        admin: Address,
        mithaq_contract: Address,
        governance_contract: Address,
        safety_fund: Address,
        initial_treasury_supply: i128,
    ) {
        if env.storage().instance().has(&DataKey::TokenAdmin) {
            panic!("Token already initialized");
        }
        admin.require_auth();

        env.storage().instance().set(&DataKey::TokenAdmin, &admin);
        env.storage().instance().set(&DataKey::MithaqContract, &mithaq_contract);
        env.storage().instance().set(&DataKey::GovernanceContract, &governance_contract);
        env.storage().instance().set(&DataKey::SafetyFund, &safety_fund);

        env.storage().persistent().set(&DataKey::PlatformFeeRateBPS, &DEFAULT_PLATFORM_FEE_RATE_BPS);
        env.storage().persistent().set(&DataKey::InsuranceFeeRateBPS, &DEFAULT_INSURANCE_FEE_RATE_BPS);
        extend_persistent_ttl(&env, &DataKey::PlatformFeeRateBPS);
        extend_persistent_ttl(&env, &DataKey::InsuranceFeeRateBPS);

        write_balance(&env, admin.clone(), initial_treasury_supply);
        env.storage().persistent().set(&DataKey::Treasury, &admin);
        env.storage().persistent().set(&DataKey::CirculatingSupply, &0i128);

        extend_instance_ttl(&env);
        env.events().publish((symbol_short!("init"),), (admin, initial_treasury_supply));
    }

    pub fn name(env: Env) -> String {
        extend_instance_ttl(&env);
        String::from_str(&env, "Azman")
    }

    pub fn symbol(env: Env) -> String {
        extend_instance_ttl(&env);
        String::from_str(&env, "AZM")
    }

    pub fn decimals(env: Env) -> u32 {
        extend_instance_ttl(&env);
        DECIMALS
    }

    pub fn total_supply(env: Env) -> i128 {
        let supply_key = DataKey::CirculatingSupply;
        let supply = env.storage().persistent().get(&supply_key).unwrap_or(0i128);
        extend_persistent_ttl(&env, &supply_key);
        supply
    }

    pub fn balance(env: Env, id: Address) -> i128 {
        extend_instance_ttl(&env);
        read_balance(&env, id)
    }

    pub fn transfer(env: Env, from: Address, to: Address, amount: i128) {
        from.require_auth();
        extend_instance_ttl(&env);
        if amount <= 0 { panic!("Amount must be positive"); }
        if from == to { panic!("Cannot transfer to self"); }

        let from_balance = read_balance(&env, from.clone());
        if from_balance < amount { panic!("Insufficient balance"); }

        write_balance(&env, from.clone(), from_balance - amount);
        let to_balance = read_balance(&env, to.clone());
        write_balance(&env, to.clone(), to_balance + amount);

        env.events().publish((symbol_short!("transfer"), from, to), amount);
    }

    pub fn approve(env: Env, from: Address, spender: Address, amount: i128, expiration_ledger: u32) {
        from.require_auth();
        extend_instance_ttl(&env);
        if amount < 0 { panic!("Negative amount is not allowed"); }
        if expiration_ledger <= env.ledger().sequence() { panic!("Expiration ledger must be in future"); }

        let key = DataKey::Allowance(AllowanceDataKey { from: from.clone(), spender: spender.clone() });
        let allowance = AllowanceValue { amount, expiration_ledger };
        env.storage().temporary().set(&key, &allowance);
        env.storage().temporary().extend_ttl(&key, INSTANCE_LIFETIME_THRESHOLD, INSTANCE_BUMP_AMOUNT);

        env.events().publish((symbol_short!("approve"), from, spender), amount);
    }

    pub fn allowance(env: Env, from: Address, spender: Address) -> i128 {
        extend_instance_ttl(&env);
        let key = DataKey::Allowance(AllowanceDataKey { from, spender });
        if let Some(allowance) = env.storage().temporary().get::<_, AllowanceValue>(&key) {
            if allowance.expiration_ledger > env.ledger().sequence() {
                env.storage().temporary().extend_ttl(&key, INSTANCE_LIFETIME_THRESHOLD, INSTANCE_BUMP_AMOUNT);
                return allowance.amount;
            }
        }
        0i128
    }

    pub fn transfer_from(env: Env, spender: Address, from: Address, to: Address, amount: i128) {
        spender.require_auth();
        extend_instance_ttl(&env);
        if amount <= 0 { panic!("Amount must be positive"); }
        if from == to { panic!("Cannot transfer to self"); }

        let key = DataKey::Allowance(AllowanceDataKey { from: from.clone(), spender: spender.clone() });
        let mut current_allowance: AllowanceValue = env.storage().temporary().get(&key).unwrap_or_else(|| panic!("No allowance set"));

        if current_allowance.expiration_ledger <= env.ledger().sequence() { panic!("Allowance expired"); }
        if current_allowance.amount < amount { panic!("Insufficient allowance"); }

        current_allowance.amount -= amount;
        env.storage().temporary().set(&key, &current_allowance);
        env.storage().temporary().extend_ttl(&key, INSTANCE_LIFETIME_THRESHOLD, INSTANCE_BUMP_AMOUNT);

        let from_balance = read_balance(&env, from.clone());
        if from_balance < amount { panic!("Insufficient balance"); }
        write_balance(&env, from.clone(), from_balance - amount);

        let to_balance = read_balance(&env, to.clone());
        write_balance(&env, to.clone(), to_balance + amount);

        env.events().publish((symbol_short!("transfer"), from, to), amount);
    }

    pub fn release_from_treasury(env: Env, authorized_caller: Address, to: Address, amount: i128) {
        authorized_caller.require_auth();
        extend_instance_ttl(&env);

        let admin = env.storage().instance().get::<_, Address>(&DataKey::TokenAdmin).unwrap();
        let mithaq = env.storage().instance().get::<_, Address>(&DataKey::MithaqContract).unwrap();

        if authorized_caller != admin && authorized_caller != mithaq {
            panic!("Unauthorized: Only MithaqContract or Admin can release tokens");
        }
        if amount <= 0 { panic!("Amount must be positive"); }

        let treasury_addr = env.storage().persistent().get::<_, Address>(&DataKey::Treasury).unwrap();
        let treasury_balance = read_balance(&env, treasury_addr.clone());
        if treasury_balance < amount { panic!("Insufficient treasury balance"); }

        write_balance(&env, treasury_addr.clone(), treasury_balance - amount);

        let to_balance = read_balance(&env, to.clone());
        write_balance(&env, to.clone(), to_balance + amount);

        let supply_key = DataKey::CirculatingSupply;
        let supply = env.storage().persistent().get(&supply_key).unwrap_or(0i128);
        env.storage().persistent().set(&supply_key, &(supply + amount));
        extend_persistent_ttl(&env, &supply_key);

        env.events().publish((symbol_short!("release"), authorized_caller, to), amount);
    }

    pub fn seize_for_governance(env: Env, governance_contract: Address, from: Address, amount: i128) {
        let stored_gov: Address = env.storage().instance().get(&DataKey::GovernanceContract).unwrap_or_else(|| panic!("Governance contract not set"));
        if governance_contract != stored_gov { panic!("Unauthorized: Only Governance contract"); }
        governance_contract.require_auth();

        if amount <= 0 { panic!("Amount must be positive"); }

        let from_balance = read_balance(&env, from.clone());
        if from_balance < amount { panic!("Insufficient balance"); }

        write_balance(&env, from.clone(), from_balance - amount);
        let gov_balance = read_balance(&env, governance_contract.clone());
        write_balance(&env, governance_contract.clone(), gov_balance + amount);

        env.events().publish((symbol_short!("seize"), from, governance_contract), amount);
    }

    pub fn process_contract_fees(env: Env, mithaq_contract: Address, payer: Address, total_amount: i128) {
        mithaq_contract.require_auth();
        extend_instance_ttl(&env);

        let stored_mithaq = env.storage().instance().get::<_, Address>(&DataKey::MithaqContract).unwrap();
        if mithaq_contract != stored_mithaq { panic!("Unauthorized caller"); }

        let platform_rate: i128 = env.storage().persistent().get(&DataKey::PlatformFeeRateBPS).unwrap_or(DEFAULT_PLATFORM_FEE_RATE_BPS);
        let insurance_rate: i128 = env.storage().persistent().get(&DataKey::InsuranceFeeRateBPS).unwrap_or(DEFAULT_INSURANCE_FEE_RATE_BPS);
        extend_persistent_ttl(&env, &DataKey::PlatformFeeRateBPS);
        extend_persistent_ttl(&env, &DataKey::InsuranceFeeRateBPS);

        let platform_fee = (total_amount * platform_rate) / BPS_DENOMINATOR;
        let insurance_fee = (total_amount * insurance_rate) / BPS_DENOMINATOR;
        let total_fee = platform_fee + insurance_fee;

        if total_fee == 0 { return; }

        let payer_balance = read_balance(&env, payer.clone());
        if payer_balance < total_fee { panic!("Insufficient balance for fees"); }
        write_balance(&env, payer.clone(), payer_balance - total_fee);

        if platform_fee > 0 {
            let treasury_addr = env.storage().persistent().get::<_, Address>(&DataKey::Treasury).unwrap();
            let treasury_balance = read_balance(&env, treasury_addr.clone());
            write_balance(&env, treasury_addr.clone(), treasury_balance + platform_fee);

            let supply_key = DataKey::CirculatingSupply;
            let current_supply = env.storage().persistent().get(&supply_key).unwrap_or(0i128);
            env.storage().persistent().set(&supply_key, &(current_supply - platform_fee));
            extend_persistent_ttl(&env, &supply_key);
        }

        if insurance_fee > 0 {
            let fund_addr = env.storage().instance().get::<_, Address>(&DataKey::SafetyFund).unwrap();
            let fund_balance = read_balance(&env, fund_addr.clone());
            write_balance(&env, fund_addr.clone(), fund_balance + insurance_fee);
        }

        env.events().publish((symbol_short!("fees"), payer), (platform_fee, insurance_fee));
    }

    pub fn process_failed_proposal_penalty(env: Env, mithaq_contract: Address, developer: Address) {
        mithaq_contract.require_auth();
        extend_instance_ttl(&env);

        let stored_mithaq = env.storage().instance().get::<_, Address>(&DataKey::MithaqContract).unwrap();
        if mithaq_contract != stored_mithaq { panic!("Unauthorized caller"); }

        let dev_balance = read_balance(&env, developer.clone());
        write_balance(&env, developer.clone(), dev_balance + DEVELOPER_SHARE);

        let fund_addr = env.storage().instance().get::<_, Address>(&DataKey::SafetyFund).unwrap();
        let fund_balance = read_balance(&env, fund_addr.clone());
        write_balance(&env, fund_addr.clone(), fund_balance + SAFETY_FUND_SHARE);

        let supply_key = DataKey::CirculatingSupply;
        let current_supply = env.storage().persistent().get(&supply_key).unwrap_or(0i128);
        env.storage().persistent().set(&supply_key, &(current_supply + PROPOSAL_PENALTY_AMOUNT));
        extend_persistent_ttl(&env, &supply_key);

        env.events().publish((symbol_short!("penalty"), developer), (DEVELOPER_SHARE, SAFETY_FUND_SHARE));
    }

    pub fn claim_from_safety_fund(env: Env, mithaq_contract: Address, to: Address, amount: i128) {
        mithaq_contract.require_auth();
        extend_instance_ttl(&env);

        let stored_mithaq = env.storage().instance().get::<_, Address>(&DataKey::MithaqContract).unwrap();
        if mithaq_contract != stored_mithaq { panic!("Unauthorized MithaqContract"); }
        if amount <= 0 { panic!("Amount must be positive"); }

        let fund_addr = env.storage().instance().get::<_, Address>(&DataKey::SafetyFund).unwrap();
        let fund_balance = read_balance(&env, fund_addr.clone());
        if fund_balance < amount { panic!("Insufficient safety fund liquidity"); }

        write_balance(&env, fund_addr.clone(), fund_balance - amount);
        let to_balance = read_balance(&env, to.clone());
        write_balance(&env, to.clone(), to_balance + amount);

        env.events().publish((symbol_short!("claim"), to), amount);
    }

    pub fn burn(env: Env, from: Address, amount: i128) {
        from.require_auth();
        extend_instance_ttl(&env);
        if amount <= 0 { panic!("Amount must be positive"); }

        let from_balance = read_balance(&env, from.clone());
        if from_balance < amount { panic!("Insufficient balance"); }
        write_balance(&env, from.clone(), from_balance - amount);

        let supply_key = DataKey::CirculatingSupply;
        let supply = env.storage().persistent().get(&supply_key).unwrap_or(0i128);
        if supply < amount { panic!("Burn amount exceeds circulating supply"); }
        env.storage().persistent().set(&supply_key, &(supply - amount));
        extend_persistent_ttl(&env, &supply_key);

        env.events().publish((symbol_short!("burn"), from), amount);
    }

    pub fn update_mithaq_contract(env: Env, admin: Address, new_contract: Address) {
        admin.require_auth();
        extend_instance_ttl(&env);
        let current_admin = env.storage().instance().get::<_, Address>(&DataKey::TokenAdmin).unwrap();
        if admin != current_admin { panic!("Unauthorized: Only Admin"); }
        env.storage().instance().set(&DataKey::MithaqContract, &new_contract);
        env.events().publish((symbol_short!("upd_m"),), new_contract);
    }

    pub fn update_admin(env: Env, current_admin: Address, new_admin: Address) {
        current_admin.require_auth();
        extend_instance_ttl(&env);
        let stored_admin = env.storage().instance().get::<_, Address>(&DataKey::TokenAdmin).unwrap();
        if current_admin != stored_admin { panic!("Unauthorized"); }
        new_admin.require_auth();
        env.storage().instance().set(&DataKey::TokenAdmin, &new_admin);
        env.events().publish((symbol_short!("upd_a"),), new_admin);
    }

    pub fn set_fee_rates(env: Env, admin: Address, platform_rate_bps: i128, insurance_rate_bps: i128) {
        admin.require_auth();
        extend_instance_ttl(&env);
        let current_admin = env.storage().instance().get::<_, Address>(&DataKey::TokenAdmin).unwrap();
        if admin != current_admin { panic!("Unauthorized: Only Admin"); }
        if platform_rate_bps < 0 || insurance_rate_bps < 0 { panic!("Rates cannot be negative"); }
        if platform_rate_bps > 1000 || insurance_rate_bps > 1000 { panic!("Rate too high: Max 10%"); }

        env.storage().persistent().set(&DataKey::PlatformFeeRateBPS, &platform_rate_bps);
        env.storage().persistent().set(&DataKey::InsuranceFeeRateBPS, &insurance_rate_bps);
        extend_persistent_ttl(&env, &DataKey::PlatformFeeRateBPS);
        extend_persistent_ttl(&env, &DataKey::InsuranceFeeRateBPS);

        env.events().publish((symbol_short!("fees_upd"),), (platform_rate_bps, insurance_rate_bps));
    }

    pub fn set_governance_contract(env: Env, admin: Address, contract: Address) {
        admin.require_auth();
        extend_instance_ttl(&env);
        let current_admin = env.storage().instance().get::<_, Address>(&DataKey::TokenAdmin).unwrap();
        if admin != current_admin { panic!("Unauthorized: Only Admin"); }
        env.storage().instance().set(&DataKey::GovernanceContract, &contract);
        env.events().publish((symbol_short!("upd_gov"),), contract);
    }

    pub fn get_treasury(env: Env) -> Address {
        extend_instance_ttl(&env);
        env.storage().persistent().get(&DataKey::Treasury).unwrap()
    }

    pub fn get_safety_fund(env: Env) -> Address {
        extend_instance_ttl(&env);
        env.storage().instance().get(&DataKey::SafetyFund).unwrap()
    }

    pub fn get_mithaq_contract(env: Env) -> Address {
        extend_instance_ttl(&env);
        env.storage().instance().get(&DataKey::MithaqContract).unwrap()
    }

    pub fn get_token_admin(env: Env) -> Address {
        extend_instance_ttl(&env);
        env.storage().instance().get(&DataKey::TokenAdmin).unwrap()
    }

    pub fn get_governance_contract(env: Env) -> Address {
        extend_instance_ttl(&env);
        env.storage().instance().get(&DataKey::GovernanceContract).unwrap()
    }

    pub fn get_fee_rates(env: Env) -> (i128, i128) {
        extend_instance_ttl(&env);
        let platform_rate = env.storage().persistent().get(&DataKey::PlatformFeeRateBPS).unwrap_or(DEFAULT_PLATFORM_FEE_RATE_BPS);
        let insurance_rate = env.storage().persistent().get(&DataKey::InsuranceFeeRateBPS).unwrap_or(DEFAULT_INSURANCE_FEE_RATE_BPS);
        extend_persistent_ttl(&env, &DataKey::PlatformFeeRateBPS);
        extend_persistent_ttl(&env, &DataKey::InsuranceFeeRateBPS);
        (platform_rate, insurance_rate)
    }
}