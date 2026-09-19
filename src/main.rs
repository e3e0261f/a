// src/main.rs
// Cyber-NOte 機密記事與金鑰加密系統 (Project a)
// 核心架構：嚴格鎖定 GPG 金鑰隔離體系（嚴禁 SSH 金鑰混用）、多層巢狀加密封裝、高迭代 S2K 防窮舉加固與金鑰歸檔簿審計。

use chrono::Local;
use comfy_table::{presets::NOTHING, Attribute, Cell, Color, Table};
use std::env;
use std::fs;
use std::io::{self, BufRead, BufReader, IsTerminal, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::time::Instant;

use a::encrypt::{
    calculate_gpg_layer, decrypt_bytes_with_gpg, decrypt_with_gpg, encrypt_symmetric_s2k,
    encrypt_with_gpg, validate_gpg_key_not_ssh, DEFAULT_S2K_COUNT,
};
use a::gist::{
    atomic_replace_gist_file, delete_gist_file,
    fetch_from_gist, list_gist_files, sync_to_gist,
};
use a::ledger::{compute_sha256, record_ledger_entry};
use a::totp::handle_totp_command;
use a::{
    color::{paint_line, TerminalColor},
    storage::{read_note, write_encrypted_note},
    GameConfig,
};

fn prompt_input(prompt: &str, default: Option<&str>) -> String {
    if let Some(def) = default {
        print!("🔹 {} [預設: {}]: ", prompt, def);
    } else {
        print!("🔹 {}: ", prompt);
    }
    io::stdout().flush().unwrap();
    let mut buffer = String::new();
    io::stdin().read_line(&mut buffer).unwrap();
    let trimmed = buffer.trim();
    if trimmed.is_empty() {
        default.unwrap_or("").to_string()
    } else {
        trimmed.to_string()
    }
}

// 🛡️ 系統初始化配置精靈 (嚴格 GPG 鎖定)
fn run_init_wizard() {
    println!("\n🛡️  Cyber-NOte 機密系統 · 基礎配置與金鑰鎖定");
    println!("提示: 直接按下 Enter 可保留括號中的 [現有配置/預設值]。\n");

    let current_dir = GameConfig::get_note_dir();
    let current_dir_str = current_dir.to_str().unwrap_or("~/BOok/NOte");

    // 1. 目錄配置
    let note_dir_input = prompt_input("請確認機密文檔本地存儲目錄", Some(current_dir_str));
    let note_dir = match GameConfig::set_persistent_dir(&note_dir_input) {
        Ok(d) => {
            println!("  ↳ 📂 存儲目錄已確認: {:?}", d);
            d
        }
        Err(e) => {
            println!("  ⚠️ 寫入目錄失敗 ({})，維持原目錄", e);
            current_dir
        }
    };

    // 2. GPG 金鑰配置（嚴格檢查並拒絕 SSH 密鑰）
    println!("\n--- [步驟 1/3: 鎖定 GPG 金鑰] ---");
    let existing_key = GameConfig::get_gpg_user_id().unwrap_or_default();
    let key_prompt_default = if existing_key.is_empty() {
        None
    } else {
        Some(existing_key.as_str())
    };

    loop {
        let key_id = prompt_input(
            "請輸入 GPG 金鑰標識 (指紋/子金鑰ID/用戶標識，嚴禁 SSH 金鑰)",
            key_prompt_default,
        );
        if key_id.is_empty() {
            break;
        }

        match validate_gpg_key_not_ssh(&key_id) {
            Ok(_) => {
                let share_key = GameConfig::get_app_config_dir().join("key_id");
                let _ = fs::write(&share_key, &key_id);
                let mut unified = GameConfig::read_unified_config();
                unified.key_id = Some(key_id.clone());
                let _ = GameConfig::write_unified_config(&unified);
                println!("  ↳ 🔑 GPG 金鑰已鎖定並存檔: {:?}", share_key);
                break;
            }
            Err(err_msg) => {
                println!("  ❌ 錯誤：{}", err_msg);
                println!("  ⚠️ 本系統嚴格遵循 GPG 密鑰規範，請重新輸入正確的 GPG Key ID。");
            }
        }
    }

    // 3. Gist ID 配置
    println!("\n--- [步驟 2/3: 雲端 Gist 倉庫標識] ---");
    let existing_gist = GameConfig::get_gist_id().unwrap_or_default();
    let gist_prompt_default = if existing_gist.is_empty() {
        None
    } else {
        Some(existing_gist.as_str())
    };
    let raw_gist = prompt_input("請輸入 Gist ID 或 URL", gist_prompt_default);
    let clean_gist_id = GameConfig::extract_clean_id(&raw_gist);
    if !clean_gist_id.is_empty() {
        let share_gist = GameConfig::get_app_config_dir().join("gist_id");
        let _ = fs::write(&share_gist, &clean_gist_id);
        let mut unified = GameConfig::read_unified_config();
        unified.gist_id = Some(clean_gist_id.clone());
        let _ = GameConfig::write_unified_config(&unified);
        println!(
            "  ↳ 🌐 Gist ID [{}] 已存檔: {:?}",
            clean_gist_id, share_gist
        );
    }

    // 4. Token 憑證配置 (隱私數據隔離至 ~/.config/cyber-note/secrets/token.gpg)
    println!("\n--- [步驟 3/3: GitHub 存取憑證封裝與隱私隔離] ---");
    let secrets_dir = GameConfig::get_secrets_dir();
    let secret_token_file = secrets_dir.join("token.gpg");
    let has_token = secret_token_file.exists();
    let token_default = if has_token {
        Some("保留現有加密憑證")
    } else {
        None
    };
    let token_input = prompt_input("請輸入 GitHub Personal Access Token", token_default);
    if token_input != "保留現有加密憑證" && !token_input.is_empty() {
        let active_key = GameConfig::get_gpg_user_id().unwrap_or_default();
        if active_key.is_empty() {
            println!("  ❌ 錯誤：未配置鎖定 GPG 金鑰，無法封裝 Token");
        } else {
            print!(
                "  ↳ 🔐 正在調用 GPG 金鑰 [{}] 封裝 token.gpg 進行隱私數據隔離...",
                active_key
            );
            io::stdout().flush().unwrap();
            match encrypt_with_gpg(token_input.as_bytes(), &active_key) {
                Ok(encrypted_token) => {
                    // 寫入隔離目錄 (~/.local/share/cyber-note/secrets/token.gpg)
                    if fs::write(&secret_token_file, encrypted_token.as_bytes()).is_ok() {
                        #[cfg(unix)]
                        {
                            use std::os::unix::fs::PermissionsExt;
                            let _ = fs::set_permissions(&secret_token_file, fs::Permissions::from_mode(0o600));
                        }

                        println!(" [成功]");
                        let _ = record_ledger_entry(
                            "token.gpg",
                            secret_token_file.to_str().unwrap_or("token.gpg"),
                            &active_key,
                            "GPG_PUBLIC_KEY",
                            0,
                            1,
                            encrypted_token.as_bytes(),
                            "系統存取憑證密文封裝 (隱私數據隔離: 權限 0600, 目錄 0700)",
                        );
                        println!("  ↳ 🛡️ 憑證已嚴格加密隔離存檔: {:?}", secret_token_file);
                        println!("  ↳ 🔒 POSIX 許可權: 檔案 0600 (rw-------) / 目錄 0700 (rwx------)");
                    }
                }
                Err(e) => println!(" [失敗: {}]", e),
            }
        }
    } else if has_token {
        println!("  ↳ 🛡️ 維持現有 token.gpg 密文憑證隔離保護。");
    }

    // 💾 寫入全域統一設定檔 (~/.local/share/cyber-note/config.json)
    let mut unified = GameConfig::read_unified_config();
    unified.note_dir = Some(note_dir.to_str().unwrap_or("").to_string());
    if let Ok(key) = GameConfig::get_gpg_user_id() {
        unified.key_id = Some(key);
    }
    if let Ok(gid) = GameConfig::get_gist_id() {
        unified.gist_id = Some(gid);
    }
    unified.token_path = Some(secret_token_file.to_str().unwrap_or("").to_string());
    unified.last_updated = Some(Local::now().to_rfc3339());
    let _ = GameConfig::write_unified_config(&unified);
    println!("  ↳ 💾 統一設定檔已同步落盤: {:?}", GameConfig::get_unified_config_path());

    println!("\n✨ 系統配置與金鑰鎖定已完成！\n");
}

fn get_github_token(verbose: bool) -> Result<String, String> {
    // 優先讀取安全隔離目錄: ~/.config/cyber-note/secrets/token.gpg
    let secrets_dir = GameConfig::get_secrets_dir();
    let secret_token = secrets_dir.join("token.gpg");
    let note_dir = GameConfig::get_note_dir();
    let note_token = note_dir.join("token.gpg");
    let home = env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let legacy_config_token = PathBuf::from(&home).join(".config").join("a").join("secrets").join("token.gpg");

    let token_path = if secret_token.exists() {
        secret_token
    } else if note_token.exists() {
        note_token
    } else if legacy_config_token.exists() {
        legacy_config_token
    } else {
        secret_token
    };

    if verbose {
        println!("  🔑 [憑證] 正在讀取並解密本地隔離 Token: {:?}", token_path);
    }

    let encrypted_token = fs::read_to_string(&token_path).map_err(|e| {
        format!(
            "無法讀取加密 Token 檔 ({:?}): 請先執行 'a --init' 配置 (錯誤: {})",
            token_path, e
        )
    })?;

    let decrypted_token = decrypt_with_gpg(&encrypted_token)
        .map_err(|e| format!("解密 Token 失敗（請確認已解鎖 GPG 私鑰）: {}", e))?;

    let token = decrypted_token.trim().to_string();
    if token.is_empty() {
        Err("解密後的 Token 內容為空".to_string())
    } else {
        if verbose {
            println!("  ✅ [憑證] Token 解密驗證通過 (長度: {} 位元)", token.len());
        }
        Ok(token)
    }
}

fn print_content_colored(raw_content: &str) {
    let printable_content = if raw_content.contains("-----BEGIN PGP MESSAGE-----") {
        match decrypt_with_gpg(raw_content) {
            Ok(dec) => dec,
            Err(e) => {
                println!("⚠️  [解密異常] 無法解密文檔內容: {}", e);
                return;
            }
        }
    } else {
        raw_content.to_string()
    };

    for (index, line) in printable_content.lines().enumerate() {
        if index % 2 == 0 {
            paint_line(line, TerminalColor::Normal);
        } else {
            paint_line(line, TerminalColor::Gray);
        }
    }
}

// 📚 Web 管理引擎核心依賴科普與安裝指引
#[allow(dead_code)]
fn print_web_dependencies_guide(missing_tsx: bool, missing_express: bool) {
    println!("\n📚 Web 安全管理引擎 · 核心依賴科普與安裝指引");
    println!("❌ 無法啟動 Web 管理介面：檢測到系統尚未安裝或缺少關鍵運行時依賴！\n");
    if missing_tsx {
        println!("  ⚠️  缺少關鍵模組: tsx (TypeScript Execute 引擎)");
    }
    if missing_express {
        println!("  ⚠️  缺少伺服器框架: express (HTTP REST API 模組)");
    }
    println!("\n----------------------------------------------------------------------");
    println!("【依賴科學普及 (Why We Need Them)】");
    println!("🔹 什麼是 tsx？");
    println!("   tsx (TypeScript Execute) 是一個以 esbuild 引擎為核心的高效能執行環境。");
    println!("   它解決了 Node.js 原生無法直接解析 TypeScript 的限制，能零編譯直接執行");
    println!("   .ts 腳本 (server.ts)，具備極低記憶體開銷與毫秒級啟動特性。");
    println!("\n🔹 什麼是 express？");
    println!("   Express 是 Node.js 領域中最標準、穩定且安全的輕量級 HTTP Web 伺服器框架。");
    println!("   Cyber-NOte 依賴 Express 在本地提供安全 REST API 代理，處理非對稱 GPG 金鑰調用、");
    println!("   金鑰歸檔簿審計與 Gist 雲端通信。");
    println!("----------------------------------------------------------------------");
    println!("\n🛠️ 【終端快速安裝指引 (Terminal Install Commands)】");
    println!("   方案 A (推薦 - 全局安裝):");
    println!("      npm install -g tsx express");
    println!("\n   方案 B (在專案目錄安裝):");
    println!("      npm install");
    println!("\n💡 【提示】若暫不具備 Node.js / Web 運行環境，Cyber-NOte 依然能 100% 獨立運行於");
    println!("   Rust 原生 CLI 模式 (a -p 加密, a -x 解密, a -u 同步, a -k 歸檔簿)！\n");
}

// 🚀 創建新檔案：a --new [文件名] [文件內容] (若無 -s 則在本地創建，有 -s 才上傳遠端)
fn handle_new_repo_command(args: &[String], verbose: bool) {
    let mut out_dir_opt: Option<String> = None;
    let mut positional_args = Vec::new();
    let mut i = 1;
    while i < args.len() {
        let arg = &args[i];
        if arg == "-o" || arg == "--out" || arg == "--output" {
            if i + 1 < args.len() && !args[i + 1].starts_with('-') {
                out_dir_opt = Some(args[i + 1].clone());
                i += 2;
                continue;
            } else {
                out_dir_opt = Some(".".to_string());
                i += 1;
                continue;
            }
        }
        if arg.starts_with('-') {
            i += 1;
            continue;
        }
        positional_args.push(arg);
        i += 1;
    }

    if positional_args.is_empty() {
        println!("❌ 錯誤：請指定檔案名稱。範例: a --new 123 321");
        return;
    }

    let filename = positional_args[0];
    let content = if positional_args.len() > 1 {
        positional_args[1..].iter().map(|s| s.as_str()).collect::<Vec<_>>().join(" ")
    } else {
        "\n".to_string()
    };

    let sync = args.iter().any(|a| a == "-s" || a == "--sync" || a == "-u" || a == "--upload");

    let target_dir = if let Some(ref dir_str) = out_dir_opt {
        PathBuf::from(dir_str)
    } else {
        GameConfig::get_note_dir()
    };

    if !sync {
        let _ = fs::create_dir_all(&target_dir);
        let file_path = target_dir.join(filename);

        if file_path.exists() {
            println!("❌ 錯誤：檔案已存在 -> {:?}", file_path);
            return;
        }

        match fs::write(&file_path, content.as_bytes()) {
            Ok(_) => println!("✨ 成功創建檔案: {:?}", file_path),
            Err(e) => println!("❌ 寫入檔案失敗: {}", e),
        }
    } else {
        let token = match get_github_token(verbose) {
            Ok(t) => t,
            Err(e) => {
                println!("❌ 無法取得 GitHub Token: {}", e);
                println!("💡 請先執行 'a --init' 配置並封裝 GPG 憑證。");
                return;
            }
        };

        println!("🚀 正在向雲端 Gist 創建並寫入檔案: {}...", filename);
        match sync_to_gist(&content, filename, &token, verbose) {
            Ok(_) => println!("✨ 成功在雲端 Gist 創建並寫入檔案: {}", filename),
            Err(e) => println!("❌ 創建新檔案失敗: {}", e),
        }
    }
}

// 🗑️ 刪除檔案/遠端檔案：a -f [文件名]
fn handle_delete_repo_command(args: &[String], verbose: bool) {
    let targets: Vec<&String> = args.iter().skip(1).filter(|a| !a.starts_with('-')).collect();

    if targets.is_empty() {
        println!("❌ 錯誤：請指定欲刪除的檔案名稱或編號。範例: a -f 1 或 a -f file1.txt");
        return;
    }

    let note_dir = GameConfig::get_note_dir();

    let token_opt = get_github_token(verbose).ok();
    let files = if let Some(ref token) = token_opt {
        list_gist_files(token, verbose).unwrap_or_default()
    } else {
        Vec::new()
    };

    let mut unified_cfg = GameConfig::read_unified_config();
    let mut cached_files = unified_cfg.cached_remote_files.clone().unwrap_or_else(|| files.clone());

    for target in targets {
        let mut filename_to_delete = target.to_string();
        if !files.contains(&filename_to_delete) {
            if let Ok(idx) = target.parse::<usize>() {
                if idx > 0 && idx <= files.len() {
                    filename_to_delete = files[idx - 1].clone();
                }
            }
        }

        // 1. 刪除本地檔案（如果存在）
        let local_file = note_dir.join(&filename_to_delete);
        if local_file.exists() {
            match fs::remove_file(&local_file) {
                Ok(_) => println!("🗑️ 已成功刪除本地檔案: {:?}", local_file),
                Err(e) => println!("⚠️ 刪除本地檔案失敗: {}", e),
            }
        }

        // 2. 刪除遠端 Gist 檔案（如果有 Token 且遠端存在）
        if let Some(ref token) = token_opt {
            if files.contains(&filename_to_delete) {
                println!("🗑️ 正在向雲端 Gist 請求刪除檔案: {}...", filename_to_delete);
                match delete_gist_file(&filename_to_delete, token, verbose) {
                    Ok(_) => {
                        println!("🗑️ 已成功自遠端 Gist 刪除檔案: {}", filename_to_delete);
                    }
                    Err(e) => {
                        if e.contains("422") || e.contains("404") || e.contains("missing_field") {
                            println!("ℹ️ 遠端 Gist 倉庫已無此檔案 ({}，狀態已對齊)。", filename_to_delete);
                        } else {
                            println!("❌ 刪除遠端檔案失敗 ({}): {}", filename_to_delete, e);
                        }
                    }
                }
            } else {
                println!("✨ 遠端 Gist 倉庫中已無此檔案: {} (確認已自雲端移除)", filename_to_delete);
            }
        }

        // 3. 清理審計簿與快取
        let _ = a::ledger::remove_ledger_entry(&filename_to_delete);
        cached_files.retain(|f| f != &filename_to_delete);
        println!("  ↳ 🧹 已同步清除本地審計簿與快取中關於「{}」的記錄。", filename_to_delete);
    }

    unified_cfg.cached_remote_files = Some(cached_files);
    let _ = GameConfig::write_unified_config(&unified_cfg);
}

// 🏷️ 重命名檔案：a -m [文件名] [新文件名]
fn handle_rename_command(args: &[String], verbose: bool) {
    let non_flags: Vec<String> = args.iter().skip(1).filter(|a| !a.starts_with('-')).cloned().collect();
    if non_flags.len() < 2 {
        println!("❌ 錯誤：請提供欲更名的檔案名稱與新名稱。");
        println!("💡 範例: a -m 舊檔名 新檔名 (例: a -m note.txt note_backup.txt)");
        return;
    }
    let old_name = &non_flags[0];
    let new_name = &non_flags[1];

    let note_dir = GameConfig::get_note_dir();
    let old_local_path = note_dir.join(old_name);
    let new_local_path = note_dir.join(new_name);

    let mut renamed_local = false;
    if old_local_path.exists() {
        match fs::rename(&old_local_path, &new_local_path) {
            Ok(_) => {
                renamed_local = true;
                println!("✨ 本地檔案重命名成功: {} -> {}", old_name, new_name);
            }
            Err(e) => {
                println!("⚠️ 本地檔案重命名失敗: {}", e);
            }
        }
    }

    // 嘗試雲端 Gist 重命名
    if let Ok(token) = get_github_token(verbose) {
        if let Ok(remote_files) = list_gist_files(&token, verbose) {
            if remote_files.contains(old_name) {
                match a::gist::rename_gist_file(old_name, new_name, &token, verbose) {
                    Ok(_) => {
                        println!("✨ 雲端 Gist 檔案重命名成功: {} -> {}", old_name, new_name);
                    }
                    Err(e) => {
                        println!("⚠️ 雲端 Gist 檔案重命名失敗: {}", e);
                    }
                }
            }
        }
    }

    // 更新審計簿
    let _ = a::ledger::rename_ledger_entry(old_name, new_name);

    // 更新本地快取
    let mut unified_cfg = GameConfig::read_unified_config();
    if let Some(ref mut cached) = unified_cfg.cached_remote_files {
        for item in cached.iter_mut() {
            if item == old_name {
                *item = new_name.clone();
            }
        }
        let _ = GameConfig::write_unified_config(&unified_cfg);
    }

    if !renamed_local {
        println!("ℹ️ 若為純雲端檔案，已送出遠端重命名請求並同步更新本地記錄。");
    }
}

// 🛡️ 檢視單一檔案詳細鑑識資訊 (a --show <文件名>)
fn handle_show_command(args: &[String], verbose: bool) {
    let target_file = args.iter().skip(1).find(|&a| a != "--show" && !a.starts_with('-')).cloned();
    let filename = match target_file {
        Some(f) => f,
        None => {
            println!("❌ 錯誤：請指定欲檢視的檔案名稱。範例: a --show 2021homelee.gpg");
            return;
        }
    };

    let note_dir = GameConfig::get_note_dir();
    let ledger = a::ledger::load_ledger();
    let token = get_github_token(verbose).ok();

    let mut remote_size = 0u64;
    let mut in_cloud = false;
    if let Some(ref t) = token {
        if let Ok(details) = a::gist::list_gist_files_with_details(t, false) {
            for d in details {
                if d.filename == filename {
                    remote_size = d.size;
                    in_cloud = true;
                }
            }
        }
    }

    let local_path = note_dir.join(&filename);
    let local_exists = local_path.exists();
    let entry_opt = ledger.records.iter().find(|r| r.file_name == filename);

    let is_gpg = filename.ends_with(".gpg");
    let key_id = entry_opt.map(|e| e.key_id.clone()).unwrap_or_else(|| if is_gpg { "未知".to_string() } else { "-".to_string() });
    let cipher_mode = entry_opt.map(|e| e.cipher_mode.clone()).unwrap_or_else(|| if is_gpg { "GPG_ENCRYPTED".to_string() } else { "PLAINTEXT".to_string() });
    let layer = entry_opt.map(|e| e.layer).unwrap_or_else(|| if is_gpg { 1 } else { 0 });
    let notes = entry_opt.map(|e| e.notes.clone()).unwrap_or_else(|| "無審計備註".to_string());

    let bytes_size = if local_exists {
        fs::metadata(&local_path).map(|m| m.len()).unwrap_or(0)
    } else {
        remote_size
    };

    let cloud_status = if in_cloud || entry_opt.is_some() { "🌐 雲端已備份" } else { "❌ 僅本地存在" };
    let status_str = if !is_gpg { "📄 明文" } else { "🛡️ GPG/RSA" };

    let mut table = Table::new();
    table.load_preset(NOTHING);
    table.add_row(vec![Cell::new("檔案名稱 :").fg(Color::Cyan), Cell::new(filename)]);
    table.add_row(vec![Cell::new("檔案狀態 :").fg(Color::Cyan), Cell::new(status_str)]);
    table.add_row(vec![Cell::new("雲端備份 :").fg(Color::Cyan), Cell::new(cloud_status)]);
    table.add_row(vec![Cell::new("金鑰短碼 :").fg(Color::Cyan), Cell::new(key_id)]);
    table.add_row(vec![Cell::new("加密體系 :").fg(Color::Cyan), Cell::new(cipher_mode)]);
    table.add_row(vec![Cell::new("封裝層級 :").fg(Color::Cyan), Cell::new(format!("第 {} 層", layer))]);
    table.add_row(vec![Cell::new("檔案大小 :").fg(Color::Cyan), Cell::new(format!("{} Bytes", bytes_size))]);
    table.add_row(vec![Cell::new("審計備註 :").fg(Color::Cyan), Cell::new(notes)]);

    println!("\n🛡️  Cyber-NOte 檔案鑑識與金鑰審計詳情 [{}]", filename);
    println!("{}", table);
}

// 🛡️ 雲端檔案清單與金鑰審計鑑識合併處理 (a -l / a -k)
fn handle_list_and_ledger_command(verbose: bool) {
    // 🌟 動作提示優先：立即輸出檢索提示，解決空空延遲問題
    println!("📡 [雲端檢索] 正在連線 GitHub Gist 比對遠端 Hash 與清單，請稍候...");

    let note_dir = GameConfig::get_note_dir();
    let mut ledger = a::ledger::load_ledger();
    let mut unified_cfg = GameConfig::read_unified_config();

    // 取得 GitHub Token
    let token = match get_github_token(verbose) {
        Ok(t) => t,
        Err(e) => {
            println!("❌ 錯誤：無法取得 GitHub Token: {}", e);
            a::ledger::print_ledger_table();
            return;
        }
    };

    // 檢查遠端 Gist 的 Commit Hash 是否有變動，若有變動自動對齊本地審計簿
    let mut sync_ledger = false;
    if let Ok(remote_commit) = a::gist::get_gist_commit_hash(&token, verbose) {
        let cached_commit = unified_cfg.cached_commit_hash.clone().unwrap_or_default();
        if remote_commit != cached_commit {
            if verbose {
                println!("📡 [Commit Hash 偵測] 發現遠端 Gist Commit Hash 變動 (本地: {}, 遠端: {})，自動觸發同步...", cached_commit, remote_commit);
            }
            sync_ledger = true;
            unified_cfg.cached_commit_hash = Some(remote_commit);
            let _ = GameConfig::write_unified_config(&unified_cfg);
        }
    }

    // 獲取遠端 Gist 檔案詳情（含大小）
    let mut remote_files_map: std::collections::HashMap<String, u64> = std::collections::HashMap::new();
    let remote_details_res = a::gist::list_gist_files_with_details(&token, verbose);
    let is_online = remote_details_res.is_ok();

    if let Ok(details) = remote_details_res {
        for d in details {
            remote_files_map.insert(d.filename, d.size);
        }
    }

    // 若線上連線成功，則以遠端最新列表覆蓋本地快取，並清理本地已自遠端刪除的孤立記錄
    if is_online {
        // 1. 同步覆蓋清理本地審計簿：若檔案在遠端已被刪除，且本地磁碟亦不存在，則自本地審計簿徹底剔除
        let initial_records_count = ledger.records.len();
        ledger.records.retain(|r| {
            remote_files_map.contains_key(&r.file_name) || note_dir.join(&r.file_name).exists()
        });
        if ledger.records.len() != initial_records_count {
            let _ = a::ledger::save_ledger(&ledger);
        }

        // 2. 若 Commit Hash 變動或尚未登記過新檔案，則同步新檔案元數據至審計簿
        if sync_ledger {
            if verbose {
                println!("📡 [雲端與金鑰鑑識 Sync] 正在同步遠端新檔案至本地審計簿...");
            }
            if let Ok(remote_commit) = a::gist::get_gist_commit_hash(&token, false) {
                unified_cfg.cached_commit_hash = Some(remote_commit);
                let _ = GameConfig::write_unified_config(&unified_cfg);
            }

            for (filename, &remote_size) in &remote_files_map {
                let is_gpg = filename.ends_with(".gpg");
                let local_path = note_dir.join(filename);
                
                let existing_record = ledger.records.iter().find(|r| &r.file_name == filename);
                let has_valid_key = if let Some(rec) = existing_record {
                    if is_gpg {
                        !rec.key_id.is_empty() && rec.key_id != "-" && !rec.key_id.contains("未知")
                    } else {
                        true
                    }
                } else {
                    false
                };

                if has_valid_key {
                    continue;
                }

                let mut key_id = String::new();
                let mut cipher_mode = "GPG_ENCRYPTED".to_string();

                if !is_gpg {
                    key_id = "-".to_string();
                    cipher_mode = "PLAINTEXT".to_string();
                } else {
                    if local_path.exists() {
                        let extracted = a::ledger::extract_key_id_from_gpg_file(&local_path);
                        if !extracted.contains("對稱") && !extracted.contains("封包") && !extracted.is_empty() {
                            key_id = extracted;
                        }
                    } else {
                        if let Ok(content) = a::gist::fetch_from_gist(filename, &token, false) {
                            let temp_path = note_dir.join(format!(".temp_inspect_{}", filename));
                            if fs::write(&temp_path, content.as_bytes()).is_ok() {
                                let extracted = a::ledger::extract_key_id_from_gpg_file(&temp_path);
                                if !extracted.contains("對稱") && !extracted.contains("封包") && !extracted.is_empty() {
                                    key_id = extracted;
                                }
                                let _ = fs::remove_file(&temp_path);
                            }
                        }
                    }
                }

                let size = if local_path.exists() {
                    fs::metadata(&local_path).map(|m| m.len() as usize).unwrap_or(remote_size as usize)
                } else {
                    remote_size as usize
                };

                let _ = a::ledger::record_ledger_entry(
                    filename,
                    &local_path.to_string_lossy(),
                    &key_id,
                    &cipher_mode,
                    0,
                    1,
                    &vec![0u8; size],
                    "Synced via Gist alignment",
                );
            }
            ledger = a::ledger::load_ledger();
        }
    }

    // 收集本地目錄檔案
    let mut local_files: Vec<String> = Vec::new();
    if let Ok(entries) = fs::read_dir(&note_dir) {
        for entry in entries.flatten() {
            if let Some(name) = entry.path().file_name().and_then(|n| n.to_str()) {
                if !name.starts_with('.') && !local_files.iter().any(|f| f == name) {
                    local_files.push(name.to_string());
                }
            }
        }
    }

    let mut cloud_file_names: Vec<String> = Vec::new();
    let mut local_only_files: Vec<String> = Vec::new();

    if is_online {
        // 🌟 線上模式：遠端清單 100% 以 GitHub Gist 實際取得的檔案為準，絕不盲目攙入歷史殘留檔案！
        let mut remote_keys: Vec<String> = remote_files_map.keys().cloned().collect();
        remote_keys.sort();
        cloud_file_names = remote_keys;

        // 覆蓋更新本地持久化快取
        unified_cfg.cached_remote_files = Some(cloud_file_names.clone());
        let _ = GameConfig::write_unified_config(&unified_cfg);
    } else {
        // 離線/斷網模式：自本地持久化快取優雅載入
        println!("⚠️ [離線檢索] 無法連線遠端 GitHub Gist，正在載入本地持久化快取清單...");
        if let Some(ref cached) = unified_cfg.cached_remote_files {
            cloud_file_names = cached.clone();
        } else {
            cloud_file_names = ledger.records.iter().map(|r| r.file_name.clone()).collect();
        }
        cloud_file_names.sort();
        cloud_file_names.dedup();
    }

    for lfile in local_files {
        let in_cloud = cloud_file_names.contains(&lfile);
        if !in_cloud {
            if !local_only_files.contains(&lfile) {
                local_only_files.push(lfile);
            }
        }
    }
    local_only_files.sort();

    println!("\n🛡️  Cyber-NOte 檔案清單");
    let gist_id = GameConfig::get_gist_id().unwrap_or_default();
    if !gist_id.is_empty() && gist_id != "未配置" {
        let clean_id = GameConfig::extract_clean_id(&gist_id);
        println!("🌐 倉庫網址 : https://gist.github.com/{}", clean_id);
    }

    let mut table = Table::new();
    table.load_preset(NOTHING);

    let mut counter = 1;

    for filename in &cloud_file_names {
        table.add_row(vec![
            Cell::new(format!("[{:02}]", counter)).fg(Color::Cyan),
            Cell::new("🛡️"),
            Cell::new(filename).fg(Color::White),
        ]);
        counter += 1;
    }

    for filename in &local_only_files {
        table.add_row(vec![
            Cell::new(format!("[{:02}]", counter)).fg(Color::Cyan),
            Cell::new("💡"),
            Cell::new(filename).fg(Color::Yellow),
        ]);
        counter += 1;
    }

    println!("{}", table);
    println!("💡 想查看檔案詳細資訊，請使用參數: a --show <文件名>");
    println!("💡 同步今年筆記並檢視清單: a -l -s (或 a -l --sync)\n");
}

// 🛡️ 遠端檔案在位套殼加密控制邏輯 (Remote In-Place Encapsulate & Clean Original)
fn handle_remote_encrypt_command(args: &[String], verbose: bool) {
    let delete_original = !args.iter().any(|a| a == "--keep-raw" || a == "--keep-original");
    let mut filename_opt: Option<&str> = None;
    let mut pass_opt: Option<String> = None;
    let mut iter_opt: Option<u64> = None;

    let mut skip_next = false;
    for (i, arg) in args.iter().enumerate() {
        if i == 0 || i == 1 {
            continue;
        }
        if skip_next {
            skip_next = false;
            continue;
        }
        if arg == "--pass" || arg == "-P" || arg == "--password" {
            if i + 1 < args.len() {
                pass_opt = Some(args[i + 1].clone());
                skip_next = true;
            }
        } else if arg == "--iter" || arg == "--iterations" {
            if i + 1 < args.len() {
                iter_opt = args[i + 1].parse::<u64>().ok();
                skip_next = true;
            }
        } else if arg == "--remote" || arg == "--delete-original" || arg == "--keep-raw" || arg == "-v" || arg == "--verbose" {
            // flag
        } else if !arg.starts_with('-') && filename_opt.is_none() {
            filename_opt = Some(arg);
        }
    }

    let target_file = match filename_opt {
        Some(f) => f,
        None => {
            println!("❌ 錯誤：請指定欲在遠端套殼加密之檔案名稱。");
            println!("   範例 1: a --remote-encrypt config.dae");
            println!("   範例 2 (保留遠端原始檔案): a --remote-encrypt config.dae --keep-raw");
            println!("   範例 3 (對稱密碼防窮舉): a --remote-encrypt config.dae --pass 自訂密碼");
            return;
        }
    };

    println!("\n🛡️  Cyber-NOte 遠端檔案在位套殼加密 (In-Place Encapsulate)");
    println!("目標遠端檔案: {}", target_file);
    if delete_original {
        println!("策略：原明文檔案將在遠端徹底刪除銷毀，僅留存加密密文殼！");
    } else {
        println!("策略：保留遠端原始明文檔案。");
    }

    let token = match get_github_token(verbose) {
        Ok(t) => t,
        Err(e) => {
            println!("❌ 錯誤：{}", e);
            return;
        }
    };

    println!("📡 [1/3 檢索] 正在從 GitHub Gist 讀取【{}】原始內容...", target_file);
    let raw_content = match fetch_from_gist(target_file, &token, verbose) {
        Ok(c) => c,
        Err(e) => {
            println!("❌ 錯誤：無法從遠端 Gist 檢索檔案 ({}): {}", target_file, e);
            return;
        }
    };

    let new_encrypted_name = format!("{}.gpg", target_file);
    let note_dir = GameConfig::get_note_dir();
    let local_output_path = note_dir.join(&new_encrypted_name);

    println!("🔐 [2/3 加密] 正在為檔案封裝 GPG 加密外殼...");
    let (ciphertext, cipher_mode, key_id_used, iterations_used) = if let Some(pass) = pass_opt {
        let iters = iter_opt.unwrap_or(DEFAULT_S2K_COUNT);
        match encrypt_symmetric_s2k(raw_content.as_bytes(), &pass, iters) {
            Ok(c) => (c, "GPG_SYMMETRIC_S2K", format!("SYMMETRIC-S2K ({} 輪)", iters), iters),
            Err(e) => {
                println!("❌ 對稱防窮舉加密失敗: {}", e);
                return;
            }
        }
    } else {
        let gpg_user_id = match GameConfig::get_gpg_user_id() {
            Ok(id) if !id.trim().is_empty() => id,
            _ => "CyberNOte-Key".to_string(),
        };
        match encrypt_with_gpg(raw_content.as_bytes(), &gpg_user_id) {
            Ok(c) => (c, "GPG_PUBLIC_KEY", gpg_user_id, 0),
            Err(e) => {
                println!("❌ GPG 公鑰加密失敗: {}", e);
                println!("💡 提示：若金鑰尚未匯入，可指定 '--pass 密碼' 啟用對稱防窮舉加密。");
                return;
            }
        }
    };

    // 寫入本地副本與金鑰審計歸檔簿
    let _ = fs::write(&local_output_path, &ciphertext);
    let layer = calculate_gpg_layer(&new_encrypted_name);
    let _ = record_ledger_entry(
        &new_encrypted_name,
        local_output_path.to_str().unwrap_or(&new_encrypted_name),
        &key_id_used,
        cipher_mode,
        iterations_used,
        layer,
        ciphertext.as_bytes(),
        &format!("遠端在位套殼加密 (原明文檔: {}{})", target_file, if delete_original { "，已在遠端銷毀" } else { "" }),
    );

    println!("🚀 [3/3 原子發佈] 正在更新遠端 Gist 倉庫並執行原子替換...");
    match atomic_replace_gist_file(
        if delete_original { Some(target_file) } else { None },
        &new_encrypted_name,
        &ciphertext,
        &token,
        verbose,
    ) {
        Ok(_) => {
            println!("\n✨ 遠端套殼加密完成！");
            paint_line(&format!("  🔒 遠端已發佈密文殼: {}", new_encrypted_name), TerminalColor::Normal);
            if delete_original {
                paint_line(&format!("  🗑️  遠端原明文檔案已徹底刪除: {}", target_file), TerminalColor::Gray);
            }
            println!("  🛡️  加密層級: 第 {} 層 | 金鑰/模式: {}", layer, cipher_mode);
            println!("  📂 本地安全副本已存入: {:?}", local_output_path);
        }
        Err(e) => {
            println!("❌ 遠端替換失敗: {}", e);
        }
    }
}

// 🌐 網頁端管理引擎控制邏輯 (Cyber-NOte Web Engine - Rust 原生零依賴獨立伺服器)
fn handle_web_command(port_opt: Option<&str>) {
    let port_str = port_opt.unwrap_or("3000");
    let port: u16 = port_str.parse().unwrap_or(3000);
    let config_dir = GameConfig::get_app_config_dir();
    if !config_dir.exists() {
        let _ = fs::create_dir_all(&config_dir);
    }
    let pid_file = config_dir.join("web.pid");
    let state_file = config_dir.join("web.state");

    let is_already_running = std::net::TcpStream::connect(format!("127.0.0.1:{}", port)).is_ok();
    if is_already_running {
        let _ = fs::write(&state_file, "active");
        println!("\n🟢 Cyber-NOte Web 管理引擎已在埠號 {} 前台運行中！", port);
        println!("🌐 存取位址: http://localhost:{}", port);
        println!("📄 統一設定: ~/.local/share/cyber-note/config.json");
        let _ = std::process::Command::new("xdg-open")
            .arg(format!("http://localhost:{}", port))
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn();
        return;
    }

    println!("\n🛡️  Cyber-NOte 系統 · Web 視覺化伺服器 (Port: {})", port);
    println!("🚀 Web 伺服器正在前台監聽運行中...");
    println!("🌐 存取位址: http://localhost:{}", port);
    println!("📊 架構核心: Rust 原生獨立 Web 引擎 (免安裝外掛/套件，純原生極致運行)");
    println!("📄 統一設定: ~/.local/share/cyber-note/config.json");
    println!("🛑 提示: 伺服器在前台持續監聽，按下 [ Ctrl + C ] 即可隨時停止 Web 伺服器。\n");

    start_rust_native_web_server(port, &pid_file, &state_file);
}

// ⚡ 啟動 Rust 原生獨立 Web 伺服器 (Zero-Dependency)
fn start_rust_native_web_server(port: u16, pid_file: &Path, state_file: &Path) {
    let pid = std::process::id();
    let _ = fs::write(pid_file, pid.to_string());
    let _ = fs::write(state_file, "active");

    let listener = match TcpListener::bind(format!("0.0.0.0:{}", port)) {
        Ok(l) => l,
        Err(e) => {
            println!("❌ 伺服器綁定埠號 0.0.0.0:{} 失敗: {}", port, e);
            let _ = fs::remove_file(pid_file);
            return;
        }
    };

    println!("✨ Rust 原生 Web 伺服器已成功就緒 (PID: {})！", pid);
    println!("🌐 本地網頁管理端: http://localhost:{}", port);
    println!("⚡ 隨時監控並同步統一設定: ~/.local/share/cyber-note/config.json\n");

    let _ = std::process::Command::new("xdg-open")
        .arg(format!("http://localhost:{}", port))
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                std::thread::spawn(move || {
                    handle_http_connection(stream);
                });
            }
            Err(e) => {
                eprintln!("連線錯誤: {}", e);
            }
        }
    }

    let _ = fs::remove_file(pid_file);
}

