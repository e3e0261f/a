// src/lib.rs
use std::env;
use std::path::PathBuf;
use std::fs;
use serde::{Deserialize, Serialize};

pub mod color;
pub mod storage;
pub mod gist;
pub mod encrypt;
pub mod ledger;
pub mod totp;

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub struct AppUnifiedConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note_dir: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gist_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub web_port: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub web_state: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_updated: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cached_commit_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cached_remote_files: Option<Vec<String>>,
}

pub struct GameConfig;

impl GameConfig {
    pub const GIST_BASE_API: &'static str = "https://api.github.com/gists";

    // 獲取應用程式全局設定目錄 (統一收斂至 ~/.local/share/cyber-note)
    pub fn get_app_config_dir() -> PathBuf {
        let home = env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let config_dir = PathBuf::from(&home).join(".local").join("share").join("cyber-note");
        if !config_dir.exists() {
            let _ = fs::create_dir_all(&config_dir);
        }
        config_dir
    }

    // 統一設定檔路徑 (~/.local/share/cyber-note/config.json)
    pub fn get_unified_config_path() -> PathBuf {
        Self::get_app_config_dir().join("config.json")
    }

    // 讀取統一設定檔 (支援從 ~/.local/share/cyber-note/ 及舊目錄 ~/.config/cyber-note 自動無縫遷移)
    pub fn read_unified_config() -> AppUnifiedConfig {
        let path = Self::get_unified_config_path();
        if path.exists() {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(cfg) = serde_json::from_str::<AppUnifiedConfig>(&content) {
                    return cfg;
                }
            }
        }

        let mut cfg = AppUnifiedConfig::default();
        let home = env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let local_share = Self::get_app_config_dir();
        let legacy_config = PathBuf::from(&home).join(".config").join("cyber-note");
        let legacy_a = PathBuf::from(&home).join(".config").join("a");

        // 遷移讀取 note_dir
        if let Ok(d) = fs::read_to_string(local_share.join("dir")) {
            cfg.note_dir = Some(d.trim().to_string());
        } else if let Ok(d) = fs::read_to_string(legacy_config.join("dir")) {
            cfg.note_dir = Some(d.trim().to_string());
        } else if let Ok(d) = fs::read_to_string(legacy_a.join("dir")) {
            cfg.note_dir = Some(d.trim().to_string());
        }

        // 遷移讀取 key_id
        if let Ok(k) = fs::read_to_string(local_share.join("key_id")) {
            cfg.key_id = Some(k.trim().to_string());
        } else if let Ok(k) = fs::read_to_string(legacy_config.join("key_id")) {
            cfg.key_id = Some(k.trim().to_string());
        } else if let Ok(k) = fs::read_to_string(legacy_a.join("key_id")) {
            cfg.key_id = Some(k.trim().to_string());
        }

        // 遷移讀取 gist_id
        if let Ok(g) = fs::read_to_string(local_share.join("gist_id")) {
            cfg.gist_id = Some(g.trim().to_string());
        } else if let Ok(g) = fs::read_to_string(legacy_config.join("gist_id")) {
            cfg.gist_id = Some(g.trim().to_string());
        } else if let Ok(g) = fs::read_to_string(legacy_a.join("gist_id")) {
            cfg.gist_id = Some(g.trim().to_string());
        }

