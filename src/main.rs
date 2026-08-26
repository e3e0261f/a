// src/main.rs
use std::env;
use std::process::Command;
use chrono::Local;

// 🚢 諸侯引渡
use a::{GameConfig, color::{paint_line, TerminalColor}, storage::{write_encrypted_note, read_note}};
use a::encrypt::{encrypt_with_gpg, decrypt_with_gpg};
use a::gist::{sync_to_gist, fetch_from_gist};

// 🔐 向系統 Secret Service（KeePassXC）安全索取 GitHub Token
fn get_github_token() -> Result<String, String> {
    let output = Command::new("secret-tool")
        .arg("lookup")
        .arg("title")
        .arg("GITHUB_GIST_TOKEN") // 對應 KeePassXC 中的標題
        .output()
        .map_err(|e| format!("無法調用 secret-tool: {}", e))?;

    if output.status.success() {
        let token = String::from_utf8(output.stdout)
            .map_err(|e| format!("Token 編碼錯誤: {}", e))?
            .trim()
            .to_string();
        
        if token.is_empty() {
            Err("KeePassXC 條目內容為空".to_string())
        } else {
            Ok(token)
        }
    } else {
        Err("無法透過 Secret Service 取得密鑰，請確認 KeePassXC 是否已解鎖並啟用整合".to_string())
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let current_year = Local::now().format("%Y").to_string();
    let file_path = format!("{}/{}.note.gpg", GameConfig::NOTE_DIR, current_year);

    if args.len() < 2 {
        println!("用法: a [您的靈感創意/支援多行貼上] #自動解密拼接並整檔公鑰加密");
        println!("      a -a 或 a --all           #解密並列印今年全部本地內容");
        println!("      a -s 或 a --sync          #從 KeePassXC 提領 Token 並推送至雲端");
        println!("      a -d [年份] 或 --download [年份] #從雲端下載指定年份筆記");
        return;
    }

    // ✨ 1. 查看本地全部筆記
    if args[1] == "-a" || args[1] == "--all" {
        if let Ok(encrypted_full_content) = read_note(&file_path) { 
            match decrypt_with_gpg(&encrypted_full_content) {
                Ok(decrypted_content) => {
                    for (index, line) in decrypted_content.lines().enumerate() {
                        if index % 2 == 0 {
                            paint_line(line, TerminalColor::Green);
                        } else {
                            paint_line(line, TerminalColor::Cyan);
                        }
                    }
                },
                Err(e) => println!("⚠️  [保密局] 解密失敗（可能需要輸入私鑰密碼）: {}", e),
            }
        } else {
            println!("📂 今年還沒有任何靈感記錄哦！");
        }
        return;
    }

    // ✨ 2. 雲端同步（自動從 KeePassXC 提領 Token）
    if args[1] == "-s" || args[1] == "--sync" {
        if let Ok(encrypted_content) = read_note(&file_path) {
            match get_github_token() {
                Ok(token) => {
                    println!("🔄 [主動同步] 已安全取得 KeePassXC 通行證，正在將密文包裹發射至 GitHub Gist...");
                    let file_name = format!("{}.note.gpg", current_year);
                    
                    match sync_to_gist(&encrypted_content, &file_name, &token) {
                        Ok(_) => println!("☁️  [GitHub] 雲端備份同步完美打通！"),
                        Err(e) => println!("⚠️  [GitHub] 傳輸失敗: {}", e),
                    }
                },
                Err(e) => println!("❌ 錯誤：{}", e),
            }
        } else {
            println!("📂 本地空空如也，沒有什麼好同步的。");
        }
        return;
    }

    // ✨ 3. 雲端下載
    if args[1] == "-d" || args[1] == "--download" {
        let target_year = if args.len() > 2 { args[2].clone() } else { current_year.clone() };
        let file_name = format!("{}.note.gpg", target_year);
        let target_local_path = format!("{}/{}", GameConfig::NOTE_DIR, file_name);

        match get_github_token() {
            Ok(token) => {
                println!("☁️  [雲端雷達] 正在下載【{}】...", file_name);
                match fetch_from_gist(&file_name, &token) {
                    Ok(encrypted_content) => {
                        if std::fs::write(&target_local_path, encrypted_content).is_ok() {
                            println!("✨ 雲端密文已成功同步覆蓋至本地廠房：{}", target_local_path);
                        } else {
                            println!("⚠️ 寫入本地硬碟失敗");
                        }
                    },
                    Err(e) => println!("⚠️ 下載失敗: {}", e),
                }
            },
            Err(e) => println!("❌ 錯誤：{}", e),
        }
        return;
    }

    // ✨ 4. 核心：寫入新筆記（解密舊檔 ➔ 拼接 ➔ 公鑰整檔加密 ➔ 覆蓋本地）
    let new_note = args[1..].join(" ");
    
    let mut existing_content = String::new();
    if let Ok(encrypted_old) = read_note(&file_path) {
        if let Ok(decrypted_old) = decrypt_with_gpg(&encrypted_old) {
            existing_content = decrypted_old;
        } else {
            println!("⚠️  [保密局] 警告：無法解密舊筆記，操作終止以防數據丟失。");
            return;
        }
    }

    if !existing_content.is_empty() && !existing_content.ends_with('\n') {
        existing_content.push('\n');
    }
    existing_content.push_str(&new_note);

    match encrypt_with_gpg(&existing_content, GameConfig::GPG_USER_ID) {
        Ok(new_encrypted_block) => {
            if write_encrypted_note(&file_path, &new_encrypted_block).is_ok() {
                println!("✨ 靈感已安全縫合並以【單一GPG密文包裹】加密封存於本地 {} 廠房！", current_year);
            }
        },
        Err(e) => println!("⚠️ 全局公鑰加密失敗: {}", e),
    }
}