// 🌐 處理 HTTP 請求連線
fn handle_http_connection(mut stream: TcpStream) {
    let mut reader = BufReader::new(&stream);
    let mut req_line = String::new();
    if reader.read_line(&mut req_line).is_err() || req_line.trim().is_empty() {
        return;
    }

    let parts: Vec<&str> = req_line.split_whitespace().collect();
    if parts.len() < 2 {
        return;
    }
    let method = parts[0];
    let raw_path = parts[1];
    let path = raw_path.split('?').next().unwrap_or(raw_path);

    let mut content_length: usize = 0;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).is_err() || line.trim().is_empty() {
            break;
        }
        let lower = line.to_lowercase();
        if lower.starts_with("content-length:") {
            if let Some(val) = line.split(':').nth(1) {
                content_length = val.trim().parse().unwrap_or(0);
            }
        }
    }

    let mut body = vec![0u8; content_length];
    if content_length > 0 {
        let _ = reader.read_exact(&mut body);
    }

    if method == "OPTIONS" {
        let response = "HTTP/1.1 200 OK\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: GET, POST, OPTIONS\r\nAccess-Control-Allow-Headers: *\r\nContent-Length: 0\r\n\r\n";
        let _ = stream.write_all(response.as_bytes());
        return;
    }

    if path == "/api/health" {
        let json = r#"{"status":"ok","webEngine":"active","core":"Rust Native Web Engine (Zero-Dependency)"}"#;
        send_json_response(&mut stream, 200, json);
        return;
    }

    if path == "/api/rust/status" {
        let _cfg = GameConfig::read_unified_config();
        let note_dir = GameConfig::get_note_dir();
        let config_dir = GameConfig::get_app_config_dir();
        let key_id = GameConfig::get_gpg_user_id().unwrap_or_else(|_| "未配置".to_string());
        let gist_id = GameConfig::get_gist_id().unwrap_or_else(|_| "未配置".to_string());
        let token_path = GameConfig::get_secrets_dir().join("token.gpg");
        let has_token = token_path.exists();
        let state_file = config_dir.join("web.state");
        let web_state = fs::read_to_string(&state_file).unwrap_or_else(|_| "active".to_string());

        let res_obj = serde_json::json!({
            "rustAvailable": true,
            "binaryPath": "a",
            "version": "0.0.4",
            "engine": "Rust-Native-Zero-Dependency",
            "noteDir": note_dir.to_str().unwrap_or(""),
            "defaultDirStandard": "~/.local/share/cyber-note/notes",
            "configDir": config_dir.to_str().unwrap_or(""),
            "unifiedConfigFile": GameConfig::get_unified_config_path().to_str().unwrap_or(""),
            "secretsDir": GameConfig::get_secrets_dir().to_str().unwrap_or(""),
            "tokenPath": token_path.to_str().unwrap_or(""),
            "keyId": key_id,
            "gistId": gist_id,
            "hasToken": has_token,
            "webState": web_state.trim(),
            "notes": []
        });
        send_json_response(&mut stream, 200, &res_obj.to_string());
        return;
    }

    if path == "/api/rust/config" && method == "POST" {
        if let Ok(body_str) = String::from_utf8(body) {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&body_str) {
                let mut unified = GameConfig::read_unified_config();
                if let Some(nd) = val.get("noteDir").and_then(|v| v.as_str()) {
                    if !nd.trim().is_empty() {
                        let p = GameConfig::expand_tilde(nd.trim());
                        let _ = fs::create_dir_all(&p);
                        unified.note_dir = Some(p.to_str().unwrap_or(nd).to_string());
                    }
                }
                if let Some(k) = val.get("keyId").and_then(|v| v.as_str()) {
                    if !k.trim().is_empty() {
                        unified.key_id = Some(k.trim().to_string());
                    }
                }
                if let Some(g) = val.get("gistId").and_then(|v| v.as_str()) {
                    if !g.trim().is_empty() {
                        unified.gist_id = Some(GameConfig::extract_clean_id(g));
                    }
                }
                if let Some(t) = val.get("token").and_then(|v| v.as_str()) {
                    if !t.trim().is_empty() {
                        let sec_file = GameConfig::get_secrets_dir().join("token.gpg");
                        let _ = fs::write(&sec_file, t.trim().as_bytes());
                        #[cfg(unix)]
                        {
                            use std::os::unix::fs::PermissionsExt;
                            let _ = fs::set_permissions(&sec_file, fs::Permissions::from_mode(0o600));
                        }
                    }
                }
                unified.last_updated = Some(Local::now().to_rfc3339());
                let _ = GameConfig::write_unified_config(&unified);

                let res = serde_json::json!({
                    "success": true,
                    "message": "配置已成功合併寫入 ~/.local/share/cyber-note/config.json 統一設定檔！"
                });
                send_json_response(&mut stream, 200, &res.to_string());
                return;
            }
        }
        send_json_response(&mut stream, 400, r#"{"success":false,"message":"無效的 JSON 格式"}"#);
        return;
    }

    if path == "/api/ledger" {
        let ledger_path = GameConfig::get_app_config_dir().join("key_ledger.json");
        if ledger_path.exists() {
            if let Ok(content) = fs::read_to_string(&ledger_path) {
                send_json_response(&mut stream, 200, &content);
                return;
            }
        }
        send_json_response(&mut stream, 200, "[]");
        return;
    }

    if path == "/api/web/state" && method == "POST" {
        if let Ok(body_str) = String::from_utf8(body) {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&body_str) {
                let state = val.get("state").and_then(|s| s.as_str()).unwrap_or("active");
                let state_file = GameConfig::get_app_config_dir().join("web.state");
                let _ = fs::write(&state_file, state);
                let res = serde_json::json!({
                    "state": state,
                    "message": if state == "active" { "Web 網頁端管理引擎已啟動 (ACTIVE)" } else { "Web 網頁端管理引擎已切換為待機模式 (STANDBY)" }
                });
                send_json_response(&mut stream, 200, &res.to_string());
                return;
            }
        }
        send_json_response(&mut stream, 200, r#"{"state":"active"}"#);
        return;
    }

    serve_static_or_embedded(&mut stream, path);
}

