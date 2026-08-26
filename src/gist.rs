// src/gist.rs
use reqwest::blocking::Client;
use serde_json::{json, Value};
use crate::GameConfig;

pub fn sync_to_gist(content: &str, file_name: &str, token: &str) -> Result<(), String> {
    let client = Client::builder()
        .user_agent("Cyber-Forge-Client")
        .build()
        .unwrap_or_else(|_| Client::new());

    let url = GameConfig::GIST_URL; 

    let body = json!({
        "description": "Cyber-Forge 赛博灵感管家 自动云端加密备份法典",
        "files": {
            file_name: {
                "content": content
            }
        }
    });

    let response = client.patch(url)
        .header("Authorization", format!("Bearer {}", token))
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .json(&body)
        .send();

    match response {
        Ok(res) => {
            let status = res.status();
            let response_text = res.text().unwrap_or_else(|_| "無法讀取雲端回傳文本".to_string());

            if status.is_success() {
                Ok(())
            } else {
                println!("🚨 [調試雷達] 雲端無情砸回錯誤！狀態碼: {}", status);
                println!("💬 [內部解密官方原文]：\n{}", response_text);
                Err(format!("❌ 雲端拒絕了貨物，狀態碼: {}", status))
            }
        },
        Err(e) => Err(format!("❌ 跨海管道斷裂: {}", e)),
    }
}

// 🌐 新增：從雲端 Gist 抓取特定檔案內容的公式
pub fn fetch_from_gist(file_name: &str, token: &str) -> Result<String, String> {
    let client = Client::builder()
        .user_agent("Cyber-Forge-Client")
        .build()
        .unwrap_or_else(|_| Client::new());

    let url = GameConfig::GIST_URL;

    let response = client.get(url)
        .header("Authorization", format!("Bearer {}", token))
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .send();

    match response {
        Ok(res) => {
            if !res.status().is_success() {
                return Err(format!("❌ 無法取得 Gist 內容，狀態碼: {}", res.status()));
            }

            let text = res.text().map_err(|e| e.to_string())?;
            let json_val: Value = serde_json::from_str(&text).map_err(|e| format!("解析 JSON 失敗: {}", e))?;

            // 從 JSON 結構中尋找 files -> file_name -> content
            if let Some(content) = json_val["files"][file_name]["content"].as_str() {
                Ok(content.to_string())
            } else {
                Err(format!("❌ 在雲端 Gist 中找不到檔案: {}", file_name))
            }
        },
        Err(e) => Err(format!("❌ 聯絡雲端失敗: {}", e)),
    }
}