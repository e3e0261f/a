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

    if verbose {
        println!("  📡 [網路] 正在向 {} 發送 PATCH 請求 (資料量: {} Bytes)...", url, content.len());
    }

    let body = json!({
        "description": "Cyber-Forge 赛博灵感管家 自动云端加密备份法典",
        "files": {
            file_name: {
                "content": content
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