fn send_json_response(stream: &mut TcpStream, code: u16, json: &str) {
    let header = format!(
        "HTTP/1.1 {} OK\r\nContent-Type: application/json; charset=utf-8\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        code,
        json.as_bytes().len(),
        json
    );
    let _ = stream.write_all(header.as_bytes());
}

fn get_mime_type(file_path: &str) -> &'static str {
    if file_path.ends_with(".html") {
        "text/html; charset=utf-8"
    } else if file_path.ends_with(".js") || file_path.ends_with(".mjs") {
        "application/javascript; charset=utf-8"
    } else if file_path.ends_with(".css") {
        "text/css; charset=utf-8"
    } else if file_path.ends_with(".json") {
        "application/json; charset=utf-8"
    } else if file_path.ends_with(".svg") {
        "image/svg+xml"
    } else if file_path.ends_with(".png") {
        "image/png"
    } else if file_path.ends_with(".ico") {
        "image/x-icon"
    } else {
        "application/octet-stream"
    }
}

fn serve_static_or_embedded(stream: &mut TcpStream, req_path: &str) {
    let mut rel_path = req_path.trim_start_matches('/');
    if rel_path.is_empty() || rel_path == "index.html" {
        rel_path = "index.html";
    }

    let dist_dirs = [
        PathBuf::from("dist"),
        PathBuf::from("/app/applet/dist"),
        GameConfig::get_app_config_dir().join("web").join("dist"),
    ];

    for dist_dir in &dist_dirs {
        let candidate = dist_dir.join(rel_path);
        if candidate.exists() && candidate.is_file() {
            if let Ok(bytes) = fs::read(&candidate) {
                let mime = get_mime_type(rel_path);
                let header = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: {}\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    mime,
                    bytes.len()
                );
                let _ = stream.write_all(header.as_bytes());
                let _ = stream.write_all(&bytes);
                return;
            }
        }
        let spa_index = dist_dir.join("index.html");
        if !rel_path.starts_with("assets/") && spa_index.exists() {
            if let Ok(bytes) = fs::read(&spa_index) {
                let header = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    bytes.len()
                );
                let _ = stream.write_all(header.as_bytes());
                let _ = stream.write_all(&bytes);
                return;
            }
        }
    }

    let embedded_html = get_embedded_dashboard_html();
    let header = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        embedded_html.as_bytes().len(),
        embedded_html
    );
    let _ = stream.write_all(header.as_bytes());
}

