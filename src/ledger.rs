// src/ledger.rs
// Cyber-NOte 金鑰審計與加密檔案歸檔模組 (支援 GPG Packet 封包解析)

use chrono::Local;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::PathBuf;
use crate::GameConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyLedgerEntry {
    pub id: String,
    pub file_name: String,
    pub target_path: String,
    pub key_id: String,
    pub cipher_mode: String,
    pub iterations: u64,
    pub layer: u32,
    pub timestamp: String,
    pub file_size_bytes: usize,
    pub sha256: String,
    pub notes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyLedger {
    pub system: String,
    pub version: String,
    pub updated_at: String,
    pub records: Vec<KeyLedgerEntry>,
}

impl Default for KeyLedger {
    fn default() -> Self {
        Self {
            system: "Cyber-NOte".to_string(),
            version: "1.0.0".to_string(),
            updated_at: Local::now().to_rfc3339(),
            records: Vec::new(),
        }
    }
}

pub fn get_ledger_path() -> PathBuf {
    let app_config_dir = GameConfig::get_app_config_dir();
    app_config_dir.join("key_ledger.json")
}

pub fn compute_sha256(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

pub fn load_ledger() -> KeyLedger {
    let path = get_ledger_path();
    if path.exists() {
        if let Ok(content) = fs::read_to_string(&path) {
            if let Ok(ledger) = serde_json::from_str::<KeyLedger>(&content) {
                return ledger;
            }
        }
    }
    KeyLedger::default()
}

pub fn record_ledger_entry(
    file_name: &str,
    target_path: &str,
    key_id: &str,
    cipher_mode: &str,
    iterations: u64,
    layer: u32,
    data: &[u8],
    notes: &str,
) -> Result<(), String> {
    let mut ledger = load_ledger();
    let now = Local::now();
    let id = format!("REC-{}-{}", now.format("%Y%m%d%H%M%S"), now.timestamp_subsec_millis());
    let sha256 = compute_sha256(data);

    let entry = KeyLedgerEntry {
        id,
        file_name: file_name.to_string(),
        target_path: target_path.to_string(),
        key_id: key_id.to_string(),
        cipher_mode: cipher_mode.to_string(),
        iterations,
        layer,
        timestamp: now.to_rfc3339(),
        file_size_bytes: data.len(),
        sha256,
        notes: notes.to_string(),
    };

    ledger.records.retain(|r| !(r.file_name == file_name && r.layer == layer));
    ledger.records.push(entry);
    ledger.updated_at = now.to_rfc3339();

    save_ledger(&ledger)
}

pub fn save_ledger(ledger: &KeyLedger) -> Result<(), String> {
    let path = get_ledger_path();
    let json = serde_json::to_string_pretty(ledger).map_err(|e| e.to_string())?;
    fs::write(&path, &json).map_err(|e| format!("無法寫入金鑰歸檔簿: {}", e))?;
    Ok(())
}

pub fn remove_ledger_entry(file_name: &str) -> Result<(), String> {
    let mut ledger = load_ledger();
    let before = ledger.records.len();
    ledger.records.retain(|r| r.file_name != file_name);
    if ledger.records.len() != before {
        ledger.updated_at = Local::now().to_rfc3339();
        save_ledger(&ledger)?;
    }
    Ok(())
}

pub fn extract_key_id_from_gpg_file(path: &std::path::Path) -> String {
    if let Ok(output) = std::process::Command::new("gpg")
        .arg("--list-packets")
        .arg(path)
        .output()
    {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let combined = format!("{}\n{}", stdout, stderr);
        for line in combined.lines() {
            if line.contains("keyid") || line.contains("key ID") || line.contains("issuer") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                for part in parts {
                    if part.len() >= 8 && part.chars().all(|c| c.is_ascii_hexdigit()) {
                        return part.to_string();
                    }
                }
            }
        }
    }
    "對稱加固 (S2K) 或 GPG 封包".to_string()
}

pub fn print_ledger_table() {
    println!("┌────────────────────────────────────────────────────────────────────────────────────────┐");
    println!("│ 🛡️  Cyber-NOte 金鑰歸檔審計簿 (Key Ledger Manifest & GPG Packet Inspection)            │");
    println!("├────────────────────────────────────────────────────────────────────────────────────────┤");
    
    let note_dir = GameConfig::get_note_dir();
    let mut gpg_files = Vec::new();
    if let Ok(entries) = fs::read_dir(&note_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if name.ends_with(".gpg") {
                        gpg_files.push((name.to_string(), path));
                    }
                }
            }
        }
    }

    if gpg_files.is_empty() {
        println!("  (尚無本地 .gpg 加密檔案供審計)");
        println!("└────────────────────────────────────────────────────────────────────────────────────────┘");
        return;
    }

    println!("{:<28} {:<36} {:<24}", "加密檔案名稱", "GPG Packet 封包解析金鑰 ID", "檔案狀態");
    println!("{:-<28} {:-<36} {:-<24}", "", "", "");
    for (name, path) in gpg_files {
        let key_id = extract_key_id_from_gpg_file(&path);
        let size = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        println!(
            "{:<28} {:<36} {:<24}",
            name,
            key_id,
            format!("{} bytes (正常)", size)
        );
    }
    println!("└────────────────────────────────────────────────────────────────────────────────────────┘");
}
