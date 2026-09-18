use chrono::Utc;
use hmac::{Hmac, Mac};
use sha1::Sha1;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use crate::GameConfig;
use crate::encrypt::{encrypt_with_gpg, decrypt_with_gpg};

type HmacSha1 = Hmac<Sha1>;

pub fn get_totp_storage_path() -> PathBuf {
    GameConfig::get_note_dir().join("totp_secrets.json.gpg")
}

pub fn get_legacy_totp_storage_path() -> PathBuf {
    GameConfig::get_app_config_dir().join("totp_secrets.json")
}

pub fn load_totp_secrets() -> HashMap<String, String> {
    let path = get_totp_storage_path();
    let legacy_path = get_legacy_totp_storage_path();

    // 1. Try loading encrypted .gpg file from note_dir
    if path.exists() {
        if let Ok(ciphertext) = fs::read_to_string(&path) {
            if let Ok(json_str) = decrypt_with_gpg(&ciphertext) {
                if let Ok(map) = serde_json::from_str::<HashMap<String, String>>(&json_str) {
                    return map;
                }
            }
        }
    }

    // 2. Fallback to legacy unencrypted file if exists, and auto-encrypt it
    if legacy_path.exists() {
        if let Ok(content) = fs::read_to_string(&legacy_path) {
            if let Ok(map) = serde_json::from_str::<HashMap<String, String>>(&content) {
                let _ = save_totp_secrets(&map);
                let _ = fs::remove_file(&legacy_path);
                return map;
            }
        }
    }

    HashMap::new()
}

pub fn save_totp_secrets(secrets: &HashMap<String, String>) -> Result<(), String> {
    let path = get_totp_storage_path();
    let json_str = serde_json::to_string_pretty(secrets)
        .map_err(|e| format!("JSON 序列化失敗: {}", e))?;

    let gpg_user_id = GameConfig::get_gpg_user_id()
        .unwrap_or_else(|_| "".to_string());

    let encrypted_content = if !gpg_user_id.is_empty() {
        encrypt_with_gpg(json_str.as_bytes(), &gpg_user_id)?
    } else {
        // Fallback if no GPG key configured yet
        json_str
    };

    fs::write(&path, encrypted_content)
        .map_err(|e| format!("寫入加密 TOTP 檔案失敗: {}", e))?;

    Ok(())
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
            let mut label = String::new();
            let mut secret = String::new();

            let mut j = i + 1;
            while j < args.len() {
                let sub = &args[j];
                if sub == "--name" || sub == "-n" {
                    if j + 1 < args.len() {
                        label = args[j + 1].clone();
                        j += 2;
                        continue;
                    }
                } else if sub == "--code" || sub == "-c" {
                    if j + 1 < args.len() {
                        secret = args[j + 1].clone();
                        j += 2;
                        continue;
                    }
                } else if !sub.starts_with('-') {
                    if label.is_empty() {
                        label = sub.clone();
                    } else if secret.is_empty() {
                        secret = sub.clone();
                    }
                }
                j += 1;
            }

            if label.is_empty() {
                println!("❌ 錯誤：請指定標籤名稱 (label)。範例: a -t --add google --code \"...\" 或 a -t --add --name google --code \"...\"");
                return;
            }

            if secret.is_empty() {
                println!("❌ 錯誤：請提供 TOTP 密鑰碼。範例: a -t --add google --code \"oufb d3w6...\"");
                return;
            }

            match generate_totp(&secret) {
                Ok(_) => {
                    secrets.insert(label.clone(), secret);
                    match save_totp_secrets(&secrets) {
                        Ok(_) => {
                            println!("✨ 成功新增/更新 TOTP 標籤: [{}] (已加密儲存於本地筆記目錄，可透過 a -s 同步至雲端)", label);
                        }
                        Err(e) => {
                            println!("❌ 儲存 TOTP 密鑰失敗: {}", e);
                        }
                    }
                }
                Err(e) => {
                    println!("❌ 無效的 TOTP Base32 密鑰: {}", e);
                }
            }
            return;
        } else if arg == "--delete" || arg == "--rm" || arg == "-d" {
            if i + 1 >= args.len() {
                println!("❌ 錯誤：請指定欲刪除的標籤名稱或編號。範例: a -t --delete google");
                return;
            }
            let target = &args[i + 1];
            let mut keys: Vec<String> = secrets.keys().cloned().collect();
            keys.sort();

            let mut label_to_remove = target.clone();
            if let Ok(idx) = target.parse::<usize>() {
                if idx > 0 && idx <= keys.len() {
                    label_to_remove = keys[idx - 1].clone();
                }
            }

            if secrets.remove(&label_to_remove).is_some() {
                match save_totp_secrets(&secrets) {
                    Ok(_) => {
                        println!("🗑️ 已成功刪除 TOTP 標籤: [{}] (已同步更新加密存盤)", label_to_remove);
                    }
                    Err(e) => {
                        println!("❌ 儲存失敗: {}", e);
                    }
                }
            } else {
                println!("⚠️ 找不到標籤或編號 [{}]", target);
            }
            return;
        } else if arg == "--list" || arg == "-l" {
            print_totp_index_list(&secrets);
            return;
        } else if !arg.starts_with('-') {
            // Lookup by index or label
            let mut keys: Vec<String> = secrets.keys().cloned().collect();
            keys.sort();

            let mut matched_label = arg.clone();
            if let Ok(idx) = arg.parse::<usize>() {
                if idx > 0 && idx <= keys.len() {
                    matched_label = keys[idx - 1].clone();
                }
            }

            if let Some(secret) = secrets.get(&matched_label) {
                match generate_totp(secret) {
                    Ok((code, remaining)) => {
                        println!("🔑 [{}] 驗證碼: {} (有效剩餘: {}s)", matched_label, code, remaining);
                    }
                    Err(e) => {
                        println!("❌ 計算 [{}] 驗證碼失敗: {}", matched_label, e);
                    }
                }
            } else {
                println!("❌ 找不到標籤或編號 [{}]。請先使用 a -t --list 查看可用項目。", arg);
            }
            return;
        }
        i += 1;
    }

    print_totp_index_list(&secrets);
}

fn print_totp_index_list(secrets: &HashMap<String, String>) {
    if secrets.is_empty() {
        println!("📂 目前尚未儲存任何 TOTP 驗證密鑰。");
        println!("💡 新增範例: a -t --add google --code \"oufb d3w6 krma 7tcu dsaz cdis emey df5b\"");
        return;
    }

    let mut keys: Vec<String> = secrets.keys().cloned().collect();
    keys.sort();

    println!("\n🛡️ Cyber-NOte TOTP 雙重認證項目列表:");
    println!("------------------------------------------------------------");
    for (idx, label) in keys.iter().enumerate() {
        println!("  [{}] {}", idx + 1, label);
    }
    println!("------------------------------------------------------------");
    println!("💡 取得驗證碼: 'a -t [編號或標籤]' (例: a -t 1 或 a -t google)");
    println!("💡 新增密鑰:   'a -t --add [標籤] --code [密鑰]' 或 'a -t --add --name [標籤] --code [密鑰]'");
}