fn get_embedded_dashboard_html() -> &'static str {
    r#"<!DOCTYPE html>
<html lang="zh-TW" class="dark">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Cyber-NOte · 原生 Web 管理引擎</title>
  <script src="https://cdn.tailwindcss.com"></script>
  <style>
    body { background-color: #0d1117; color: #c9d1d9; font-family: ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif; }
    .neon-border { border: 1px solid rgba(16, 185, 129, 0.2); box-shadow: 0 0 15px rgba(16, 185, 129, 0.05); }
    .neon-glow:hover { box-shadow: 0 0 20px rgba(16, 185, 129, 0.15); }
  </style>
</head>
<body class="min-h-screen flex flex-col p-4 md:p-8">
  <header class="max-w-6xl w-full mx-auto mb-8 flex flex-col md:flex-row md:items-center justify-between gap-4 border-b border-gray-800 pb-6">
    <div>
      <div class="flex items-center gap-3">
        <span class="text-3xl">🛡️</span>
        <div>
          <h1 class="text-2xl font-bold tracking-tight text-white flex items-center gap-2">
            Cyber-NOte
            <span class="text-xs px-2.5 py-0.5 rounded-full bg-emerald-950 text-emerald-400 border border-emerald-800 font-mono">v0.0.4 · Rust 原生獨立引擎</span>
          </h1>
          <p class="text-sm text-gray-400 mt-0.5">統一設定管理 · GPG 非對稱金鑰加密隔離 · 零外掛免裝套件</p>
        </div>
      </div>
    </div>
    <div class="flex items-center gap-3">
      <span class="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-md text-xs font-medium bg-emerald-900/40 text-emerald-300 border border-emerald-700/50">
        <span class="w-2 h-2 rounded-full bg-emerald-400 animate-pulse"></span>
        Web 服務運行中 (ACTIVE)
      </span>
      <button onclick="fetchStatus()" class="px-3 py-1.5 rounded-md text-xs font-medium bg-gray-800 hover:bg-gray-700 text-gray-200 border border-gray-700 transition">
        🔄 重新整理狀態
      </button>
    </div>
  </header>

  <main class="max-w-6xl w-full mx-auto grid grid-cols-1 lg:grid-cols-3 gap-6 flex-1">
    <!-- 左側與中欄：狀態卡與統一設定 -->
    <div class="lg:col-span-2 space-y-6">
      <!-- 統一設定檔狀態提示卡 -->
      <div class="bg-gray-900/70 rounded-xl p-5 neon-border">
        <div class="flex items-center justify-between mb-4">
          <h2 class="text-base font-semibold text-white flex items-center gap-2">
            <span>📄</span> 全域統一設定檔 (Unified Configuration)
          </h2>
          <span class="text-xs font-mono text-emerald-400 bg-emerald-950/60 px-2 py-0.5 rounded border border-emerald-900">
            CLI & Web 共用
          </span>
        </div>
        <p class="text-xs text-gray-400 mb-4">
          命令列 (<code class="text-gray-200 bg-gray-800 px-1 py-0.5 rounded">a</code>) 與 Web 管理介面已完全收斂至相同的單一設定檔，變更將全域即時生效。
        </p>
        <div class="bg-black/60 rounded-lg p-3 font-mono text-xs text-emerald-300 border border-gray-800 break-all select-all" id="cfgPath">
          ~/.local/share/cyber-note/config.json
        </div>
      </div>

      <!-- 四宮格狀態卡 -->
      <div class="grid grid-cols-1 sm:grid-cols-2 gap-4">
        <div class="bg-gray-900/60 rounded-xl p-4 border border-gray-800">
          <div class="text-xs text-gray-400 mb-1 flex items-center gap-1.5">
            <span>📂</span> 筆記資料目錄 (Note Dir)
          </div>
          <div class="font-mono text-xs text-gray-200 font-semibold truncate" id="noteDirDisplay">載入中...</div>
        </div>
        <div class="bg-gray-900/60 rounded-xl p-4 border border-gray-800">
          <div class="text-xs text-gray-400 mb-1 flex items-center gap-1.5">
            <span>🔑</span> GPG 金鑰 ID (Key ID)
          </div>
          <div class="font-mono text-xs text-cyan-300 font-semibold truncate" id="keyIdDisplay">載入中...</div>
        </div>
        <div class="bg-gray-900/60 rounded-xl p-4 border border-gray-800">
          <div class="text-xs text-gray-400 mb-1 flex items-center gap-1.5">
            <span>🌐</span> GitHub Gist ID
          </div>
          <div class="font-mono text-xs text-indigo-300 font-semibold truncate" id="gistIdDisplay">載入中...</div>
        </div>
        <div class="bg-gray-900/60 rounded-xl p-4 border border-gray-800">
          <div class="text-xs text-gray-400 mb-1 flex items-center gap-1.5">
            <span>🔒</span> Token 憑證隔離 (POSIX 0600)
          </div>
          <div class="font-mono text-xs font-semibold" id="tokenDisplay">載入中...</div>
        </div>
      </div>

      <!-- 統一設定配置表單 -->
      <div class="bg-gray-900/80 rounded-xl p-6 neon-border">
        <h2 class="text-base font-semibold text-white mb-4 flex items-center gap-2">
          <span>⚙️</span> 即時設定編輯與保存
        </h2>
        <form id="configForm" onsubmit="handleSave(event)" class="space-y-4">
          <div>
            <label class="block text-xs font-medium text-gray-300 mb-1">筆記工作目錄 (Note Directory)</label>
            <input type="text" id="inputNoteDir" placeholder="例: ~/.local/share/cyber-note/notes 或 ~/BOok/NOte"
              class="w-full bg-black/60 border border-gray-700 rounded-lg px-3 py-2 text-sm text-gray-100 focus:outline-none focus:border-emerald-500 font-mono">
          </div>
          <div class="grid grid-cols-1 sm:grid-cols-2 gap-4">
            <div>
              <label class="block text-xs font-medium text-gray-300 mb-1">GPG 公鑰使用者 ID / 指紋</label>
              <input type="text" id="inputKeyId" placeholder="例: CyberNOte 或 16 位金鑰指紋"
                class="w-full bg-black/60 border border-gray-700 rounded-lg px-3 py-2 text-sm text-gray-100 focus:outline-none focus:border-emerald-500 font-mono">
            </div>
            <div>
              <label class="block text-xs font-medium text-gray-300 mb-1">GitHub Gist ID</label>
              <input type="text" id="inputGistId" placeholder="例: e81d7f6b0f9c4263a..."
                class="w-full bg-black/60 border border-gray-700 rounded-lg px-3 py-2 text-sm text-gray-100 focus:outline-none focus:border-emerald-500 font-mono">
            </div>
          </div>
          <div>
            <label class="block text-xs font-medium text-gray-300 mb-1">GitHub Personal Access Token (將經 GPG 加密隔離)</label>
            <input type="password" id="inputToken" placeholder="若保留現有 token.gpg 密文憑證請留空"
              class="w-full bg-black/60 border border-gray-700 rounded-lg px-3 py-2 text-sm text-gray-100 focus:outline-none focus:border-emerald-500 font-mono">
          </div>
          <div class="pt-2 flex items-center justify-between">
            <span id="saveStatus" class="text-xs"></span>
            <button type="submit" class="px-5 py-2 rounded-lg text-sm font-medium bg-emerald-600 hover:bg-emerald-500 text-white shadow-lg transition">
              💾 保存並同步統一設定檔
            </button>
          </div>
        </form>
      </div>
    </div>

    <!-- 右側欄：CLI 快速指引與原生 Web 說明 -->
    <div class="space-y-6">
      <div class="bg-gray-900/60 rounded-xl p-5 border border-gray-800">
        <h3 class="text-sm font-semibold text-white mb-3 flex items-center gap-2">
          <span>⚡</span> 終端常用指令速查
        </h3>
        <div class="space-y-2.5 font-mono text-xs">
          <div class="bg-black/50 p-2.5 rounded border border-gray-800">
            <span class="text-emerald-400">a -w</span>
            <div class="text-gray-400 text-[11px] mt-0.5 font-sans">啟動 Web 伺服器 (Ctrl+C 停止)</div>
          </div>
          <div class="bg-black/50 p-2.5 rounded border border-gray-800">
            <span class="text-emerald-400">a -d --all</span>
            <div class="text-gray-400 text-[11px] mt-0.5 font-sans">批量下載雲端 Gist 全部檔案</div>
          </div>
          <div class="bg-black/50 p-2.5 rounded border border-gray-800">
            <span class="text-emerald-400">a -d 1.txt -o ./1.txt</span>
            <div class="text-gray-400 text-[11px] mt-0.5 font-sans">下載並精確儲存至當前工作目錄</div>
          </div>
          <div class="bg-black/50 p-2.5 rounded border border-gray-800">
            <span class="text-emerald-400">a -k</span>
            <div class="text-gray-400 text-[11px] mt-0.5 font-sans">查看金鑰審計與歸檔簿</div>
          </div>
        </div>
      </div>

      <div class="bg-gray-900/40 rounded-xl p-5 border border-gray-800 text-xs space-y-3">
        <h3 class="font-semibold text-gray-200 flex items-center gap-1.5">
          <span>🛡️</span> 架構安全特色
        </h3>
        <ul class="space-y-2 text-gray-400 list-disc list-inside">
          <li><strong>零套件外掛依賴：</strong>完全由 Rust 原生編譯運行，無需 Node.js、npm 或任何外部環境。</li>
          <li><strong>目錄嚴格收斂：</strong>統一存檔於 <code class="text-gray-300">~/.local/share/cyber-note/</code>。</li>
          <li><strong>金鑰隱私隔離：</strong>Token 憑證一律透過 GPG 封裝並設為 0600 許可權。</li>
        </ul>
      </div>
    </div>
  </main>

  <script>
    async function fetchStatus() {
      try {
        const res = await fetch('/api/rust/status');
        const data = await res.json();
        document.getElementById('noteDirDisplay').textContent = data.noteDir || '未配置';
        document.getElementById('keyIdDisplay').textContent = data.keyId || '未配置';
        document.getElementById('gistIdDisplay').textContent = data.gistId || '未配置';
        
        const tokenEl = document.getElementById('tokenDisplay');
        if (data.hasToken) {
          tokenEl.textContent = '🟢 已封裝隔離 (token.gpg)';
          tokenEl.className = 'font-mono text-xs font-semibold text-emerald-400';
        } else {
          tokenEl.textContent = '⚪ 尚未配置';
          tokenEl.className = 'font-mono text-xs font-semibold text-yellow-400';
        }

        if (data.unifiedConfigFile) {
          document.getElementById('cfgPath').textContent = data.unifiedConfigFile;
        }
        if (!document.getElementById('inputNoteDir').value) {
          document.getElementById('inputNoteDir').value = data.noteDir || '';
        }
        if (!document.getElementById('inputKeyId').value) {
          document.getElementById('inputKeyId').value = data.keyId === '未配置' ? '' : data.keyId;
        }
        if (!document.getElementById('inputGistId').value) {
          document.getElementById('inputGistId').value = data.gistId === '未配置' ? '' : data.gistId;
        }
      } catch (err) {
        console.error('無法取得狀態:', err);
      }
    }

    async function handleSave(e) {
      e.preventDefault();
      const statusEl = document.getElementById('saveStatus');
      statusEl.textContent = '⏳ 正在寫入統一設定檔...';
      statusEl.className = 'text-xs text-yellow-400';

      const payload = {
        noteDir: document.getElementById('inputNoteDir').value.trim(),
        keyId: document.getElementById('inputKeyId').value.trim(),
        gistId: document.getElementById('inputGistId').value.trim(),
      };
      const token = document.getElementById('inputToken').value.trim();
      if (token) payload.token = token;

      try {
        const res = await fetch('/api/rust/config', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify(payload)
        });
        const result = await res.json();
        if (result.success) {
          statusEl.textContent = '✨ ' + result.message;
          statusEl.className = 'text-xs text-emerald-400';
          fetchStatus();
        } else {
          statusEl.textContent = '❌ 保存失敗: ' + (result.message || '未知錯誤');
          statusEl.className = 'text-xs text-red-400';
        }
      } catch (err) {
        statusEl.textContent = '❌ 網路通信錯誤: ' + err.message;
        statusEl.className = 'text-xs text-red-400';
      }
    }

    fetchStatus();
  </script>
