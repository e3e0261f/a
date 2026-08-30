// src/lib.rs
use std::env;
use std::path::PathBuf;
use std::fs;

pub mod color;
pub mod storage;
pub mod gist;
pub mod encrypt;

pub struct GameConfig;

impl GameConfig {
    pub const GIST_BASE_API: &'static str = "https://api.github.com/gists";

    // 獲取應用程式全局設定目錄 (~/.config/a)
    pub fn get_app_config_dir() -> PathBuf {
        let home = env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let config_dir = PathBuf::from(home).join(".config").join("a");
        if !config_dir.exists() {
            let _ = fs::create_dir_all(&config_dir);
        }
        config_dir
    }

    // 📂 動態獲取筆記目錄 (支援: 1.環境變數 > 2.常駐設定 ~/.config/a/dir > 3.預設 ~/BOok/NOte)
    pub fn get_note_dir() -> PathBuf {
        if let Ok(custom) = env::var("A_NOTE_DIR") {
            return Self::expand_tilde(&custom);
        }

        // 讀取常駐設定檔
        let persistent_dir_file = Self::get_app_config_dir().join("dir");
        if let Ok(content) = fs::read_to_string(&persistent_dir_file) {
            let trimmed = content.trim();
            if !trimmed.is_empty() {
                let p = Self::expand_tilde(trimmed);
                if !p.exists() {
                    let _ = fs::create_dir_all(&p);
                }
                return p;
            }
        }

        let home = env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let dir = PathBuf::from(home).join("BOok").join("NOte");

        if !dir.exists() {
            let _ = fs::create_dir_all(&dir);
        }

        dir
    }

    // 💾 單獨持久化寫入新目錄
    pub fn set_persistent_dir(new_path: &str) -> std::io::Result<PathBuf> {
        let clean_path = Self::expand_tilde(new_path);
        if !clean_path.exists() {
            fs::create_dir_all(&clean_path)?;
        }
        let persistent_dir_file = Self::get_app_config_dir().join("dir");
        fs::write(&persistent_dir_file, clean_path.to_str().unwrap_or(new_path))?;
        Ok(clean_path)
    }

    // 展開路徑波浪號 (~)
    pub fn expand_tilde(path_str: &str) -> PathBuf {
        if path_str.starts_with("~/") || path_str == "~" {
            let home = env::var("HOME").unwrap_or_else(|_| ".".to_string());
            if path_str == "~" {
                PathBuf::from(home)
            } else {
                PathBuf::from(home).join(&path_str[2..])
            }
        } else {
            PathBuf::from(path_str)
        }
    }

    // 🔑 動態提領 GPG 金鑰指紋
    pub fn get_gpg_user_id() -> Result<String, String> {
        if let Ok(val) = env::var("A_GPG_KEY") {
            let trimmed = val.trim().to_string();
            if !trimmed.is_empty() {
                return Ok(trimmed);
            }
        }

        let note_dir = Self::get_note_dir();
        let key_file = note_dir.join("key_id");
        if let Ok(content) = fs::read_to_string(&key_file) {
            let trimmed = content.trim().to_string();
            if !trimmed.is_empty() {
                return Ok(trimmed);
            }
        }

        Err("未配置 GPG 金鑰指紋，請執行 'a --init' 進行配置。".to_string())
    }

    // 🌐 動態提領 Gist ID 並組裝標準 API 網址
    pub fn get_gist_url() -> Result<String, String> {
        let gist_id = Self::get_gist_id()?;
        Ok(format!("{}/{}", Self::GIST_BASE_API, gist_id))
    }

    // 🔍 智慧提領 Gist ID
    pub fn get_gist_id() -> Result<String, String> {
        if let Ok(val) = env::var("A_GIST_ID") {
            let trimmed = val.trim().to_string();
            if !trimmed.is_empty() {
                return Ok(Self::extract_clean_id(&trimmed));
            }
        }

        let note_dir = Self::get_note_dir();
        let id_file = note_dir.join("gist_id");
        if let Ok(content) = fs::read_to_string(&id_file) {
            let trimmed = content.trim().to_string();
            if !trimmed.is_empty() {
                return Ok(Self::extract_clean_id(&trimmed));
            }
        }

        let legacy_file = note_dir.join("gist_url");
        if let Ok(content) = fs::read_to_string(&legacy_file) {
            let trimmed = content.trim().to_string();
            if !trimmed.is_empty() {
                return Ok(Self::extract_clean_id(&trimmed));
            }
        }

        Err("未配置雲端 Gist ID，請執行 'a --init' 進行配置。".to_string())
    }

    // 🧹 萃取乾淨 ID
    pub fn extract_clean_id(raw_input: &str) -> String {
        raw_input
            .trim()
            .trim_end_matches('/')
            .rsplit('/')
            .next()
            .unwrap_or(raw_input)
            .trim()
            .to_string()
    }

    // 🛡️ 檢查是否已完成基礎配置
    pub fn is_configured() -> bool {
        let note_dir = Self::get_note_dir();
        let has_key = note_dir.join("key_id").exists() || env::var("A_GPG_KEY").is_ok();
        let has_gist = note_dir.join("gist_id").exists() || env::var("A_GIST_ID").is_ok();
        let has_token = note_dir.join("token.gpg").exists();
        has_key && has_gist && has_token
    }
}