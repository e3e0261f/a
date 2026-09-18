use chrono::Utc;
use hmac::{Hmac, Mac};
use sha1::Sha1;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use crate::lib::GameConfig;

type HmacSha1 = Hmac<Sha1>;

pub fn get_totp_storage_path() -> PathBuf {
    GameConfig::get_app_config_dir().join("totp_secrets.json")
}

pub fn load_totp_secrets() -> HashMap<String, String> {
    let path = get_totp_storage_path();
    if path.exists() {
        if let Ok(content) = fs::read_to_string(&path) {
            if let Ok(map) = serde_json::from_str::<HashMap<String, String>>(&content) {
                return map;
            }
        }
    }
    HashMap::new()
}

pub fn save_totp_secrets(secrets: &HashMap<String, String>) -> std::io::Result<()> {
    let path = get_totp_storage_path();
    let content = serde_json::to_string_pretty(secrets)?;
    fs::write(&path, content)
}

pub fn generate_totp(secret_base32: &str) -> Result<(String, u64), String> {
    let clean_secret = secret_base32.chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>()
        .to_uppercase();

    let decoded = base32::decode(base32::Alphabet::RFC4648 { padding: false }, &clean_secret)
        .or_else(|| base32::decode(base32::Alphabet::RFC4648 { padding: true }, &clean_secret))
        .ok_or_else(|| "Base32 解碼失敗：密鑰格式不正確".to_string())?;

    let now = Utc::now().timestamp() as u64;
    let step = 30;
    let counter = now / step;
    let remaining = step - (now % step);

    let mut msg = [0u8; 8];
    for i in (0..8).rev() {
        msg[i] = (counter >> (8 * (7 - i))) as u8;
    }

    let mut mac = HmacSha1::new_from_slice(&decoded)
        .map_err(|_| "HMAC 初始化失敗".to_string())?;
    mac.update(&msg);
    let result = mac.finalize().into_bytes();

    let offset = (result[result.len() - 1] & 0x0f) as usize;
    let binary = (((result[offset] & 0x7f) as u32) << 24)
        | (((result[offset + 1]) as u32) << 16)
        | (((result[offset + 2]) as u32) << 8)
        | ((result[offset + 3]) as u32);

    let otp = binary % 1_000_000;
    let code = format!("{:06}", otp);

    Ok((code, remaining))
}

pub fn handle_totp_command(args: &[String], _verbose: bool) {
    let mut secrets = load_totp_secrets();

    let mut i = 2;
    while i < args.len() {
        let arg = &args[i];
        if arg == "--add" || arg == "-a" {
            if i + 1 >= args.len() {
                println!("❌ 錯誤：請指定標籤名稱 (label)。範例: a -t --add google --code \"...\"");
                return;
            }
            let label = args[i + 1].clone();
            let mut secret = String::new();

            if i + 2 < args.len() && (args[i + 2] == "--code" || args[i + 2] == "-c") {
                if i + 3 < args.len() {
                    secret = args[i + 3].clone();
                }
            } else if i + 2 < args.len() {
                secret = args[i + 2].clone();
            }

            if secret.is_empty() {
                println!("❌ 錯誤：請提供 TOTP 密鑰碼。範例: a -t --add google --code \"oufb d3w6...\"");
                return;
            }

            match generate_totp(&secret) {
                Ok(_) => {
                    secrets.insert(label.clone(), secret);
                    if let Err(e) = save_totp_secrets(&secrets) {
                        println!("❌ 儲存 TOTP 密鑰失敗: {}", e);
                    } else {
                        println!("✨ 成功新增/更新 TOTP 標籤: [{}]", label);
                    }
                }
                Err(e) => {
                    println!("❌ 無效的 TOTP Base32 密鑰: {}", e);
                }
            }
            return;
        } else if arg == "--delete" || arg == "--rm" || arg == "-d" {
            if i + 1 >= args.len() {
                println!("❌ 錯誤：請指定欲刪除的標籤名稱。範例: a -t --delete google");
                return;
            }
            let label = &args[i + 1];
            if secrets.remove(label).is_some() {
                if let Err(e) = save_totp_secrets(&secrets) {
                    println!("❌ 儲存失敗: {}", e);
                } else {
                    println!("🗑️ 已成功刪除 TOTP 標籤: [{}]", label);
                }
            } else {
                println!("⚠️ 找不到標籤 [{}]", label);
            }
            return;
        } else if arg == "--list" || arg == "-l" {
            print_all_totps(&secrets);
            return;
        } else if !arg.starts_with('-') {
            if let Some(secret) = secrets.get(arg) {
                match generate_totp(secret) {
                    Ok((code, remaining)) => {
                        println!("🔑 [{}] 驗證碼: {} (有效剩餘: {}s)", arg, code, remaining);
                    }
                    Err(e) => {
                        println!("❌ 計算 [{}] 驗證碼失敗: {}", arg, e);
                    }
                }
            } else {
                println!("❌ 找不到標籤 [{}]。請先使用 a -t --add {} --code \"...\" 新增。", arg, arg);
            }
            return;
        }
        i += 1;
    }

    print_all_totps(&secrets);
}

fn print_all_totps(secrets: &HashMap<String, String>) {
    if secrets.is_empty() {
        println!("📂 目前尚未儲存任何 TOTP 驗證密鑰。");
        println!("💡 新增範例: a -t --add google --code \"oufb d3w6 krma 7tcu dsaz cdis emey df5b\"");
        return;
    }

    println!("\n🛡️ Cyber-NOte TOTP 雙重認證驗證碼列表:");
    println!("------------------------------------------------------------");
    for (label, secret) in secrets {
        match generate_totp(secret) {
            Ok((code, remaining)) => {
                println!("  🔑 {:<16} : {}  (剩餘 {:2}s)", label, code, remaining);
            }
            Err(_) => {
                println!("  🔑 {:<16} : [金鑰解析錯誤]", label);
            }
        }
    }
    println!("------------------------------------------------------------");
    println!("💡 取得單一驗證碼: 'a -t [標籤名]' | 新增: 'a -t --add [標籤] --code [密鑰]'");
}