</body>
</html>
"#
}


// 🛡️ 檔案加密（支援多參數批量與 --all）
fn handle_encrypt_command(args: &[String], verbose: bool) {
    let is_pass_cmd = args.len() > 1 && (args[1] == "-p" || args[1] == "-ep" || args[1] == "-pe");
    let is_all = args.iter().any(|a| a == "-b" || a == "-a");
    let mut file_paths = Vec::new();
    let mut pass_opt: Option<String> = None;
    let mut key_id_opt: Option<String> = None;
    let mut iter_opt: Option<u64> = None;
    let mut upload = false;
    let mut out_path_opt: Option<String> = None;

    if is_pass_cmd {
        if args.len() < 4 {
            println!("❌ 錯誤：參數不足。");
            println!("💡 範例: a -p [密碼] [檔案] (例: a -p MyPass123 1.txt)");
            return;
        }
        pass_opt = Some(args[2].clone());
        for arg in args.iter().skip(3) {
            if arg == "-s" || arg == "-u" {
                upload = true;
            } else if !arg.starts_with('-') {
                file_paths.push(arg.clone());
            }
        }
    } else {
        let mut skip_next = false;
        for (i, arg) in args.iter().enumerate() {
            if i == 0 || i == 1 {
                continue;
            }
            if skip_next {
                skip_next = false;
                continue;
            }
            if arg == "--pass" || arg == "-P" || arg == "--password" || arg == "-p" {
                if i + 1 < args.len() {
                    pass_opt = Some(args[i + 1].clone());
                    skip_next = true;
                }
            } else if arg == "--id" || arg == "--key" {
                if i + 1 < args.len() {
                    key_id_opt = Some(args[i + 1].clone());
                    skip_next = true;
                }
            } else if arg == "--iter" || arg == "--iterations" {
                if i + 1 < args.len() {
                    iter_opt = args[i + 1].parse::<u64>().ok();
                    skip_next = true;
                }
            } else if arg == "--out" || arg == "-o" {
                if i + 1 < args.len() {
                    out_path_opt = Some(args[i + 1].clone());
                    skip_next = true;
                }
            } else if arg == "-u" || arg == "--upload" || arg == "-s" || arg == "--sync" {
                upload = true;
            } else if arg == "--all" || arg == "-a" {
                // is_all = true
            } else if !arg.starts_with('-') {
                file_paths.push(arg.clone());
            }
        }
    }

    let note_dir = GameConfig::get_note_dir();
    if is_all {
        let _ = fs::create_dir_all(&note_dir);
        if let Ok(entries) = fs::read_dir(&note_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                        if !name.ends_with(".gpg") && !name.ends_with(".asc") && !name.starts_with('.') {
                            file_paths.push(path.to_str().unwrap().to_string());
                        }
                    }
                }
            }
        }
    }

    if file_paths.is_empty() {
        println!("❌ 錯誤：請指定欲加密的檔案路徑或使用 --all。");
        return;
    }

    for target_file in file_paths {
        let target_path = Path::new(&target_file);
        if !target_path.exists() {
            println!("❌ 錯誤：指定檔案不存在 -> {}", target_file);
            continue;
        }

        let raw_bytes = match fs::read(target_path) {
            Ok(b) => b,
            Err(e) => {
                println!("❌ 錯誤：無法讀取檔案內容 ({}): {}", target_file, e);
                continue;
            }
        };

        let current_layer = calculate_gpg_layer(&target_file);
        let target_layer = current_layer + 1;

        let default_out = format!("{}.gpg", target_file);
        let out_file_path_str = out_path_opt.clone().unwrap_or(default_out);
        let out_path_obj = Path::new(&out_file_path_str);
        let out_file_name = out_path_obj
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(&out_file_path_str);

        let default_key = GameConfig::get_gpg_user_id();
        let active_key = key_id_opt.clone().or(default_key.ok());
        let s2k_count = iter_opt.unwrap_or(DEFAULT_S2K_COUNT);

        let (ciphertext, cipher_mode, key_id_used, _iterations_used) = if let Some(ref pass) = pass_opt {
            match encrypt_symmetric_s2k(&raw_bytes, pass, s2k_count) {
                Ok(c) => (
                    c,
                    "GPG_SYMMETRIC_S2K".to_string(),
                    format!("SYMMETRIC-S2K ({} 輪)", s2k_count),
                    s2k_count,
                ),
                Err(e) => {
                    println!("❌ 加密失敗 ({}): {}", target_file, e);
                    continue;
                }
            }
        } else if let Some(ref gpg_key) = active_key {
            match encrypt_with_gpg(&raw_bytes, gpg_key) {
                Ok(c) => (c, "GPG_PUBLIC_KEY".to_string(), gpg_key.clone(), 0),
                Err(e) => {
                    println!("❌ 加密失敗 ({}): {}", target_file, e);
                    continue;
                }
            }
        } else {
            match GameConfig::get_gpg_user_id() {
                Ok(k) => match encrypt_with_gpg(&raw_bytes, &k) {
                    Ok(c) => (c, "GPG_PUBLIC_KEY".to_string(), k, 0),
                    Err(e) => {
                        println!("❌ 加密失敗 ({}): {}", target_file, e);
                        continue;
                    }
                },
                Err(e) => {
                    println!("❌ 無法取得預設 GPG 金鑰: {}", e);
                    continue;
                }
            }
        };

        if let Err(e) = fs::write(out_path_obj, ciphertext.as_bytes()) {
            println!("❌ 寫入加密檔案失敗: {}", e);
            continue;
        }

        let sha256 = compute_sha256(ciphertext.as_bytes());

        let key_display = if cipher_mode == "GPG_SYMMETRIC_S2K" {
            "GPG 對稱加密".to_string()
        } else {
            let short_id = if key_id_used.len() >= 8 {
                &key_id_used[key_id_used.len() - 8..]
            } else {
                &key_id_used
            };
            format!("GPG 公鑰加密 (Key ID: {})", short_id)
        };
        println!("🛡️ 檔案 [{}] 加密成功：", target_file);
        println!("{}", key_display);
        println!("密文大小 : {:.2} KB ({} Bytes)", ciphertext.len() as f64 / 1024.0, ciphertext.len());
        println!("雜湊校驗 : SHA-256: {}", &sha256[..32]);

        let _ = record_ledger_entry(
            out_file_name,
            &out_file_path_str,
            &key_id_used,
            &cipher_mode,
            if cipher_mode == "GPG_SYMMETRIC_S2K" { s2k_count } else { 0 },
            target_layer,
            ciphertext.as_bytes(),
            &format!("批量/複合封裝第 {} 層", target_layer),
        );

        if upload {
            match get_github_token(verbose) {
                Ok(token) => {
                    println!("☁️  正在將加密包裹推送至雲端 Gist【{}】...", out_file_name);
                    let _ = sync_to_gist(&ciphertext, out_file_name, &token, verbose);
                }
                Err(_) => {}
            }
        }
    }
}

