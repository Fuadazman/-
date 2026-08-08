/**
 * © جميع الحقوق محفوظة 2026 - المطور: فؤاد يحيى عزمان
 * البريد الإلكتروني: fuad.mithaq@gmail.com
 * Pi Chat: @Fuad207
 * مشروع: ميثاق (Mithaq) - سجل الشهادات (v1.0.3 - Mainnet Hardened + Audit Ready)
 * الملف: contracts/src/certificate.rs
 * الوصف: سجل شهادات SBT غير قابلة للتحويل لإثبات إتمام الالتزامات على شبكة ستيلر.
 * متوافق مع استدعاء العقد الرئيسي v7.0 ومعزز بإدارة متقدمة لـ TTL.
 */

#![no_std]
use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, Address, BytesN, Env, Map, Option, String, Symbol,
};

// ===== الثوابت الاقتصادية والتنظيمية =====
const RENT_THRESHOLD: u32 = 172_800; // ~10 أيام (عتبة التجديد)
const RENT_EXTEND: u32 = 6_312_000; // ~سنة واحدة (مدة التمديد)
const MAX_EXPIRY_YEARS: u64 = 50; // أقصى مدة صلاحية
const SECONDS_IN_YEAR: u64 = 31_536_000; // ثانية في السنة

// ===== هياكل البيانات المصغرة Micro-State =====
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

/// تحويل BytesN<32> إلى String سداسي عشري (hex) للاستخدام كمفتاح شهادة.
fn bytes32_to_string(env: &Env, bytes: &BytesN<32>) -> String {
    let mut s = String::new(env);
    for byte in bytes.to_array().iter() {
        let hex_chars = [
            "0", "1", "2", "3", "4", "5", "6", "7", "8", "9", "a", "b", "c", "d", "e", "f",
        ];
        s.push_str(&String::from_str(env, hex_chars[(byte >> 4) as usize]));
        s.push_str(&String::from_str(env, hex_chars[(byte & 0xf) as usize]));
    }
    s
}

#[contract]
pub struct CertificateRegistry;

#[contractimpl]
impl CertificateRegistry {
    /// تهيئة العقد وتعيين المسؤول الأول وإضافته كجهة إصدار معتمدة.
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

    /// إصدار شهادة جديدة (للاستخدام العام). تتحقق من صلاحية المُصدر والبيانات المدخلة.
    pub fn register_certificate(
        env: Env,
        issuer: Address,
        certificate_id: String,
        holder: Address,
        counterparty: Address,
        holder_name: String,
        certificate_type: Symbol,
        issue_date: u64,
        expiry_date: u64,
        success_rate: u32,
        data_hash: BytesN<32>,
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

        let cert_key = DataKey::Certificate(certificate_id.clone());
        if env.storage().persistent().has(&cert_key) {
            panic!("Certificate ID already exists");
        }
        if success_rate > 100 {
            panic!("Invalid success rate");
        }

        let now = env.ledger().timestamp();
        if issue_date > now {
            panic!("Issue date in future");
        }
        if expiry_date <= issue_date {
            panic!("Expiry before issue");
        }
        if expiry_date > now + MAX_EXPIRY_YEARS * SECONDS_IN_YEAR {
            panic!("Expiry too far");
        }

        let cert_onchain = CertOnChain {
            holder,
            counterparty,
            issue_date,
            expiry_date,
            revoked: false,
            revocation_reason: None,
            imprint: imprint_text,
        };

        env.storage().persistent().set(&cert_key, &cert_onchain);
        env.storage()
           .persistent()
           .extend_ttl(&cert_key, RENT_THRESHOLD, RENT_EXTEND);

        let topics = (symbol_short!("cert_iss"), certificate_type, certificate_id, holder);
        let data = (
            holder_name,
            issuer,
            certificate_type,
            issue_date,
            expiry_date,
            success_rate,
            data_hash,
        );
        env.events().publish(topics, data);
        true
    }

    /// إصدار شهادة تلقائي من العقد الرئيسي (MithaqContract) بعد إتمام الالتزام.
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
        let expiry = now + (365 * SECONDS_IN_YEAR); // صلاحية افتراضية سنة واحدة

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

    /// التحقق من صلاحية جهة إصدار.
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

    /// إلغاء شهادة (للمسؤول فقط، مع سبب إلزامي).
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

    /// التحقق من صلاحية شهادة (غير ملغاة ولم تنته صلاحيتها).
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

    /// إضافة جهة إصدار جديدة (للمسؤول فقط).
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

    /// إزالة جهة إصدار (للمسؤول فقط).
    pub fn remove_issuer(env: Env, issuer_to_remove: Address) {
        let admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        admin.require_auth();

        let issuers_key = DataKey::AuthorizedIssuers;
        let mut issuers: Map<Address, bool> = env
           .storage()               // ← تم إصلاح الخطأ: نقطة واحدة فقط
           .persistent()
           .get(&issuers_key)
           .unwrap_or_else(|| Map::new(&env));
        issuers.remove(issuer_to_remove);
        env.storage().persistent().set(&issuers_key, &issuers);
        env.storage()
           .persistent()
           .extend_ttl(&issuers_key, RENT_THRESHOLD, RENT_EXTEND);
    }

    /// نقل صلاحية المسؤول (يتطلب توقيع كل من المسؤول الحالي والجديد).
    /// يتم أيضاً تحديث قائمة المصدرين تلقائياً.
    pub fn transfer_admin(env: Env, new_admin: Address) {
        let admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        admin.require_auth();
        new_admin.require_auth();

        // تحديث قائمة المصدرين
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

        // نقل الادمن
        env.storage().instance().set(&DataKey::Admin, &new_admin);
    }

    /// الاستعلام عن عنوان المسؤول الحالي.
    pub fn get_admin(env: Env) -> Address {
        env.storage().instance().get(&DataKey::Admin).unwrap()
    }
}