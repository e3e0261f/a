// src/gist.rs
use std::time::Duration;
use reqwest::blocking::Client;
use serde_json::{json, Value};
use crate::GameConfig;

fn build_client() -> Client {
    Client::builder()
        .user_agent("Cyber-Forge-Client")
        .connect_timeout(Duration::from_secs(10)) // 10秒連線超時保護
        .timeout(Duration::from_secs(30))         // 30秒全域傳輸超時保護
        .build()
        .unwrap_or_else(|_| Client::new())
}

pub fn sync_to_gist(content: &str, file_name: &str, token: &str, verbose: bool) -> Result<(), String> {
    let client = build_client();
    let url = GameConfig::get_gist_url()?; 

    let safe_content = if content.is_empty() { "\n" } else { content };

    if verbose {
        println!("  📡 [網路] 正在向 {} 發送 PATCH 請求 (資料量: {} Bytes)...", url, safe_content.len());
    }

    let body = json!({
        "description": "Cyber-Forge 赛博灵感管家 自动云端加密备份法典",
        "files": {
            file_name: {
                "content": safe_content
            }
        }
    });

    let response = client.patch(&url)
        .header("Authorization", format!("Bearer {}", token))
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .json(&body)
        .send();

    match response {
        Ok(res) => {
            let status = res.status();
            if verbose {
                println!("  📥 [網路] 伺服器響應狀態碼: {}", status);
            }

            let response_text = res.text().unwrap_or_else(|_| "無法讀取雲端回傳文本".to_string());

            if status.is_success() {
                Ok(())
            } else {
                println!("🚨 [調試雷達] 雲端無情砸回錯誤！狀態碼: {}", status);
                println!("💬 [內部解密官方原文]：\n{}", response_text);
                Err(format!("❌ 雲端拒絕了貨物，狀態碼: {}", status))
            }
        },
        Err(e) => {
            if e.is_timeout() {
                Err("❌ 跨海連線超時：GitHub 連線緩慢或代理未通".to_string())
            } else {
                Err(format!("❌ 跨海管道斷裂: {}", e))
            }
        },
    }
}

pub fn fetch_from_gist(file_name: &str, token: &str, verbose: bool) -> Result<String, String> {
    let client = build_client();
    let url = GameConfig::get_gist_url()?;

    if verbose {
        println!("  📡 [網路] 正在向 {} 請求物資檔案【{}】...", url, file_name);
    }

    let response = client.get(&url)
        .header("Authorization", format!("Bearer {}", token))
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .send();

    match response {
        Ok(res) => {
            let status = res.status();
            if verbose {
                println!("  📥 [網路] 伺服器響應狀態碼: {}", status);
            }

            if !status.is_success() {
                return Err(format!("❌ 無法取得 Gist 內容，狀態碼: {}", status));
            }

            let text = res.text().map_err(|e| e.to_string())?;
            let json_val: Value = serde_json::from_str(&text).map_err(|e| format!("解析 JSON 失敗: {}", e))?;

            if let Some(content) = json_val["files"][file_name]["content"].as_str() {
                Ok(content.to_string())
            } else {
                Err(format!("❌ 在雲端 Gist 中找不到檔案: {}", file_name))
            }
        },
        Err(e) => {
            if e.is_timeout() {
                Err("❌ 下載請求超時：請檢查網路狀況".to_string())
            } else {
                Err(format!("❌ 聯絡雲端失敗: {}", e))
            }
        },
    }
}

pub fn list_gist_files(token: &str, verbose: bool) -> Result<Vec<String>, String> {
    let client = build_client();
    let url = GameConfig::get_gist_url()?;

    if verbose {
        println!("  📡 [網路] 正在向 {} 掃描倉庫清單...", url);
    }

    let response = client.get(&url)
        .header("Authorization", format!("Bearer {}", token))
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .send();

    match response {
        Ok(res) => {
            let status = res.status();
            if verbose {
                println!("  📥 [網路] 伺服器響應狀態碼: {}", status);
            }

            if !status.is_success() {
                return Err(format!("❌ 無法獲取雲端清單，狀態碼: {}", status));
            }

            let text = res.text().map_err(|e| e.to_string())?;
            let json_val: Value = serde_json::from_str(&text).map_err(|e| format!("解析 JSON 失敗: {}", e))?;

            if let Some(files_obj) = json_val["files"].as_object() {
                let file_list: Vec<String> = files_obj.keys().cloned().collect();
                Ok(file_list)
            } else {
                Err("❌ 解析 Gist 檔案架構失敗".to_string())
            }
        },
        Err(e) => {
            if e.is_timeout() {
                Err("❌ 掃描請求超時：請檢查網路連線".to_string())
            } else {
                Err(format!("❌ 聯絡雲端失敗: {}", e))
            }
        },
    }
}

