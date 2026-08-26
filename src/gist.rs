// src/gist.rs
use reqwest::blocking::Client;
use serde_json::json;

// 🚢 諸侯引渡
use crate::GameConfig;

pub fn sync_to_gist(content: &str, file_name: &str, token: &str) -> Result<(), String> {
    // 🏛️ 聽總工程師的命令：不繞彎、不走代理！用最純淨、最合法的正規軍身分直接出海！
    let client = Client::builder()
        .user_agent("Cyber-Forge-Client")
        .build()
        .unwrap_or_else(|_| Client::new());

    let url = GameConfig::GIST_URL; 

    // 📦 毫無瑕疵的完美官方 JSON 貨物（只更新當前檔案，其他年份的檔案會安全地在雲端並存）
    let body = json!({
        "description": "Cyber-Forge 赛博灵感管家 自动云端加密备份法典",
        "files": {
            file_name: {
                "content": content
            }
        }
    });

    // 🚀 跨海大炮直接發射！帶上 GitHub 規定的合約標頭
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