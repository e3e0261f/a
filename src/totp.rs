use chrono::Utc;
use comfy_table::{presets::NOTHING, Cell, Color, Table};
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

    // 收集非旗標的位置參數
    let non_flags: Vec<String> = args.iter().skip(1).filter(|a| !a.starts_with('-')).cloned().collect();

    // 檢查是否有刪除旗標 (-f, -d, --delete)
    let is_delete = args.iter().any(|a| a == "-f" || a == "-d" || a == "--delete" || a == "--rm");

    if is_delete {
        if non_flags.is_empty() {
            println!("❌ 錯誤：請指定欲刪除的標籤名稱或編號。範例: a -t -f google");
            return;
        }
        let target = &non_flags[0];
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
    }

    // 1. a -t [標籤] [密鑰] -> 新增 TOTP 註冊驗證密鑰
    if non_flags.len() >= 2 {
        let label = non_flags[0].clone();
        let secret = non_flags[1..].join("");

        match generate_totp(&secret) {
            Ok(_) => {
                secrets.insert(label.clone(), secret);
                match save_totp_secrets(&secrets) {
                    Ok(_) => {
                        println!("✨ 成功新增/註冊 TOTP 標籤: [{}] (已加密儲存於本地筆記目錄，可透過 a -s 同步至雲端)", label);
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
    }

    // 2. a -t [標籤] -> 列印 6 位動態碼
    if non_flags.len() == 1 {
        let arg = &non_flags[0];
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
            println!("❌ 找不到標籤或編號 [{}]。請先使用 a -t 查看可用標籤清單。", arg);
        }
        return;
    }

    // 3. a -t -> 打印所有 TOTP 標籤
    print_totp_index_list(&secrets);
}

fn print_totp_index_list(secrets: &HashMap<String, String>) {
    if secrets.is_empty() {
        println!("📂 目前尚未儲存任何 TOTP 驗證密鑰。");
        println!("💡 新增範例: a -t google \"oufb d3w6 krma 7tcu dsaz cdis emey df5b\"");
        return;
    }

    let mut keys: Vec<String> = secrets.keys().cloned().collect();
    keys.sort();

    let mut table = Table::new();
    table.load_preset(NOTHING);

    for (idx, label) in keys.iter().enumerate() {
        table.add_row(vec![
            Cell::new(format!("[{:02}]", idx + 1)).fg(Color::Cyan),
            Cell::new(label).fg(Color::White),
        ]);
    }

    println!("\n🛡️  Cyber-NOte TOTP 雙重認證項目列表:");
    println!("{}", table);
    println!("💡 取得驗證碼: 'a -t [標籤]' (例: a -t google 或 a -t 1)");
    println!("💡 新增密鑰:   'a -t [標籤] [密鑰]'");
}
