#![no_std]

pub mod azman_token;
pub mod certificate;
pub mod governance;
pub mod main_contract;

// تصدير العقد الرئيسي ليكون هو واجهة الـ WASM الخارجية
pub use crate::main_contract::MainContract;