        cfg
    }

    // 寫入統一設定檔 (~/.local/share/cyber-note/config.json 與相容檔)
    pub fn write_unified_config(cfg: &AppUnifiedConfig) -> std::io::Result<()> {
        let app_dir = Self::get_app_config_dir();
        let path = app_dir.join("config.json");
        let content = serde_json::to_string_pretty(cfg)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        fs::write(&path, content)?;

        // 同步寫入單一相容檔，保持與腳本及外部工具完全相容
        if let Some(ref d) = cfg.note_dir {
            let _ = fs::write(app_dir.join("dir"), d.trim());
        }
        if let Some(ref k) = cfg.key_id {
            let _ = fs::write(app_dir.join("key_id"), k.trim());
        }
        if let Some(ref g) = cfg.gist_id {
            let _ = fs::write(app_dir.join("gist_id"), g.trim());
        }
        Ok(())
    }

    // 🛡️ 隱私數據隔離目錄 (~/.local/share/cyber-note/secrets)
    // 專門存儲 token.gpg 等機密憑證，嚴禁明文落盤，目錄權限設為 0700
    pub fn get_secrets_dir() -> PathBuf {
        let secrets_dir = Self::get_app_config_dir().join("secrets");
        if !secrets_dir.exists() {
            let _ = fs::create_dir_all(&secrets_dir);
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = fs::set_permissions(&secrets_dir, fs::Permissions::from_mode(0o700));
            }
        }

        // 若舊位置存在 token.gpg，自動無縫遷移至新目錄
        let home = env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let legacy_token = PathBuf::from(&home).join(".config").join("cyber-note").join("secrets").join("token.gpg");
        let new_token = secrets_dir.join("token.gpg");
        if legacy_token.exists() && !new_token.exists() {
            let _ = fs::copy(&legacy_token, &new_token);
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = fs::set_permissions(&new_token, fs::Permissions::from_mode(0o600));
            }
        }

        secrets_dir
    }

    // 📂 動態獲取筆記目錄 (符合 Linux FHS / XDG 規範，每台 Linux 均預設存在的標準目錄: ~/.local/share/cyber-note/notes)
    pub fn get_note_dir() -> PathBuf {
        if let Ok(custom) = env::var("A_NOTE_DIR") {
            return Self::expand_tilde(&custom);
        }

        // 1. 優先從統一設定檔讀取 (~/.local/share/cyber-note/config.json)
        let unified = Self::read_unified_config();
        if let Some(ref d) = unified.note_dir {
            let trimmed = d.trim();
            if !trimmed.is_empty() {
                let p = Self::expand_tilde(trimmed);
                if !p.exists() {
                    let _ = fs::create_dir_all(&p);
                }
                return p;
            }
        }

        // 2. 讀取常駐設定檔 (~/.local/share/cyber-note/dir)
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

        // 3. 檢查舊版 config 目錄中的 dir
        let legacy_dir_file = PathBuf::from(env::var("HOME").unwrap_or_else(|_| ".".to_string()))
            .join(".config")
            .join("cyber-note")
            .join("dir");
        if let Ok(content) = fs::read_to_string(&legacy_dir_file) {
            let trimmed = content.trim();
            if !trimmed.is_empty() {
                let p = Self::expand_tilde(trimmed);
                if !p.exists() {
                    let _ = fs::create_dir_all(&p);
                }
                return p;
            }
        }

        // 4. Linux 標準 XDG 資料目錄: ~/.local/share/cyber-note/notes
        let home = env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let dir = PathBuf::from(home)
            .join(".local")
            .join("share")
            .join("cyber-note")
            .join("notes");

        if !dir.exists() {
            let _ = fs::create_dir_all(&dir);
        }

        dir
    }

    // 💾 單獨持久化寫入新目錄 (同步寫入 ~/.local/share/cyber-note/config.json)
    pub fn set_persistent_dir(new_path: &str) -> std::io::Result<PathBuf> {
        let clean_path = Self::expand_tilde(new_path);
        if !clean_path.exists() {
            fs::create_dir_all(&clean_path)?;
        }
        let mut unified = Self::read_unified_config();
        let path_str = clean_path.to_str().unwrap_or(new_path).to_string();
        unified.note_dir = Some(path_str.clone());
        let _ = Self::write_unified_config(&unified);

        let persistent_dir_file = Self::get_app_config_dir().join("dir");
        let _ = fs::write(&persistent_dir_file, &path_str);
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

    // 🔑 動態提領並驗證 GPG 金鑰指紋（嚴格禁止 SSH 金鑰）
    pub fn get_gpg_user_id() -> Result<String, String> {
        if let Ok(val) = env::var("A_GPG_KEY") {
            let trimmed = val.trim().to_string();
            if !trimmed.is_empty() {
                crate::encrypt::validate_gpg_key_not_ssh(&trimmed)?;
                return Ok(trimmed);
            }
        }

        // 1. 檢查 ~/.local/share/cyber-note/key_id
        let share_key = Self::get_app_config_dir().join("key_id");
        if let Ok(content) = fs::read_to_string(&share_key) {
            let trimmed = content.trim().to_string();
            if !trimmed.is_empty() {
                crate::encrypt::validate_gpg_key_not_ssh(&trimmed)?;
                return Ok(trimmed);
            }
        }

        // 2. 檢查 note_dir/key_id
        let note_dir = Self::get_note_dir();
        let key_file = note_dir.join("key_id");
        if let Ok(content) = fs::read_to_string(&key_file) {
            let trimmed = content.trim().to_string();
            if !trimmed.is_empty() {
                crate::encrypt::validate_gpg_key_not_ssh(&trimmed)?;
                return Ok(trimmed);
            }
        }

        // 3. 從統一設定檔讀取 (~/.local/share/cyber-note/config.json)
        let unified = Self::read_unified_config();
        if let Some(ref k) = unified.key_id {
            let trimmed = k.trim().to_string();
            if !trimmed.is_empty() {
                crate::encrypt::validate_gpg_key_not_ssh(&trimmed)?;
                return Ok(trimmed);
            }
        }

        // 4. 檢查舊版 ~/.config/cyber-note/key_id
        let home = env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let legacy_key = PathBuf::from(&home).join(".config").join("cyber-note").join("key_id");
        if let Ok(content) = fs::read_to_string(&legacy_key) {
            let trimmed = content.trim().to_string();
            if !trimmed.is_empty() {
                crate::encrypt::validate_gpg_key_not_ssh(&trimmed)?;
                return Ok(trimmed);
            }
        }

        Err("未配置 GPG 金鑰，請執行 'a --init' 進行配置。".to_string())
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

        // 1. 檢查 ~/.local/share/cyber-note/gist_id
        let share_id = Self::get_app_config_dir().join("gist_id");
        if let Ok(content) = fs::read_to_string(&share_id) {
            let trimmed = content.trim().to_string();
            if !trimmed.is_empty() {
                return Ok(Self::extract_clean_id(&trimmed));
            }
        }

        // 2. 檢查 note_dir/gist_id
        let note_dir = Self::get_note_dir();
        let id_file = note_dir.join("gist_id");
        if let Ok(content) = fs::read_to_string(&id_file) {
            let trimmed = content.trim().to_string();
            if !trimmed.is_empty() {
                return Ok(Self::extract_clean_id(&trimmed));
            }
        }

        // 3. 從統一設定檔讀取 (~/.local/share/cyber-note/config.json)
        let unified = Self::read_unified_config();
        if let Some(ref g) = unified.gist_id {
            let trimmed = g.trim().to_string();
            if !trimmed.is_empty() {
                return Ok(Self::extract_clean_id(&trimmed));
            }
        }

        // 4. 檢查舊版 ~/.config/cyber-note/gist_id
        let home = env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let legacy_gist = PathBuf::from(&home).join(".config").join("cyber-note").join("gist_id");
        if let Ok(content) = fs::read_to_string(&legacy_gist) {
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
        let has_key = note_dir.join("key_id").exists()
            || Self::get_app_config_dir().join("key_id").exists()
            || Self::get_gpg_user_id().is_ok()
            || env::var("A_GPG_KEY").is_ok();
        let has_gist = note_dir.join("gist_id").exists()
            || Self::get_app_config_dir().join("gist_id").exists()
            || Self::get_gist_id().is_ok()
            || env::var("A_GIST_ID").is_ok();
        let has_token = note_dir.join("token.gpg").exists()
            || Self::get_secrets_dir().join("token.gpg").exists();
        has_key && has_gist && has_token
    }
}