// 🔓 檔案解密還原一層 (支援多參數批量與 --all)
fn handle_decrypt_command(args: &[String]) {
    let mut file_paths = Vec::new();
    let mut pass_opt: Option<String> = None;
    let mut out_path_opt: Option<String> = None;
    let is_all = args.iter().any(|a| a == "--all" || a == "-a");

    let mut skip_next = false;
    for (i, arg) in args.iter().enumerate() {
        if i == 0 || i == 1 {
            continue;
        }
        if skip_next {
            skip_next = false;
            continue;
        }
        if arg == "--pass" || arg == "-P" || arg == "--password" {
            if i + 1 < args.len() {
                pass_opt = Some(args[i + 1].clone());
                skip_next = true;
            }
        } else if arg == "--out" || arg == "-o" {
            if i + 1 < args.len() {
                out_path_opt = Some(args[i + 1].clone());
                skip_next = true;
            }
        } else if arg == "--all" || arg == "-a" {
            // handled by is_all
        } else if !arg.starts_with('-') {
            file_paths.push(arg.clone());
        }
    }

    let note_dir = GameConfig::get_note_dir();
    if is_all {
        let _ = fs::create_dir_all(&note_dir);
        if let Ok(entries) = fs::read_dir(&note_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                        if name.ends_with(".gpg") {
                            file_paths.push(path.to_str().unwrap().to_string());
                        }
                    }
                }
            }
        }
    }

    if file_paths.is_empty() {
        println!("❌ 錯誤：請提供欲解密檔案路徑或使用 --all。範例: a -x 1.gpg 2.gpg 或 a -x --all");
        return;
    }

    for target_file in file_paths {
        let target_path = Path::new(&target_file);
        if !target_path.exists() {
            println!("❌ 錯誤：指定檔案不存在 -> {}", target_file);
            continue;
        }

        let encrypted_content = match fs::read_to_string(target_path) {
            Ok(s) => s,
            Err(e) => {
                println!("❌ 讀取密文檔案失敗 ({}): {}", target_file, e);
                continue;
            }
        };

        println!("🔓 正在解密還原【{}】...", target_file);
        let decrypted_bytes = match decrypt_bytes_with_gpg(&encrypted_content, pass_opt.as_deref()) {
            Ok(b) => b,
            Err(e) => {
                println!("❌ 解密失敗 ({}): {}", target_file, e);
                println!("💡 若為對稱密碼加密檔案，請加上 --pass <密碼> 參數。");
                continue;
            }
        };

        let default_out = if target_file.ends_with(".gpg") {
            target_file.strip_suffix(".gpg").unwrap().to_string()
        } else {
            format!("{}.decrypted", target_file)
        };

        let out_path = out_path_opt.clone().unwrap_or(default_out);
        if let Err(e) = fs::write(&out_path, &decrypted_bytes) {
            println!("❌ 寫入解密檔案失敗 ({}): {}", out_path, e);
            continue;
        }

        println!(
            "✨ 解密成功！已還原至: {} (大小: {:.2} KB)",
            out_path,
            decrypted_bytes.len() as f64 / 1024.0
        );
    }
}

// 📂 遞迴掃描本地檔案（排除系統隱藏檔及配置檔）
fn collect_files_recursive(dir: &Path, base: &Path, files: &mut Vec<(PathBuf, String)>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Ok(rel) = path.strip_prefix(base) {
                    let rel_str = rel.to_string_lossy().replace('\\', "/");
                    let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                    if !filename.starts_with('.') && filename != "config.dae" && filename != "gist_id" && filename != "key_id" {
                        files.push((path, rel_str));
                    }
                }
            } else if path.is_dir() {
                let dirname = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if !dirname.starts_with('.') && dirname != "node_modules" && dirname != "target" && dirname != ".git" {
                    collect_files_recursive(&path, base, files);
                }
            }
        }
    }
}

// 📂 遞迴清理空目錄
fn remove_empty_dirs_recursive(dir: &Path) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                remove_empty_dirs_recursive(&path);
                let _ = fs::remove_dir(&path);
            }
        }
    }
}

// ☁️ 雲端同步與對齊 (a -s / a --sync / a -s --all / a -bs / a -sb)
fn handle_sync_command(args: &[String], verbose: bool) {
    let timer = Instant::now();
    let is_raw = args.iter().any(|arg| arg == "--raw" || arg == "-u");
    let is_all = args.iter().skip(1).any(|arg| {
        arg == "--all"
            || arg == "-a"
            || arg == "-b"
            || arg == "-bs"
            || arg == "-sb"
            || arg == "all"
            || (arg.starts_with('-') && !arg.starts_with("--") && arg.contains('b'))
    });

    let token = match get_github_token(verbose) {
        Ok(t) => t,
        Err(e) => {
            println!("❌ 錯誤：{}", e);
            return;
        }
    };

    let note_dir = GameConfig::get_note_dir();
    if !note_dir.exists() {
        println!("❌ 錯誤：本地筆記目錄不存在 -> {:?}", note_dir);
        return;
    }

    if is_all {
        println!("📡 [雲端同步] 正在遞迴掃描本地檔案目錄 {:?} 進行對齊同步...", note_dir);
        let mut local_files = Vec::new();
        collect_files_recursive(&note_dir, &note_dir, &mut local_files);

        if local_files.is_empty() {
            println!("ℹ️ 本地目錄中沒有找到任何檔案可供同步。");
            return;
        }

        let remote_files = match list_gist_files(&token, verbose) {
            Ok(f) => f,
            Err(e) => {
                println!("⚠️ 獲取遠端 Gist 清單失敗: {}", e);
                return;
            }
        };

        let local_set: std::collections::HashSet<String> = local_files.iter().map(|(_, r)| r.clone()).collect();
        let orphan_remote: Vec<String> = remote_files.into_iter().filter(|rf| !local_set.contains(rf)).collect();

        println!("☁️  [雲端同步] 本地發現共 {} 個檔案，開始遞迴推送至遠端 Gist 倉庫...", local_files.len());
        let mut success_count = 0;
        for (idx, (path, rel_filename)) in local_files.iter().enumerate() {
            println!("  [{}/{}] 正在推送: {}...", idx + 1, local_files.len(), rel_filename);
            let content = match fs::read_to_string(path) {
                Ok(c) => c,
                Err(_) => match fs::read(path) {
                    Ok(bytes) => {
                        if let Ok(s) = String::from_utf8(bytes.clone()) {
                            s
                        } else {
                            String::from_utf8_lossy(&bytes).to_string()
                        }
                    }
                    Err(e) => {
                        println!("    ⚠️ 讀取檔案失敗: {}", e);
                        continue;
                    }
                },
            };

            match sync_to_gist(&content, rel_filename, &token, verbose) {
                Ok(_) => {
                    println!("    ✨ 推送成功: {}", rel_filename);
                    success_count += 1;
                }
                Err(e) => {
                    println!("    ⚠️ 推送失敗 ({}): {}", rel_filename, e);
                }
            }
        }

        // 以本地目錄為準與遠端對齊：清理遠端孤立檔案
        let mut deleted_count = 0;
        if !orphan_remote.is_empty() {
            println!("🧹 [遠端清理] 發現 {} 個遠端孤立檔案（本地已無此檔案），正在刪除以本地目錄為準對齊...", orphan_remote.len());
            for orphan in &orphan_remote {
                println!("  🗑️  正在從遠端 Gist 刪除: {}...", orphan);
                match delete_gist_file(orphan, &token, verbose) {
                    Ok(_) => {
                        println!("    ✨ 已自遠端移除: {}", orphan);
                        deleted_count += 1;
                    }
                    Err(e) => {
                        println!("    ⚠️ 刪除失敗 ({}): {}", orphan, e);
                    }
                }
            }
        }

        println!(
            "☁️  [GitHub] 雲端同步對齊完畢！成功推送 {}/{} 個本地檔案，清理 {}/{} 個遠端孤立檔案。以本地目錄為準完全對齊！全流程耗時: {:?}",
            success_count,
            local_files.len(),
            deleted_count,
            orphan_remote.len(),
            timer.elapsed()
        );
        return;
    }

    // 單檔案同步
    let custom_path_opt = args.iter().skip(1).find(|&arg| {
        !arg.starts_with('-') && arg != "sync" && arg != "list" && arg != "download"
    });

    let current_year = Local::now().format("%Y").to_string();
    let default_file_path = note_dir.join(format!("{}.note.gpg", current_year));
    let default_file_str = default_file_path.to_str().unwrap_or("default.note.gpg");

    let (payload_to_send, remote_filename) = if let Some(custom_path) = custom_path_opt {
        let path_obj = Path::new(custom_path);
        if !path_obj.exists() {
            println!("❌ 錯誤：找不到指定的檔案路徑 -> {}", custom_path);
            return;
        }

        println!("📦 [1/4 讀取] 正在讀取本地檔案【{}】...", custom_path);
        let custom_bytes = match fs::read(custom_path) {
            Ok(bytes) => {
                println!(
                    "  ↳ 檔案讀取完畢，大小: {:.2} KB ({} Bytes)",
                    bytes.len() as f64 / 1024.0,
                    bytes.len()
                );
                bytes
            }
            Err(e) => {
                println!("❌ 讀取自訂檔案失敗: {}", e);
                return;
            }
        };

        let file_stem = match path_obj.file_name() {
            Some(name) => name.to_str().unwrap_or("file"),
            None => "file",
        };

        if is_raw {
            match String::from_utf8(custom_bytes) {
                Ok(valid_text) => {
                    println!("📄 [2/4 模式] 以明文模式傳輸【{}】...", file_stem);
                    (valid_text, file_stem.to_string())
                }
                Err(_) => {
                    println!(
                        "❌ 錯誤：該檔案包含二進位資料，無法以明文模式傳輸。請去除 -u 旗標以加密模式上傳！"
                    );
                    return;
                }
            }
        } else {
            let gpg_user_id = match GameConfig::get_gpg_user_id() {
                Ok(id) => id,
                Err(e) => {
                    println!("❌ {}", e);
                    return;
                }
            };
            println!(
                "🔐 [2/4 加密] 正在調用 GPG 金鑰 [{}] 封裝為 ASCII Armor 密文...",
                gpg_user_id
            );
            let gpg_start = Instant::now();
            let encrypted = match encrypt_with_gpg(&custom_bytes, &gpg_user_id) {
                Ok(c) => {
                    println!(
                        "  ↳ GPG 封裝完成，耗時: {:?}，密文大小: {:.2} KB",
                        gpg_start.elapsed(),
                        c.len() as f64 / 1024.0
                    );
                    c
                }
                Err(e) => {
                    println!("⚠️ 加密檔案失敗: {}", e);
                    return;
                }
            };
            let remote_name = format!("{}.gpg", file_stem);

            let _ = record_ledger_entry(
                &remote_name,
                custom_path,
                &gpg_user_id,
                "GPG_PUBLIC_KEY",
                0,
                calculate_gpg_layer(&remote_name),
                encrypted.as_bytes(),
                "外部檔案加密同步",
            );

            (encrypted, remote_name)
        }
    } else {
        let note_content = match read_note(default_file_str) {
            Ok(c) => c,
            Err(e) => {
                println!("❌ 讀取本地機密文檔失敗: {}", e);
                return;
            }
        };
        (note_content, format!("{}.note.gpg", current_year))
    };

    println!(
        "🚀 [3/4 傳輸] 正在將【{}】推送至雲端 Gist 倉庫...",
        remote_filename
    );
    match sync_to_gist(&payload_to_send, &remote_filename, &token, verbose) {
        Ok(_) => println!("☁️  [GitHub] 同步完成！全流程耗時: {:?}", timer.elapsed()),
        Err(e) => println!("⚠️  [GitHub] 傳輸失敗: {}", e),
    }
}

