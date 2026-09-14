// src/ledger.rs
// Cyber-NOte 金鑰審計與加密檔案歸檔模組

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
    pub cipher_mode: String, // "GPG_PUBLIC_KEY" 或 "GPG_SYMMETRIC_S2K"
    pub iterations: u64,     // S2K 迭代輪數，例如 65011712
    pub layer: u32,          // 巢狀封裝層級 (1, 2, 3...)
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

    // 如果該檔案已存在舊記錄，更新或新增審計記錄
    ledger.records.retain(|r| !(r.file_name == file_name && r.layer == layer));
    ledger.records.push(entry);
    ledger.updated_at = now.to_rfc3339();

    let path = get_ledger_path();
    let json = serde_json::to_string_pretty(&ledger).map_err(|e| e.to_string())?;
    fs::write(&path, &json).map_err(|e| format!("無法寫入金鑰歸檔簿: {}", e))?;

    // 同步備份至筆記資料夾內
    let note_dir = GameConfig::get_note_dir();
    let note_ledger_path = note_dir.join("key_ledger.json");
    let _ = fs::write(note_ledger_path, &json);

    Ok(())
}

pub fn print_ledger_table() {
    let ledger = load_ledger();
    println!("┌────────────────────────────────────────────────────────────────────────────────────────┐");
    println!("│ 🛡️  Cyber-NOte 金鑰歸檔審計簿 (Key Ledger Manifest)                                    │");
    println!("├────────────────────────────────────────────────────────────────────────────────────────┤");
    println!("│ 系統標識 : {:<74} │", ledger.system);
    println!("│ 存檔路徑 : {:<74} │", get_ledger_path().to_str().unwrap_or(""));
    println!("│ 記錄總數 : {:<74} │", ledger.records.len());
    println!("└────────────────────────────────────────────────────────────────────────────────────────┘");

    if ledger.records.is_empty() {
        println!("  (尚無檔案加密歸檔記錄)");
        return;
    }

    println!("{:<22} {:<10} {:<24} {:<12} {:<12}", "檔案名稱", "封裝層級", "鎖定金鑰 ID / 模式", "S2K 迭代次數", "時間戳記");
    println!("{:-<22} {:-<10} {:-<24} {:-<12} {:-<12}", "", "", "", "", "");
    for rec in ledger.records.iter().rev() {
        let key_display = if rec.cipher_mode == "GPG_SYMMETRIC_S2K" {
            "S2K-對稱密碼".to_string()
        } else if rec.key_id.len() > 20 {
            format!("{}...", &rec.key_id[..18])
        } else {
            rec.key_id.clone()
        };

        let iter_display = if rec.iterations > 0 {
            format!("{} 輪", rec.iterations)
        } else {
            "-".to_string()
        };

        let time_display = if rec.timestamp.len() >= 19 {
            &rec.timestamp[..19]
        } else {
            &rec.timestamp
        };

        println!(
            "{:<22} {:<10} {:<24} {:<12} {:<12}",
            rec.file_name,
            format!("Layer {}", rec.layer),
            key_display,
            iter_display,
            time_display
        );
    }
    println!();
}