// 🚀 創建全新 Gist 倉庫以完全銷毀、抹除既有修改歷史記錄 (Clean Slate History Purge)
pub fn create_clean_slate_gist(
    files: &std::collections::HashMap<String, String>,
    description: &str,
    token: &str,
    is_public: bool,
    verbose: bool,
) -> Result<String, String> {
    let client = build_client();
    let url = GameConfig::GIST_BASE_API;

    if verbose {
        println!("  📡 [網路] 正在向 {} 發起 POST 請求建立全新乾淨倉庫 (抹除歷史記錄)...", url);
    }

    let mut files_obj = serde_json::Map::new();
    for (name, content) in files {
        let mut file_inner = serde_json::Map::new();
        file_inner.insert("content".to_string(), Value::String(content.clone()));
        files_obj.insert(name.clone(), Value::Object(file_inner));
    }

    // 若為空，放置一個合規的占位索引檔案
    if files_obj.is_empty() {
        let mut file_inner = serde_json::Map::new();
        file_inner.insert(
            "content".to_string(),
            Value::String("Cyber-NOte 乾淨無痕加密保險庫已初始化 (修訂歷史已抹除)".to_string()),
        );
        files_obj.insert("cyber_note_vault.manifest".to_string(), Value::Object(file_inner));
    }

    let mut body_map = serde_json::Map::new();
    body_map.insert("description".to_string(), Value::String(description.to_string()));
    body_map.insert("public".to_string(), Value::Bool(is_public));
    body_map.insert("files".to_string(), Value::Object(files_obj));

    let body = Value::Object(body_map);

    let response = client.post(url)
        .header("Authorization", format!("Bearer {}", token))
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .json(&body)
        .send();

    match response {
        Ok(res) => {
            let status = res.status();
            if !status.is_success() {
                let text = res.text().unwrap_or_default();
                return Err(format!("❌ 建立新 Gist 倉庫失敗，狀態碼: {} (詳情: {})", status, text));
            }

            let text = res.text().map_err(|e| e.to_string())?;
            let json_val: Value = serde_json::from_str(&text).map_err(|e| format!("解析 JSON 失敗: {}", e))?;
            if let Some(new_id) = json_val["id"].as_str() {
                Ok(new_id.to_string())
            } else {
                Err("❌ 雲端未回傳有效的 Gist ID".to_string())
            }
        }
        Err(e) => Err(format!("❌ 連線 GitHub API 失敗: {}", e)),
    }
}

// 🗑️ 完全銷毀舊版 Gist 倉庫
pub fn delete_gist(gist_id: &str, token: &str, verbose: bool) -> Result<(), String> {
    let client = build_client();
    let url = format!("{}/{}", GameConfig::GIST_BASE_API, gist_id);

    if verbose {
        println!("  🗑️  [網路] 正在向 {} 發送 DELETE 請求銷毀舊倉庫...", url);
    }

    let response = client.delete(&url)
        .header("Authorization", format!("Bearer {}", token))
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .send();

    match response {
        Ok(res) => {
            let status = res.status();
            if status.is_success() || status.as_u16() == 204 {
                Ok(())
            } else {
                Err(format!("❌ 銷毀舊倉庫失敗，狀態碼: {}", status))
            }
        }
        Err(e) => Err(format!("❌ 發送刪除請求失敗: {}", e)),
    }
}

// 🔄 遠端 Gist 檔案原子套殼替換（發佈 new_file，並於遠端刪除 old_file）
pub fn atomic_replace_gist_file(
    old_file: Option<&str>,
    new_file: &str,
    new_content: &str,
    token: &str,
    verbose: bool,
) -> Result<(), String> {
    let client = build_client();
    let url = GameConfig::get_gist_url()?;

    if verbose {
        println!("  📡 [網路] 正在向 {} 發起原子套殼替換請求...", url);
        if let Some(old) = old_file {
            println!("  🗑️  將自遠端刪除原始檔案: {}，並發佈新密文檔案: {}", old, new_file);
        } else {
            println!("  📤 發佈新檔案: {}", new_file);
        }
    }

    let mut files_obj = serde_json::Map::new();
    let mut new_inner = serde_json::Map::new();
    new_inner.insert("content".to_string(), Value::String(new_content.to_string()));
    files_obj.insert(new_file.to_string(), Value::Object(new_inner));

    if let Some(old) = old_file {
        if old != new_file {
            files_obj.insert(old.to_string(), Value::Null);
        }
    }

    let mut body_map = serde_json::Map::new();
    body_map.insert("files".to_string(), Value::Object(files_obj));
    let body = Value::Object(body_map);

    let response = client.patch(&url)
        .header("Authorization", format!("Bearer {}", token))
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .json(&body)
        .send();

    match response {
        Ok(res) => {
            let status = res.status();
            if status.is_success() {
                Ok(())
            } else {
                let text = res.text().unwrap_or_default();
                Err(format!("❌ 遠端檔案替換失敗，狀態碼: {} (詳情: {})", status, text))
            }
        }
        Err(e) => Err(format!("❌ 連線 GitHub API 失敗: {}", e)),
    }
}

// 🗑️ 刪除 Gist 中的指定檔案
pub fn delete_gist_file(file_name: &str, token: &str, verbose: bool) -> Result<(), String> {
    let client = build_client();
    let url = GameConfig::get_gist_url()?;

    if verbose {
        println!("  🗑️  [網路] 正在向 {} 請求刪除檔案 {}...", url, file_name);
    }

    let mut files_obj = serde_json::Map::new();
    files_obj.insert(file_name.to_string(), Value::Null);
    let mut body_map = serde_json::Map::new();
    body_map.insert("files".to_string(), Value::Object(files_obj));
    let body = Value::Object(body_map);

    let response = client.patch(&url)
        .header("Authorization", format!("Bearer {}", token))
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .json(&body)
        .send();

    match response {
        Ok(res) => {
            let status = res.status();
            if status.is_success() {
                Ok(())
            } else {
                let text = res.text().unwrap_or_default();
                Err(format!("❌ 刪除雲端檔案失敗，狀態碼: {} (詳情: {})", status, text))
            }
        }
        Err(e) => Err(format!("❌ 發送刪除請求失敗: {}", e)),
    }
}