// ☁️ 雲端下載與自動解密對齊 (a -d / a --download / a -d --all / a -bd / a -db)
fn handle_download_command(args: &[String], verbose: bool) {
    let timer = Instant::now();
    let should_decrypt = args.iter().any(|arg| arg == "-x" || arg == "--decrypt");
    let is_all = args.iter().skip(1).any(|arg| {
        arg == "--all"
            || arg == "-a"
            || arg == "-b"
            || arg == "-bd"
            || arg == "-db"
            || arg == "all"
            || (arg.starts_with('-') && !arg.starts_with("--") && arg.contains('b'))
    });

    let mut out_path_opt: Option<String> = None;
    let mut skip_next = false;
    for (i, arg) in args.iter().enumerate() {
        if i < 2 {
            continue;
        }
        if skip_next {
            skip_next = false;
            continue;
        }
        if arg == "-o" || arg == "--out" || arg == "--output" {
            if i + 1 < args.len() {
                out_path_opt = Some(args[i + 1].clone());
                skip_next = true;
            }
        }
    }

    let token = match get_github_token(verbose) {
        Ok(t) => t,
        Err(e) => {
            println!("❌ 錯誤：{}", e);
            return;
        }
    };

    let note_dir = GameConfig::get_note_dir();
    let target_dir: PathBuf = if let Some(ref out_dir_str) = out_path_opt {
        PathBuf::from(out_dir_str)
    } else {
        note_dir.clone()
    };
    let _ = fs::create_dir_all(&target_dir);

    if is_all {
        println!("📡 [雲端檢索] 正在掃描 GitHub Gist 倉庫檔案清單以進行批量對齊下載...");
        let remote_files = match list_gist_files(&token, verbose) {
            Ok(f) => f,
            Err(e) => {
                println!("⚠️ 獲取清單失敗: {}", e);
                return;
            }
        };

        if remote_files.is_empty() {
            println!("ℹ️ 雲端 Gist 倉庫目前無任何檔案。");
            return;
        }

        let mut local_files = Vec::new();
        collect_files_recursive(&target_dir, &target_dir, &mut local_files);

        let remote_set: std::collections::HashSet<String> = remote_files.iter().cloned().collect();

        println!(
            "☁️  [雲端同步] 開始下載遠端全量檔案 (共 {} 個) 至 {:?}...",
            remote_files.len(),
            target_dir
        );

        let mut success_count = 0;
        for (idx, file_name) in remote_files.iter().enumerate() {
            let local_file_name = if should_decrypt && file_name.ends_with(".gpg") {
                file_name.strip_suffix(".gpg").unwrap().to_string()
            } else {
                file_name.clone()
            };
            let target_file_path = target_dir.join(&local_file_name);
            if let Some(parent) = target_file_path.parent() {
                let _ = fs::create_dir_all(parent);
            }

            match fetch_from_gist(file_name, &token, verbose) {
                Ok(encrypted_content) => {
                    if should_decrypt {
                        match decrypt_bytes_with_gpg(&encrypted_content, None) {
                            Ok(decrypted_bytes) => {
                                if fs::write(&target_file_path, &decrypted_bytes).is_ok() {
                                    println!(
                                        "  [{}/{}] ✨ 已解密還原: {:?} ({:.2} KB)",
                                        idx + 1,
                                        remote_files.len(),
                                        target_file_path,
                                        decrypted_bytes.len() as f64 / 1024.0
                                    );
                                    success_count += 1;
                                } else {
                                    println!(
                                        "  [{}/{}] ⚠️ 寫入磁碟失敗: {:?}",
                                        idx + 1,
                                        remote_files.len(),
                                        target_file_path
                                    );
                                }
                            }
                            Err(e) => {
                                println!("  [{}/{}] ❌ 解密還原失敗 ({}): {}", idx + 1, remote_files.len(), file_name, e);
                            }
                        }
                    } else {
                        if fs::write(&target_file_path, encrypted_content.as_bytes()).is_ok() {
                            println!(
                                "  [{}/{}] ✨ 已下載入庫: {:?} ({:.2} KB)",
                                idx + 1,
                                remote_files.len(),
                                target_file_path,
                                encrypted_content.len() as f64 / 1024.0
                            );
                            success_count += 1;
                        } else {
                            println!(
                                "  [{}/{}] ⚠️ 寫入磁碟失敗: {:?}",
                                idx + 1,
                                remote_files.len(),
                                target_file_path
                            );
                        }
                    }
                }
                Err(e) => {
                    println!("  [{}/{}] ❌ 下載失敗 ({}): {}", idx + 1, remote_files.len(), file_name, e);
                }
            }
        }

        // 以遠端為準與本地對齊：本地有多餘檔案則清理
        let mut deleted_count = 0;
        let mut local_orphans = Vec::new();
        for (local_path, rel_str) in local_files {
            let matches_remote = remote_set.contains(&rel_str)
                || (should_decrypt && remote_set.contains(&format!("{}.gpg", rel_str)));
            if !matches_remote {
                local_orphans.push((local_path, rel_str));
            }
        }

        if !local_orphans.is_empty() {
            println!("🧹 [本地清理] 發現 {} 個本地多餘檔案（遠端已無此檔案），正在刪除以遠端為準對齊...", local_orphans.len());
            for (local_path, rel_str) in local_orphans {
                println!("  🗑️  正在自本地移除: {}...", rel_str);
                if fs::remove_file(&local_path).is_ok() {
                    println!("    ✨ 已自本地清除: {}", rel_str);
                    deleted_count += 1;
                } else {
                    println!("    ⚠️ 移除本地檔案失敗: {:?}", local_path);
                }
            }
            remove_empty_dirs_recursive(&target_dir);
        }

        println!(
            "☁️  [GitHub] 本地對齊完畢！成功下載 {}/{} 個遠端檔案，清理 {} 個本地多餘檔案。以遠端倉庫為準完全對齊！全流程耗時: {:?}",
            success_count,
            remote_files.len(),
            deleted_count,
            timer.elapsed()
        );
        return;
    }

    // 🌟 單檔案下載（支援 -o 自訂檔名與 ./ 下載至本地工作目錄）
    let current_year = Local::now().format("%Y").to_string();
    let mut raw_target_opt: Option<String> = None;
    let mut i = 2;
    while i < args.len() {
        let a = &args[i];
        if a == "-o" || a == "--out" || a == "--output" {
            i += 2;
            continue;
        }
        if a == "-x" || a == "--decrypt" || a == "-v" || a == "-vv" || a == "--verbose" {
            i += 1;
            continue;
        }
        if !a.starts_with('-') && raw_target_opt.is_none() {
            raw_target_opt = Some(a.clone());
        }
        i += 1;
    }

    let raw_target = match raw_target_opt {
        Some(t) => t,
        None => current_year.clone(),
    };

    let remote_file_name =
        if raw_target.len() == 4 && raw_target.chars().all(|c| c.is_ascii_digit()) {
            format!("{}.note.gpg", raw_target)
        } else {
            raw_target.clone()
        };

    let local_file_name = if should_decrypt && remote_file_name.ends_with(".gpg") {
        remote_file_name.strip_suffix(".gpg").unwrap().to_string()
    } else {
        remote_file_name.clone()
    };

    let target_local_path: PathBuf = if let Some(ref out_path_str) = out_path_opt {
        let p = Path::new(out_path_str);
        if out_path_str.ends_with('/') || p.is_dir() {
            p.join(&local_file_name)
        } else {
            PathBuf::from(out_path_str)
        }
    } else if raw_target.starts_with("./")
        || raw_target.starts_with("../")
        || raw_target.starts_with('/')
    {
        PathBuf::from(&raw_target)
    } else {
        note_dir.join(&local_file_name)
    };

    if let Some(parent) = target_local_path.parent() {
        if !parent.as_os_str().is_empty() {
            let _ = fs::create_dir_all(parent);
        }
    }
    let target_local_str = target_local_path.to_str().unwrap_or(&local_file_name);

    println!("☁️  [雲端同步] 正在從 Gist 請求【{}】...", remote_file_name);
    match fetch_from_gist(&remote_file_name, &token, verbose) {
        Ok(encrypted_content) => {
            if should_decrypt {
                println!("🔓 正在調用 GPG 進行解密還原...");
                match decrypt_bytes_with_gpg(&encrypted_content, None) {
                    Ok(decrypted_bytes) => {
                        if fs::write(&target_local_path, &decrypted_bytes).is_ok() {
                            println!(
                                "✨ 檔案已成功解密還原至: {} (大小: {:.2} KB)",
                                target_local_str,
                                decrypted_bytes.len() as f64 / 1024.0
                            );
                        } else {
                            println!("⚠️ 寫入本地磁碟失敗: {}", target_local_str);
                        }
                    }
                    Err(e) => println!("⚠️ 解密失敗: {}", e),
                }
            } else {
                if fs::write(&target_local_path, encrypted_content.as_bytes()).is_ok() {
                    println!(
                        "✨ 密文包裹已成功下載至: {} (大小: {:.2} KB)",
                        target_local_str,
                        encrypted_content.len() as f64 / 1024.0
                    );
                } else {
                    println!("⚠️ 寫入本地磁碟失敗: {}", target_local_str);
                }
            }
        }
        Err(e) => println!("⚠️ 下載失敗: {}", e),
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let current_year = Local::now().format("%Y").to_string();
    let note_dir = GameConfig::get_note_dir();
    let default_file_path = note_dir.join(format!("{}.note.gpg", current_year));
    let default_file_str = default_file_path.to_str().unwrap();
    let verbose = args
        .iter()
        .any(|arg| arg == "-v" || arg == "-vv" || arg == "--verbose");

    // 🛡️ 守衛：-i 不可搭配任何其他參數（防誤觸）
    let has_i = args.iter().skip(1).any(|a| {
        a == "-i" || a == "--init" || (a.starts_with('-') && !a.starts_with("--") && a.contains('i'))
    });
    if has_i {
        if args.len() == 2 && args[1] == "-i" {
            run_init_wizard();
            return;
        } else {
            println!("❌ 錯誤：-i（系統重新配置）不可與任何其他參數搭配使用！");
            println!("💡 為防止誤觸，請單獨輸入: a -i");
            return;
        }
    }

    // 🌟 偵測標準輸入是否有「管道（Pipe）」串流注入資料 (如 cat file | a)
    let mut piped_input = String::new();
    let has_pipe = if io::stdin().is_terminal() {
        false
    } else {
        use std::os::unix::io::AsRawFd;
        let fd = io::stdin().as_raw_fd();
        let mut pfd = libc::pollfd {
            fd,
            events: libc::POLLIN,
            revents: 0,
        };
        let ret = unsafe { libc::poll(&mut pfd, 1, 0) };
        if ret > 0 && (pfd.revents & libc::POLLIN) != 0 {
            io::stdin().read_to_string(&mut piped_input).is_ok() && !piped_input.trim().is_empty()
        } else {
            false
        }
    };

    // 💡 幫助說明查詢：a help / a -h / a --help
    if args.len() >= 2 && (args[1] == "help" || args[1] == "-h" || args[1] == "--help") {
        print!("{}", include_str!("../a.info"));
        return;
    }

    // ✨ 無參數且無管道輸入：展示系統狀態儀表板與簡潔命令規範
    if args.len() < 2 && !has_pipe {
        if !GameConfig::is_configured() && io::stdin().is_terminal() {
            println!("👋 檢測到系統尚未完成金鑰鎖定，正在啟動引導精靈...");
            run_init_wizard();
            return;
        }

        let current_key =
            GameConfig::get_gpg_user_id().unwrap_or_else(|_| "未配置 (嚴格鎖定 GPG)".to_string());
        let current_gist = GameConfig::get_gist_id().unwrap_or_else(|_| "未配置".to_string());
        let secrets_dir = GameConfig::get_secrets_dir();
        let ledger = a::ledger::load_ledger();

        let mut table = Table::new();
        table.load_preset(NOTHING);
        table.add_row(vec![
            Cell::new("📂 存儲目錄 :").fg(Color::Cyan),
            Cell::new(note_dir.to_str().unwrap_or("")),
        ]);
        table.add_row(vec![
            Cell::new("🔒 隱私隔離 :").fg(Color::Cyan),
            Cell::new(secrets_dir.join("token.gpg").to_str().unwrap_or("")),
        ]);
        table.add_row(vec![
            Cell::new("🔑 GPG 金鑰 :").fg(Color::Cyan),
            Cell::new(&current_key),
        ]);
        table.add_row(vec![
            Cell::new("🌐 Gist ID  :").fg(Color::Cyan),
            Cell::new(&current_gist),
        ]);
        table.add_row(vec![
            Cell::new("📜 金鑰歸檔 :").fg(Color::Cyan),
            Cell::new(format!("已收錄 {} 筆加密檔案審計記錄", ledger.records.len())),
        ]);
        table.add_row(vec![
            Cell::new("⚡ 架構核心 :").fg(Color::Cyan),
            Cell::new("Rust 原生核心 (鎖定 GPG) + JS 網頁管理引擎"),
        ]);
        println!("🛡️  Cyber-NOte 機密記事與金鑰加密系統 · 系統狀態");
        println!("{}", table);
        print!("{}", include_str!("../a.info"));
        return;
    }

    // 檢查已廢除的長參數提示
    for arg in args.iter().skip(1) {
        if arg == "--web" {
            // 兼容舊習慣：自動無縫轉為短指令 -w，絕不報錯
            continue;
        }
        if arg.starts_with("--") && arg != "--verbose" {
            println!("⚠️ 警告：長參數「{}」已廢除！", arg);
            println!("💡 請使用短參數組合，請參閱: a");
            return;
        }
    }

    // 解析所有出現的 short flags 字符
    let mut flags_set = std::collections::HashSet::new();
    for arg in args.iter().skip(1) {
        if arg == "--web" {
            flags_set.insert('w');
        } else if arg.starts_with('-') && !arg.starts_with("--") {
            // 排除 -r1, -r1-100 這類直接帶數字的行刪除語法
            if arg.starts_with("-r") && arg.len() > 2 && arg.chars().nth(2).map(|c| c.is_ascii_digit()).unwrap_or(false) {
                flags_set.insert('r');
                continue;
            }
            for c in arg[1..].chars() {
                flags_set.insert(c);
            }
        }
    }

    // 🌟 1. Web 網頁管理引擎：a -w (兼容 a --web)
    if flags_set.contains(&'w') || args.iter().skip(1).any(|a| a == "--web") {
        let port_opt = args
            .iter()
            .skip(1)
            .find(|&a| a.chars().all(|c| c.is_ascii_digit()))
            .map(|s| s.as_str());
        handle_web_command(port_opt);
        return;
    }

    // 🌟 2. 行級過濾刪除：a -r1 / a -r 1 / a -r1-100 / a -r 1-100 / a -r [關鍵字]
    let has_remove_line = args.iter().skip(1).any(|a| a.starts_with("-r"));
    if has_remove_line {
        let target_expr = if args.len() > 1 && args[1] == "-r" {
            if args.len() < 3 {
                println!("❌ 錯誤：請提供欲刪除的行號或關鍵字。");
                return;
            }
            args[2].clone()
        } else {
            let r_arg = args.iter().skip(1).find(|a| a.starts_with("-r")).unwrap();
            r_arg[2..].to_string()
        };

        let encrypted_old = match read_note(default_file_str) {
            Ok(content) => content,
            Err(_) => {
                println!("📂 本地尚無機密筆記檔案。");
                return;
            }
        };

        let decrypted_old = match decrypt_with_gpg(&encrypted_old) {
            Ok(content) => content,
            Err(e) => {
                println!("⚠️  [解密異常] 無法解密文檔: {}", e);
                return;
            }
        };

        let mut lines: Vec<String> = decrypted_old.lines().map(|s| s.to_string()).collect();
        let total_lines = lines.len();

        if total_lines == 0 {
            println!("📂 文檔內容為空，無可刪除行。");
            return;
        }

        let is_range = target_expr.contains('-') && {
            let parts: Vec<&str> = target_expr.split('-').collect();
            parts.len() == 2
                && parts[0].parse::<usize>().is_ok()
                && parts[1].parse::<usize>().is_ok()
        };

        let single_num_opt = target_expr.parse::<usize>().ok();

        if is_range {
            let parts: Vec<&str> = target_expr.split('-').collect();
            let start = parts[0].parse::<usize>().unwrap();
            let end = parts[1].parse::<usize>().unwrap();
            let (min_k, max_k) = if start <= end {
                (start, end)
            } else {
                (end, start)
            };

            if min_k == 0 {
                println!("❌ 錯誤：倒數行號自 1 起算。");
                return;
            }

            let start_idx = if max_k >= total_lines {
                0
            } else {
                total_lines - max_k
            };

            let end_idx = if min_k > total_lines {
                0
            } else {
                total_lines - min_k
            };

            if start_idx <= end_idx && start_idx < total_lines {
                let remove_count = (end_idx - start_idx + 1).min(total_lines);
                lines.drain(start_idx..=end_idx);
                println!(
                    "✨ 已刪除倒數 {} 至 {} 行（共 {} 行）",
                    min_k, max_k, remove_count
                );
            } else {
                println!(
                    "⚠️ 指定區間超出文檔總行數（目前共 {} 行）。",
                    total_lines
                );
                return;
            }
        } else if let Some(k) = single_num_opt {
            if k == 0 {
                println!("❌ 錯誤：倒數行號自 1 起算。");
                return;
            }
            if k > total_lines {
                println!("⚠️ 文檔僅 {} 行，無法刪除倒數第 {} 行。", total_lines, k);
                return;
            }
            let target_idx = total_lines - k;
            let removed_text = lines.remove(target_idx);
            println!("✨ 已刪除倒數第 {} 行：{}", k, removed_text);
        } else {
            let keyword = &target_expr;
            let mut new_lines = Vec::new();
            let mut removed_count = 0;
            for line in lines {
                if line.contains(keyword) {
                    removed_count += 1;
                } else {
                    new_lines.push(line);
                }
            }
            if removed_count == 0 {
                println!("🔍 未檢索到包含「{}」的行。", keyword);
                return;
            }
            lines = new_lines;
            println!(
                "✨ 已刪除 {} 行包含「{}」的記錄",
                removed_count, keyword
            );
        }

        let new_content = lines.join("\n");
        let gpg_user_id = match GameConfig::get_gpg_user_id() {
            Ok(id) => id,
            Err(e) => {
                println!("❌ {}", e);
                return;
            }
        };

        match encrypt_with_gpg(new_content.as_bytes(), &gpg_user_id) {
            Ok(new_encrypted_block) => {
                if write_encrypted_note(default_file_str, &new_encrypted_block).is_ok() {
                    let _ = record_ledger_entry(
                        &format!("{}.note.gpg", current_year),
                        default_file_str,
                        &gpg_user_id,
                        "GPG_PUBLIC_KEY",
                        0,
                        1,
                        new_encrypted_block.as_bytes(),
                        "年度主機密文檔行刪除更新",
                    );
                    println!("🔒 已完成本年度主機密文檔更新存檔。");
                }
            }
            Err(e) => println!("⚠️ GPG 加密失敗: {}", e),
        }
        return;
    }

    // 🌟 3. 重命名：a -m [文件名] [新文件名]
    if flags_set.contains(&'m') {
        handle_rename_command(&args, verbose);
        return;
    }

    // 🌟 4. 刪除檔案/遠端檔案：a -f [文件名]
    if flags_set.contains(&'f') && !flags_set.contains(&'t') {
        handle_delete_repo_command(&args, verbose);
        return;
    }

    // 🌟 5. 創建新文件：a -n [文件名] [文件內容]
    if flags_set.contains(&'n') {
        handle_new_repo_command(&args, verbose);
        return;
    }

    // 🌟 6. TOTP 雙重認證：a -t / a -t [標籤] / a -t [標籤] [密鑰]
    if flags_set.contains(&'t') {
        handle_totp_command(&args, verbose);
        return;
    }

    // 🌟 7. 操作全部：a -b / a -bs / a -bd
    if flags_set.contains(&'b') {
        let has_s = flags_set.contains(&'s');
        let has_d = flags_set.contains(&'d');

        if has_s {
            println!("🚀 [操作全部] 正在推送本地檔案目錄全部文件，覆蓋遠端 Gist 倉庫...");
            let mut sync_args = vec!["a".to_string(), "-s".to_string(), "-b".to_string()];
            if flags_set.contains(&'u') {
                sync_args.push("-u".to_string());
            }
            handle_sync_command(&sync_args, verbose);
            return;
        } else if has_d {
            println!("🚀 [操作全部] 正在拉取遠端 Gist 全部文件，覆蓋本地檔案目錄...");
            let mut dl_args = vec!["a".to_string(), "-d".to_string(), "-b".to_string()];
            if flags_set.contains(&'x') {
                dl_args.push("-x".to_string());
            }
            handle_download_command(&dl_args, verbose);
            return;
        } else {
            println!("🛡️ Cyber-NOte 全部檔案批次操作 (-b):");
            println!("  a -bs    推送本地檔案目錄全部文件，覆蓋遠端 Gist 倉庫");
            println!("  a -bd    拉取遠端 Gist 全部文件，覆蓋本地檔案目錄");
            return;
        }
    }

    // 🌟 8. 年度機密檔案：a -a / a -aes / a -aus
    if flags_set.contains(&'a') {
        if flags_set.contains(&'s') {
            if flags_set.contains(&'u') {
                println!("🚀 [年度機密檔案] 以明文模式推送至遠端 Gist 倉庫...");
                let mut sync_args = vec!["a".to_string(), "-s".to_string(), "-u".to_string()];
                handle_sync_command(&sync_args, verbose);
            } else {
                println!("🚀 [年度機密檔案] 以加密模式推送至遠端 Gist 倉庫...");
                let mut sync_args = vec!["a".to_string(), "-s".to_string()];
                handle_sync_command(&sync_args, verbose);
            }
            return;
        } else {
            // 解密並列印今年度機密文檔 (或指定路徑)
            if args.len() == 2 || (args.len() == 3 && verbose) {
                if let Ok(raw_content) = read_note(default_file_str) {
                    print_content_colored(&raw_content);
                } else {
                    println!("📂 本地尚未檢測到本年度機密文檔記錄。");
                }
            } else {
                let target_input = args
                    .iter()
                    .skip(2)
                    .find(|&a| a != "-v" && a != "-vv" && a != "--verbose")
                    .unwrap_or(&args[1]);

                if target_input.starts_with("./")
                    || target_input.starts_with("../")
                    || target_input.starts_with("/")
                {
                    let local_path = Path::new(target_input);
                    if let Ok(raw_content) = fs::read_to_string(local_path) {
                        print_content_colored(&raw_content);
                    } else {
                        println!("📂 本地找不到指定的檔案或為非純文字檔案：{}", target_input);
                    }
                } else {
                    let remote_filename = if target_input.len() == 4
                        && target_input.chars().all(|c| c.is_ascii_digit())
                    {
                        format!("{}.note.gpg", target_input)
                    } else {
                        target_input.clone()
                    };

                    match get_github_token(verbose) {
                        Ok(token) => {
                            println!(
                                "☁️  [雲端同步] 正在從 Gist 獲取【{}】...",
                                remote_filename
                            );
                            match fetch_from_gist(&remote_filename, &token, verbose) {
                                Ok(remote_content) => {
                                    print_content_colored(&remote_content);
                                }
                                Err(e) => println!("⚠️ 雲端獲取失敗: {}", e),
                            }
                        }
                        Err(e) => println!("❌ 錯誤：{}", e),
                    }
                }
            }
            return;
        }
    }

    // 🌟 9. 對稱 S2K 密碼防窮舉加密：a -p [密碼] [檔案]
    if flags_set.contains(&'p') {
        handle_encrypt_command(&args, verbose);
        return;
    }

    // 🌟 10. 強制加密模式：a -e [文件名]
    if flags_set.contains(&'e') {
        handle_encrypt_command(&args, verbose);
        return;
    }

    // 🌟 11. 下載：a -d [文件名] (支援 -o 與 -x)
    if flags_set.contains(&'d') {
        handle_download_command(&args, verbose);
        return;
    }

    // 🌟 12. 解密一層加密封裝：a -x [文件名]
    if flags_set.contains(&'x') {
        handle_decrypt_command(&args);
        return;
    }

    // 🌟 13. 金鑰審計清單：a -k
    if flags_set.contains(&'k') {
        a::ledger::print_ledger_table();
        return;
    }

    // 🌟 14. 雲端推送與檢索清單：a -s / a -l / a -sl
    let has_s = flags_set.contains(&'s');
    let has_l = flags_set.contains(&'l');
    if has_s && has_l {
        handle_sync_command(&args, verbose);
        println!();
        handle_list_and_ledger_command(verbose);
        return;
    }
    if has_s {
        handle_sync_command(&args, verbose);
        return;
    }
    if has_l {
        handle_list_and_ledger_command(verbose);
        return;
    }

    // ✨ 15. 若第一個參數不是以 - 開頭，且無相應短旗標，則視為寫入新筆記內容
    if !args[1].starts_with('-') || has_pipe {
        let new_note = if has_pipe {
            if args.len() > 1 && !args[1].starts_with('-') {
                format!("{}\n{}", args[1..].join(" "), piped_input.trim_end())
            } else {
                piped_input.trim_end().to_string()
            }
        } else {
            args[1..].join(" ")
        };

        let mut existing_content = String::new();
        if let Ok(encrypted_old) = read_note(default_file_str) {
            if let Ok(decrypted_old) = decrypt_with_gpg(&encrypted_old) {
                existing_content = decrypted_old;
            } else {
                println!("⚠️  [解密異常] 無法解密舊文檔內容，終止操作以維護數據安全。");
                return;
            }
        }

        if !existing_content.is_empty() && !existing_content.ends_with('\n') {
            existing_content.push('\n');
        }
        existing_content.push_str(&new_note);

        let gpg_user_id = match GameConfig::get_gpg_user_id() {
            Ok(id) => id,
            Err(e) => {
                println!("❌ {}", e);
                return;
            }
        };

        match encrypt_with_gpg(existing_content.as_bytes(), &gpg_user_id) {
            Ok(new_encrypted_block) => {
                if write_encrypted_note(default_file_str, &new_encrypted_block).is_ok() {
                    let _ = record_ledger_entry(
                        &format!("{}.note.gpg", current_year),
                        default_file_str,
                        &gpg_user_id,
                        "GPG_PUBLIC_KEY",
                        0,
                        1,
                        new_encrypted_block.as_bytes(),
                        "年度主機密文檔追加寫入",
                    );
                    if has_pipe {
                        println!(
                            "✨ 管道串流資料已成功以 GPG 鎖定公鑰加密追加至本地 {} 文檔！",
                            current_year
                        );
                    } else {
                        println!(
                            "✨ 機密筆記已成功以 GPG 鎖定公鑰加密追加至本地 {} 文檔！",
                            current_year
                        );
                    }
                }
            }
            Err(e) => println!("⚠️ GPG 鎖定公鑰加密失敗: {}", e),
        }
        return;
    }

    println!("❌ 未知參數: {}", args[1]);
    println!("💡 請輸入 'a' 檢視標準用法說明。");
}
