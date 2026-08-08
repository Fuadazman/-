#![no_std]
/**
 * © جميع الحقوق محفوظة 2026 - المطور: فؤاد يحيى عزمان
 * البريد الإلكتروني: fuad.mithaq@gmail.com
 * Pi Chat: @Fuad207
 * مشروع: ميثاق (Mithaq) - سجل الشهادات (v1.0.3 - Mainnet Hardened + Audit Ready)
 * الملف: contracts/src/certificate.rs
 * الوصف: سجل شهادات SBT غير قابلة للتحويل لإثبات إتمام الالتزامات على شبكة ستيلر.
 * متوافق مع استدعاء العقد الرئيسي v7.0 ومعزز بإدارة متقدمة لـ TTL.
 */
use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, Address, BytesN, Env, Map, String, Symbol,
};

const RENT_THRESHOLD: u32 = 172_800;
const RENT_EXTEND: u32 = 6_312_000;
const MAX_EXPIRY_YEARS: u64 = 50;
const SECONDS_IN_YEAR: u64 = 31_536_000;

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CertOnChain {
    pub holder: Address,
    pub counterparty: Address,
    pub issue_date: u64,
    pub expiry_date: u64,
    pub revoked: bool,
    pub revocation_reason: Option<String>,
    pub imprint: String,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    Admin,
    AuthorizedIssuers,
    Certificate(String),
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CertData {
    pub holder: Address,
    pub counterparty: Address,
    pub holder_name: String,
    pub certificate_type: Symbol,
    pub issue_date: u64,
    pub expiry_date: u64,
    pub success_rate: u32,
    pub data_hash: BytesN<32>,
    pub imprint_text: String,
}

fn bytes32_to_string(env: &Env, bytes: &BytesN<32>) -> String {
    let hex_chars: [u8; 16] = [
        b'0', b'1', b'2', b'3', b'4', b'5', b'6', b'7',
        b'8', b'9', b'a', b'b', b'c', b'd', b'e', b'f',
    ];
    let mut buf = [0u8; 64];
    for (i, byte) in bytes.to_array().iter().enumerate() {
        buf[i * 2] = hex_chars[(byte >> 4) as usize];
        buf[i * 2 + 1] = hex_chars[(byte & 0xf) as usize];
    }
    String::from_bytes(env, &buf)
}

#[contract]
pub struct CertificateRegistry;

#[contractimpl]
impl CertificateRegistry {
    pub fn initialize(env: Env, admin: Address) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("Contract already initialized");
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);

        let mut issuers: Map<Address, bool> = Map::new(&env);
        issuers.set(admin.clone(), true);
        let issuers_key = DataKey::AuthorizedIssuers;
        env.storage().persistent().set(&issuers_key, &issuers);
        env.storage()
           .persistent()
           .extend_ttl(&issuers_key, RENT_THRESHOLD, RENT_EXTEND);
    }

    pub fn register_certificate(
        env: Env,
        issuer: Address,
        certificate_id: String,
        data: CertData,
    ) -> bool {
        issuer.require_auth();

        let issuers_key = DataKey::AuthorizedIssuers;
        let issuers: Map<Address, bool> = env
           .storage()
           .persistent()
           .get(&issuers_key)
           .unwrap_or_else(|| panic!("Issuers map not set"));
        env.storage()
           .persistent()
           .extend_ttl(&issuers_key, RENT_THRESHOLD, RENT_EXTEND);

        if !issuers.get(issuer.clone()).unwrap_or(false) {
            panic!("Unauthorized issuer");
        }

        let cert_key = DataKey::Certificate(certificate_id.clone());
        if env.storage().persistent().has(&cert_key) {
            panic!("Certificate ID already exists");
        }
        if data.success_rate > 100 {
            panic!("Invalid success rate");
        }

        let now = env.ledger().timestamp();
        if data.issue_date > now {
            panic!("Issue date in future");
        }
        if data.expiry_date <= data.issue_date {
            panic!("Expiry before issue");
        }
        if data.expiry_date > now + MAX_EXPIRY_YEARS * SECONDS_IN_YEAR {
            panic!("Expiry too far");
        }

        let cert_onchain = CertOnChain {
            holder: data.holder,
            counterparty: data.counterparty,
            issue_date: data.issue_date,
            expiry_date: data.expiry_date,
            revoked: false,
            revocation_reason: None,
            imprint: data.imprint_text,
        };

        env.storage().persistent().set(&cert_key, &cert_onchain);
        env.storage()
           .persistent()
           .extend_ttl(&cert_key, RENT_THRESHOLD, RENT_EXTEND);

        let topics = (
            symbol_short!("cert_iss"),
            data.certificate_type,
            certificate_id,
            data.holder,
        );
        let event_data = (
            data.holder_name,
            issuer,
            data.certificate_type,
            data.issue_date,
            data.expiry_date,
            data.success_rate,
            data.data_hash,
        );
        env.events().publish(topics, event_data);
        true
    }

    pub fn issue_certificate(
        env: Env,
        issuer: Address,
        certificate_id: BytesN<32>,
        holder: Address,
        counterparty: Address,
        imprint_text: String,
    ) -> bool {
        issuer.require_auth();

        let issuers_key = DataKey::AuthorizedIssuers;
        let issuers: Map<Address, bool> = env
           .storage()
           .persistent()
           .get(&issuers_key)
           .unwrap_or_else(|| panic!("Issuers map not set"));
        env.storage()
           .persistent()
           .extend_ttl(&issuers_key, RENT_THRESHOLD, RENT_EXTEND);

        if !issuers.get(issuer.clone()).unwrap_or(false) {
            panic!("Unauthorized issuer");
        }

        let cert_id_string = bytes32_to_string(&env, &certificate_id);
        let cert_key = DataKey::Certificate(cert_id_string.clone());
        if env.storage().persistent().has(&cert_key) {
            panic!("Certificate ID already exists");
        }

        let now = env.ledger().timestamp();
        let expiry = now + (365 * SECONDS_IN_YEAR);

        let cert_onchain = CertOnChain {
            holder,
            counterparty,
            issue_date: now,
            expiry_date: expiry,
            revoked: false,
            revocation_reason: None,
            imprint: imprint_text,
        };

        env.storage().persistent().set(&cert_key, &cert_onchain);
        env.storage()
           .persistent()
           .extend_ttl(&cert_key, RENT_THRESHOLD, RENT_EXTEND);

        let topics = (
            symbol_short!("cert_iss"),
            symbol_short!("CONTRACT"),
            cert_id_string,
            holder,
        );
        let data = (
            String::from_str(&env, ""),
            issuer,
            symbol_short!("CONTRACT"),
            now,
            expiry,
            100u32,
            certificate_id,
        );
        env.events().publish(topics, data);
        true
    }

    pub fn is_authorized_issuer(env: Env, issuer: Address) -> bool {
        let issuers_key = DataKey::AuthorizedIssuers;
        let issuers: Map<Address, bool> = env
           .storage()
           .persistent()
           .get(&issuers_key)
           .unwrap_or_else(|| Map::new(&env));
        env.storage()
           .persistent()
           .extend_ttl(&issuers_key, RENT_THRESHOLD, RENT_EXTEND);
        issuers.get(issuer).unwrap_or(false)
    }

    pub fn revoke_certificate(env: Env, certificate_id: String, reason: String) {
        let admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        admin.require_auth();
        if reason.is_empty() {
            panic!("Reason required");
        }

        let cert_key = DataKey::Certificate(certificate_id.clone());
        let mut cert: CertOnChain = env
           .storage()
           .persistent()
           .get(&cert_key)
           .unwrap_or_else(|| panic!("Certificate not found"));
        if cert.revoked {
            panic!("Already revoked");
        }

        cert.revoked = true;
        cert.revocation_reason = Some(reason.clone());
        env.storage().persistent().set(&cert_key, &cert);
        env.storage()
           .persistent()
           .extend_ttl(&cert_key, RENT_THRESHOLD, RENT_EXTEND);

        let topics = (symbol_short!("cert_rev"), certificate_id);
        env.events().publish(topics, (admin, reason));
    }

    pub fn verify_certificate(env: Env, certificate_id: String) -> (bool, Option<CertOnChain>) {
        let cert_key = DataKey::Certificate(certificate_id);
        if let Some(cert) = env.storage().persistent().get::<_, CertOnChain>(&cert_key) {
            env.storage()
               .persistent()
               .extend_ttl(&cert_key, RENT_THRESHOLD, RENT_EXTEND);
            let now = env.ledger().timestamp();
            let is_valid = !cert.revoked && now <= cert.expiry_date;
            return (is_valid, Some(cert));
        }
        (false, None)
    }

    pub fn add_issuer(env: Env, new_issuer: Address) {
        let admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        admin.require_auth();

        let issuers_key = DataKey::AuthorizedIssuers;
        let mut issuers: Map<Address, bool> = env
           .storage()
           .persistent()
           .get(&issuers_key)
           .unwrap_or_else(|| Map::new(&env));
        issuers.set(new_issuer, true);
        env.storage().persistent().set(&issuers_key, &issuers);
        env.storage()
           .persistent()
           .extend_ttl(&issuers_key, RENT_THRESHOLD, RENT_EXTEND);
    }

    pub fn remove_issuer(env: Env, issuer_to_remove: Address) {
        let admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        admin.require_auth();

        let issuers_key = DataKey::AuthorizedIssuers;
        let mut issuers: Map<Address, bool> = env
           .storage()
           .persistent()
           .get(&issuers_key)
           .unwrap_or_else(|| Map::new(&env));
        issuers.remove(issuer_to_remove);
        env.storage().persistent().set(&issuers_key, &issuers);
        env.storage()
           .persistent()
           .extend_ttl(&issuers_key, RENT_THRESHOLD, RENT_EXTEND);
    }

    pub fn transfer_admin(env: Env, new_admin: Address) {
        let admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        admin.require_auth();
        new_admin.require_auth();

        let issuers_key = DataKey::AuthorizedIssuers;
        let mut issuers: Map<Address, bool> = env
           .storage()
           .persistent()
           .get(&issuers_key)
           .unwrap_or_else(|| Map::new(&env));
        issuers.remove(admin.clone());
        issuers.set(new_admin.clone(), true);
        env.storage().persistent().set(&issuers_key, &issuers);
        env.storage().persistent().extend_ttl(&issuers_key, RENT_THRESHOLD, RENT_EXTEND);

        env.storage().instance().set(&DataKey::Admin, &new_admin);
    }

    pub fn get_admin(env: Env) -> Address {
        env.storage().instance().get(&DataKey::Admin).unwrap()
    }
} 