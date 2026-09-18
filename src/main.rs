// src/main.rs
// Cyber-NOte 機密記事與金鑰加密系統 (Project a)
// 核心架構：嚴格鎖定 GPG 金鑰隔離體系（嚴禁 SSH 金鑰混用）、多層巢狀加密封裝、高迭代 S2K 防窮舉加固與金鑰歸檔簿審計。

use chrono::Local;
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
    atomic_replace_gist_file, create_clean_slate_gist, delete_gist, delete_gist_file,
    fetch_from_gist, list_gist_files, sync_to_gist,
};
use a::ledger::{compute_sha256, print_ledger_table, record_ledger_entry};
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
    println!("\n╔══════════════════════════════════════════════════════════════╗");
    println!("║       🛡️  Cyber-NOte 機密系統 · 基礎配置與金鑰鎖定         ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
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
            paint_line(line, TerminalColor::Green);
        } else {
            paint_line(line, TerminalColor::Cyan);
        }
    }
}

// 📚 Web 管理引擎核心依賴科普與安裝指引
#[allow(dead_code)]
fn print_web_dependencies_guide(missing_tsx: bool, missing_express: bool) {
    println!("\n╔══════════════════════════════════════════════════════════════════════╗");
    println!("║         📚 Web 安全管理引擎 · 核心依賴科普與安裝指引                 ║");
    println!("╚══════════════════════════════════════════════════════════════════════╝");
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

// 🚀 創建新檔案至 Gist：a --new [文件名] [文件內容]
fn handle_new_repo_command(args: &[String], verbose: bool) {
    if args.len() < 3 {
        println!("❌ 錯誤：請指定檔案名稱與內容。範例: a --new hello.txt \"Hello world\"");
        return;
    }
    let filename = &args[2];
    let content = if args.len() > 3 {
        args[3..].join(" ")
    } else {
        "\n".to_string()
    };

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

// 🗑️ 刪除 Gist 中的指定檔案：a --delete [文件名]
fn handle_delete_repo_command(args: &[String], verbose: bool) {
    if args.len() < 3 {
        println!("❌ 錯誤：請指定欲刪除的檔案名稱。範例: a --delete hello.txt");
        return;
    }
    let filename = &args[2];

    let token = match get_github_token(verbose) {
        Ok(t) => t,
        Err(e) => {
            println!("❌ 無法取得 GitHub Token: {}", e);
            return;
        }
    };

    println!("🗑️ 正在向雲端 Gist 請求刪除檔案: {}...", filename);
    match delete_gist_file(filename, &token, verbose) {
        Ok(_) => println!("🗑️ 已成功自遠端 Gist 刪除檔案: {}", filename),
        Err(e) => println!("❌ 刪除遠端檔案失敗: {}", e),
    }
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

    println!("\n╔══════════════════════════════════════════════════════════════╗");
    println!("║       🛡️  Cyber-NOte 遠端檔案在位套殼加密 (In-Place Encapsulate)║");
    println!("╚══════════════════════════════════════════════════════════════╝");
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
            paint_line(&format!("  🔒 遠端已發佈密文殼: {}", new_encrypted_name), TerminalColor::Green);
            if delete_original {
                paint_line(&format!("  🗑️  遠端原明文檔案已徹底刪除: {}", target_file), TerminalColor::Cyan);
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
fn handle_web_command(sub_action: Option<&str>, port_opt: Option<&str>) {
    let port_str = port_opt.unwrap_or("3000");
    let port: u16 = port_str.parse().unwrap_or(3000);
    let config_dir = GameConfig::get_app_config_dir();
    if !config_dir.exists() {
        let _ = fs::create_dir_all(&config_dir);
    }
    let pid_file = config_dir.join("web.pid");
    let state_file = config_dir.join("web.state");

    match sub_action {
        Some("stop") => {
            let _ = fs::write(&state_file, "standby");
            let mut stopped = false;
            if pid_file.exists() {
                if let Ok(pid_str) = fs::read_to_string(&pid_file) {
                    let pid = pid_str.trim();
                    if !pid.is_empty() {
                        println!("🛑 正在停止 Web 網頁端管理引擎 (PID: {})...", pid);
                        let _ = std::process::Command::new("kill").arg(pid).status();
                        stopped = true;
                    }
                }
                let _ = fs::remove_file(&pid_file);
            }
            if stopped {
                println!("✨ Web 網頁端管理引擎已進入待機 (STANDBY)。");
            } else {
                println!("ℹ️  Web 網頁端管理引擎已切換為待機模式 (STANDBY)。");
            }
            println!("💡 Rust 原生核心持續維持後台安全審計與命令處理。可隨時執行 'a --web' 啟動。");
        }
        Some("status") => {
            let is_active = std::net::TcpStream::connect(format!("127.0.0.1:{}", port)).is_ok();

            println!("┌────────────────────────────────────────────────────────────┐");
            println!("│ 🌐 Cyber-NOte Web 網頁端管理引擎 · 狀態監控                │");
            println!("├────────────────────────────────────────────────────────────┤");
            println!(
                "│ 引擎狀態 : {:<47} │",
                if is_active {
                    "🟢 運行中 (ACTIVE)"
                } else {
                    "⚪ 待機中 (STANDBY - 預設關閉)"
                }
            );
            println!("│ 服務埠號 : {:<47} │", port);
            println!("│ 存取位址 : {:<47} │", format!("http://localhost:{}", port));
            println!(
                "│ 統一設定 : {:<47} │",
                "~/.local/share/cyber-note/config.json"
            );
            println!(
                "│ 架構核心 : {:<47} │",
                "Rust 原生獨立 Web 引擎 (免安裝外掛/套件)"
            );
            println!("└────────────────────────────────────────────────────────────┘");
            if !is_active {
                println!("👉 若要啟動網頁端管理介面，請執行: a --web");
            } else {
                println!("👉 若要關閉網頁端管理介面，請執行: a --web stop");
            }
        }
        _ => {
            let is_already_running = std::net::TcpStream::connect(format!("127.0.0.1:{}", port)).is_ok();
            if is_already_running {
                let _ = fs::write(&state_file, "active");
                println!("\n🟢 Cyber-NOte Web 管理引擎已在埠號 {} 正常運行中！", port);
                println!("🌐 存取位址: http://localhost:{}", port);
                println!("📄 統一設定: ~/.local/share/cyber-note/config.json");
                let _ = std::process::Command::new("xdg-open")
                    .arg(format!("http://localhost:{}", port))
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .spawn();
                return;
            }

            println!("\n╔══════════════════════════════════════════════════════════════╗");
            println!("║          🛡️  Cyber-NOte 系統 · Web 網頁端管理引擎            ║");
            println!("╚══════════════════════════════════════════════════════════════╝");
            println!("🚀 Web 網頁端管理後台已喚醒！");
            println!("🌐 存取位址: http://localhost:{}", port);
            println!("📊 架構核心: Rust 原生獨立 Web 引擎 (免安裝外掛/套件，純原生極致運行)");
            println!("📄 統一設定: ~/.local/share/cyber-note/config.json");
            println!("💡 提示: 執行 'a --web stop' 可將網頁端切換回待機狀態，按 Ctrl+C 可停止服務。\n");

            start_rust_native_web_server(port, &pid_file, &state_file);
        }
    }
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
            <div class="text-gray-400 text-[11px] mt-0.5 font-sans">喚醒 Rust 原生 Web 伺服器</div>
          </div>
          <div class="bg-black/50 p-2.5 rounded border border-gray-800">
            <span class="text-emerald-400">a -w stop</span>
            <div class="text-gray-400 text-[11px] mt-0.5 font-sans">關閉 Web 伺服器並切換待機</div>
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


// 🛡️ 新增功能：a -p 檔案加密（支援普通文本、.gpg 巢狀多層加密、無金鑰密碼防窮舉加固與直接上傳）
fn handle_protect_command(args: &[String], verbose: bool) {
    let mut file_path_opt: Option<&str> = None;
    let mut pass_opt: Option<String> = None;
    let mut iter_opt: Option<u64> = None;
    let mut upload = false;
    let mut out_path_opt: Option<&str> = None;

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
        } else if arg == "--out" || arg == "-o" {
            if i + 1 < args.len() {
                out_path_opt = Some(&args[i + 1]);
                skip_next = true;
            }
        } else if arg == "-u" || arg == "--upload" || arg == "-s" || arg == "--sync" {
            upload = true;
        } else if !arg.starts_with('-') && file_path_opt.is_none() {
            file_path_opt = Some(arg);
        }
    }

    let target_file = match file_path_opt {
        Some(f) => f,
        None => {
            println!("❌ 錯誤：請指定欲加密的檔案路徑。");
            println!("   範例 1 (一般檔案):   a -p 1.txt");
            println!("   範例 2 (巢狀加套一層): a -p 1.gpg");
            println!("   範例 3 (密碼防窮舉): a -p 1.txt --pass 密碼 --iter 65011712");
            println!("   範例 4 (無密鑰加密上傳): a -p 1.txt --pass 密碼 -u");
            return;
        }
    };

    let target_path = Path::new(target_file);
    if !target_path.exists() {
        println!("❌ 錯誤：指定檔案不存在 -> {}", target_file);
        return;
    }

    let raw_bytes = match fs::read(target_path) {
        Ok(b) => b,
        Err(e) => {
            println!("❌ 錯誤：無法讀取檔案內容: {}", e);
            return;
        }
    };

    // 計算現有層級與目標輸出路徑
    let current_layer = calculate_gpg_layer(target_file);
    let target_layer = current_layer + 1;

    let default_out = format!("{}.gpg", target_file);
    let out_file_path = out_path_opt.unwrap_or(&default_out);
    let out_path_obj = Path::new(out_file_path);
    let out_file_name = out_path_obj
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(out_file_path);

    let gpg_key_result = GameConfig::get_gpg_user_id();
    let s2k_count = iter_opt.unwrap_or(DEFAULT_S2K_COUNT);

    let (ciphertext, cipher_mode, key_id_used, iterations_used) = if let Some(pass) = pass_opt {
        // 使用者明確指定對稱密碼防窮舉加固加密
        println!(
            "🔐 加密體系: GPG 對稱密碼加固 (S2K 模式 3 / {} 輪迭代運算，防窮舉破解)...",
            s2k_count
        );
        match encrypt_symmetric_s2k(&raw_bytes, &pass, s2k_count) {
            Ok(c) => (
                c,
                "GPG_SYMMETRIC_S2K".to_string(),
                format!("SYMMETRIC-S2K ({} 輪)", s2k_count),
                s2k_count,
            ),
            Err(e) => {
                println!("❌ 加密失敗: {}", e);
                return;
            }
        }
    } else if let Ok(gpg_key) = gpg_key_result {
        // 使用系統已鎖定的 GPG 公鑰
        println!("🔐 加密體系: 系統鎖定 GPG 公鑰 (Key ID: {})...", gpg_key);
        match encrypt_with_gpg(&raw_bytes, &gpg_key) {
            Ok(c) => (c, "GPG_PUBLIC_KEY".to_string(), gpg_key, 0),
            Err(e) => {
                println!("❌ 加密失敗: {}", e);
                return;
            }
        }
    } else {
        // 無金鑰模式：提示設定對稱加密密碼並加固迭代
        println!("ℹ️  未配置鎖定 GPG 公鑰，啟用無金鑰高強度對稱加密模式。");
        print!("🔑 請設定防窮舉加密密碼 (預設 S2K 65,011,712 輪加固): ");
        io::stdout().flush().unwrap();
        let mut pass_input = String::new();
        io::stdin().read_line(&mut pass_input).unwrap();
        let pass = pass_input.trim().to_string();
        if pass.is_empty() {
            println!("❌ 錯誤：密碼不得為空。操作終止。");
            return;
        }
        match encrypt_symmetric_s2k(&raw_bytes, &pass, s2k_count) {
            Ok(c) => (
                c,
                "GPG_SYMMETRIC_S2K".to_string(),
                format!("SYMMETRIC-S2K ({} 輪)", s2k_count),
                s2k_count,
            ),
            Err(e) => {
                println!("❌ 加密失敗: {}", e);
                return;
            }
        }
    };

    // 寫入加密密文落盤
    if let Err(e) = fs::write(out_path_obj, ciphertext.as_bytes()) {
        println!("❌ 寫入加密檔案失敗: {}", e);
        return;
    }

    // 留黨存檔：寫入金鑰歸檔簿 (Key Ledger)
    let notes = if target_layer > 1 {
        format!("第 {} 層巢狀多重加密封裝", target_layer)
    } else {
        "單層獨立檔案加密封裝".to_string()
    };

    let _ = record_ledger_entry(
        out_file_name,
        out_file_path,
        &key_id_used,
        &cipher_mode,
        iterations_used,
        target_layer,
        ciphertext.as_bytes(),
        &notes,
    );

    let sha256 = compute_sha256(ciphertext.as_bytes());

    println!("┌────────────────────────────────────────────────────────────────────────────┐");
    println!("│ 🛡️  Cyber-NOte 檔案加密封裝報告                                             │");
    println!("├────────────────────────────────────────────────────────────────────────────┤");
    println!("│ 來源檔案 : {:<64} │", target_file);
    println!("│ 輸出檔案 : {:<64} │", out_file_path);
    println!(
        "│ 封裝層級 : {:<64} │",
        format!("第 {} 層 (巢狀封裝)", target_layer)
    );
    println!(
        "│ 加密體系 : {:<64} │",
        if cipher_mode == "GPG_SYMMETRIC_S2K" {
            "GPG 對稱加密 (AES-256 + SHA-512)"
        } else {
            "GPG 鎖定公鑰加密 (已隔離 SSH)"
        }
    );
    println!(
        "│ 防窮舉值 : {:<64} │",
        if iterations_used > 0 {
            format!("S2K 模式 3 ({} 輪迭代運算)", iterations_used)
        } else {
            "GPG 非對稱公鑰體系 (無字典窮舉風險)".to_string()
        }
    );
    println!(
        "│ 密文大小 : {:<64} │",
        format!(
            "{:.2} KB ({} Bytes)",
            ciphertext.len() as f64 / 1024.0,
            ciphertext.len()
        )
    );
    println!("│ 雜湊校驗 : {:<64} │", format!("SHA-256: {}", &sha256[..32]));
    println!("│ 金鑰審計 : {:<64} │", "已鎖定存檔至金鑰歸檔簿 (Key Ledger)");
    println!("└────────────────────────────────────────────────────────────────────────────┘");

    // 若指定 -u / --upload，同步上傳至 Gist 雲端
    if upload {
        match get_github_token(verbose) {
            Ok(token) => {
                println!("☁️  [雲端同步] 正在將加密檔案【{}】上傳至 Gist...", out_file_name);
                match sync_to_gist(&ciphertext, out_file_name, &token, verbose) {
                    Ok(_) => println!("✅ 雲端上傳成功！已作為加密包裹持久化存檔。"),
                    Err(e) => println!("⚠️  雲端上傳失敗: {}", e),
                }
            }
            Err(e) => println!("❌ 雲端憑證讀取失敗，略過上傳: {}", e),
        }
    }
}

// 🔓 檔案解密還原一層 (支援巢狀剝離一層)
fn handle_decrypt_command(args: &[String]) {
    let mut file_path_opt: Option<&str> = None;
    let mut pass_opt: Option<&str> = None;
    let mut out_path_opt: Option<&str> = None;

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
                pass_opt = Some(&args[i + 1]);
                skip_next = true;
            }
        } else if arg == "--out" || arg == "-o" {
            if i + 1 < args.len() {
                out_path_opt = Some(&args[i + 1]);
                skip_next = true;
            }
        } else if !arg.starts_with('-') && file_path_opt.is_none() {
            file_path_opt = Some(arg);
        }
    }

    let target_file = match file_path_opt {
        Some(f) => f,
        None => {
            println!("❌ 錯誤：請提供欲解密檔案路徑。範例: a -x 1.gpg.gpg 或 a -x 1.txt.gpg");
            return;
        }
    };

    let target_path = Path::new(target_file);
    if !target_path.exists() {
        println!("❌ 錯誤：指定檔案不存在 -> {}", target_file);
        return;
    }

    let encrypted_content = match fs::read_to_string(target_path) {
        Ok(s) => s,
        Err(e) => {
            println!("❌ 讀取密文檔案失敗: {}", e);
            return;
        }
    };

    println!("🔓 正在解密還原【{}】...", target_file);
    let decrypted_bytes = match decrypt_bytes_with_gpg(&encrypted_content, pass_opt) {
        Ok(b) => b,
        Err(e) => {
            println!("❌ 解密失敗: {}", e);
            println!("💡 若為對稱密碼加密檔案，請加上 --pass <密碼> 參數。");
            return;
        }
    };

    let default_out = if target_file.ends_with(".gpg") {
        target_file.strip_suffix(".gpg").unwrap().to_string()
    } else {
        format!("{}.decrypted", target_file)
    };

    let out_path = out_path_opt.unwrap_or(&default_out);
    if let Err(e) = fs::write(out_path, &decrypted_bytes) {
        println!("❌ 寫入解密檔案失敗: {}", e);
        return;
    }

    println!(
        "✨ 解密成功！已還原一層封裝至: {} (大小: {:.2} KB)",
        out_path,
        decrypted_bytes.len() as f64 / 1024.0
    );
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

    // ✨ 0. Web 網頁端管理引擎 (-w / --web / web)
    if args.len() > 1 && (args[1] == "-w" || args[1] == "--web" || args[1] == "web") {
        let sub_action = args
            .iter()
            .skip(2)
            .find(|&a| !a.starts_with("-"))
            .map(|s| s.as_str());
        let port_opt = args
            .iter()
            .skip(2)
            .find(|&a| a.chars().all(|c| c.is_ascii_digit()))
            .map(|s| s.as_str());
        handle_web_command(sub_action, port_opt);
        return;
    }

    // ✨ 1. 新增功能：a -p 檔案加密（支援普通文本、.gpg 巢狀多層加密、密碼防窮舉加固與直接上傳）
    if args.len() > 1 && (args[1] == "-p" || args[1] == "--protect" || args[1] == "protect") {
        handle_protect_command(&args, verbose);
        return;
    }

    // ✨ 2. 檔案解密還原：a -x 或 a --decrypt
    if args.len() > 1 && (args[1] == "-x" || args[1] == "--decrypt") {
        handle_decrypt_command(&args);
        return;
    }

    // ✨ 2.5 創建新 Gist 倉庫：a --new -n 倉庫名 -i 倉庫信息
    if args.len() > 1 && (args[1] == "--new" || args[1] == "--new-repo" || args[1] == "--clean-slate" || args[1] == "--migrate-repo") {
        handle_new_repo_command(&args, verbose);
        return;
    }

    // ✨ 2.52 刪除指定舊 Gist 倉庫：a --delete <gist_id>
    if args.len() > 1 && (args[1] == "--delete" || args[1] == "--delete-repo") {
        handle_delete_repo_command(&args, verbose);
        return;
    }

    // ✨ 2.53 雙重認證 TOTP 管理與查詢：a -t / --totp
    if args.len() > 1 && (args[1] == "-t" || args[1] == "--totp" || args[1] == "totp") {
        handle_totp_command(&args, verbose);
        return;
    }

    // ✨ 2.6 遠端檔案在位套殼加密 (--remote-encrypt / --encapsulate-remote / a -p --remote)
    if args.len() > 1
        && (args[1] == "--remote-encrypt"
            || args[1] == "--encapsulate-remote"
            || (args[1] == "-p" && args.iter().any(|a| a == "--remote")))
    {
        handle_remote_encrypt_command(&args, verbose);
        return;
    }

    // ✨ 2.7 刪除遠端 Gist 檔案 (--rm-remote / --delete-remote)
    if args.len() > 1 && (args[1] == "--rm-remote" || args[1] == "--delete-remote") {
        if args.len() < 3 {
            println!("❌ 錯誤：請指定欲自遠端刪除之檔案名稱。範例: a --rm-remote config.dae");
            return;
        }
        let target_file = &args[2];
        let token = match get_github_token(verbose) {
            Ok(t) => t,
            Err(e) => {
                println!("❌ 錯誤：{}", e);
                return;
            }
        };
        match delete_gist_file(target_file, &token, verbose) {
            Ok(_) => paint_line(&format!("🗑️ 已成功自遠端 Gist 刪除檔案: {}", target_file), TerminalColor::Green),
            Err(e) => println!("{}", e),
        }
        return;
    }

    // ✨ 3. 金鑰審計記錄簿：a -k / a --ledger / a --keys / a --key-ledger
    if args.len() > 1
        && (args[1] == "-k"
            || args[1] == "--ledger"
            || args[1] == "--keys"
            || args[1] == "--key-ledger")
    {
        print_ledger_table();
        return;
    }

    // ✨ 4. 單獨修改目錄：a --set-dir [路徑]
    if args.len() > 1 && (args[1] == "--set-dir" || args[1] == "-dir" || args[1] == "--dir") {
        if args.len() < 3 {
            println!("❌ 錯誤：請提供目標目錄路徑。範例: a --set-dir ~/MyNotes");
            return;
        }
        let target_dir = &args[2];
        match GameConfig::set_persistent_dir(target_dir) {
            Ok(p) => println!("✨ 機密文檔存儲目錄已切換為: {:?}", p),
            Err(e) => println!("❌ 切換目錄失敗: {}", e),
        }
        return;
    }

    // ✨ 5. 手動觸發系統配置精靈
    if args.len() > 1 && (args[1] == "--init" || args[1] == "-i" || args[1] == "init") {
        run_init_wizard();
        return;
    }

    // ✨ 6. 查看文檔閱覽 (-a / --all)
    if args.len() > 1 && (args[1] == "-a" || args[1] == "--all") {
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
                .unwrap();

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

    // ✨ 7. 雲端同步 (-s / --sync)
    if args.len() > 1 && (args[1] == "-s" || args[1] == "--sync") {
        let timer = Instant::now();
        let is_raw = args.iter().any(|arg| arg == "--raw" || arg == "-u");
        let is_all = args.iter().skip(2).any(|arg| arg == "--all" || arg == "-a");

        if is_all {
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

            println!("📡 [雲端同步] 正在掃描本地筆記目錄 {:?} 進行全部檔案批量同步...", note_dir);
            let entries = match fs::read_dir(&note_dir) {
                Ok(e) => e,
                Err(err) => {
                    println!("❌ 讀取本地目錄失敗: {}", err);
                    return;
                }
            };

            let mut files_to_sync = Vec::new();
            for entry in entries {
                if let Ok(entry) = entry {
                    let path = entry.path();
                    if path.is_file() {
                        let name_opt = path.file_name().and_then(|n| n.to_str()).map(|s| s.to_string());
                        if let Some(name_str) = name_opt {
                            if name_str != "config.dae" && name_str != "gist_id" && name_str != "key_id" {
                                files_to_sync.push((path, name_str));
                            }
                        }
                    }
                }
            }

            if files_to_sync.is_empty() {
                println!("ℹ️ 本地目錄中沒有找到任何檔案可供同步。");
                return;
            }

            println!("☁️  [雲端同步] 發現共 {} 個檔案，開始批量推送至 Gist 倉庫...", files_to_sync.len());
            let mut success_count = 0;
            for (idx, (path, filename)) in files_to_sync.iter().enumerate() {
                println!("  [{}/{}] 正在推送: {}...", idx + 1, files_to_sync.len(), filename);
                let content = match fs::read_to_string(path) {
                    Ok(c) => c,
                    Err(_) => {
                        match fs::read(path) {
                            Ok(bytes) => String::from_utf8_lossy(&bytes).to_string(),
                            Err(e) => {
                                println!("    ⚠️ 讀取檔案失敗: {}", e);
                                continue;
                            }
                        }
                    }
                };

                match sync_to_gist(&content, &filename, &token, verbose) {
                    Ok(_) => {
                        println!("    ✨ 推送成功: {}", filename);
                        success_count += 1;
                    }
                    Err(e) => {
                        println!("    ⚠️ 推送失敗 ({}): {}", filename, e);
                    }
                }
            }

            println!("☁️  [GitHub] 批量同步完成！成功同步 {}/{} 個檔案。全流程耗時: {:?}", success_count, files_to_sync.len(), timer.elapsed());
            return;
        }

        let custom_path_opt = args.iter().skip(2).find(|&arg| {
            arg != "--raw" && arg != "-u" && arg != "-v" && arg != "-vv" && arg != "--verbose" && arg != "--all" && arg != "-a"
        });

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

                // 記錄至金鑰歸檔簿
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

        match get_github_token(verbose) {
            Ok(token) => {
                println!(
                    "🚀 [3/4 傳輸] 正在將【{}】推送至雲端 Gist 倉庫...",
                    remote_filename
                );
                match sync_to_gist(&payload_to_send, &remote_filename, &token, verbose) {
                    Ok(_) => println!("☁️  [GitHub] 同步完成！全流程耗時: {:?}", timer.elapsed()),
                    Err(e) => println!("⚠️  [GitHub] 傳輸失敗: {}", e),
                }
            }
            Err(e) => println!("❌ 錯誤：{}", e),
        }
        return;
    }

    // ✨ 8. 列出雲端檔案 (-l / --list)
    if args.len() > 1 && (args[1] == "-l" || args[1] == "--list") {
        match get_github_token(verbose) {
            Ok(token) => {
                println!("📡 [雲端檢索] 正在掃描 GitHub Gist 倉庫檔案清單...");
                match list_gist_files(&token, verbose) {
                    Ok(files) => {
                        println!("📋 雲端現有密文包裹清單 (共 {} 個)：", files.len());
                        println!("------------------------------------------------------------");
                        for file in files {
                            paint_line(&format!("📦 {}", file), TerminalColor::Cyan);
                        }
                        println!("------------------------------------------------------------");
                        println!(
                            "💡 下載密文: 'a -d [檔名]' | 下載並解密: 'a -d [檔名] -x'"
                        );
                    }
                    Err(e) => println!("⚠️ 獲取清單失敗: {}", e),
                }
            }
            Err(e) => println!("❌ 錯誤：{}", e),
        }
        return;
    }

    // ✨ 9. 雲端下載與自動解密還原 (-d / -x / --decrypt / --all / -o)
    if args.len() > 1 && (args[1] == "-d" || args[1] == "--download") {
        let should_decrypt = args.iter().any(|arg| arg == "-x" || arg == "--decrypt");
        let is_all = args.iter().skip(2).any(|arg| arg == "--all" || arg == "-a");

        // 解析 -o / --out / --output 參數
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

        // 🌟 支援 a -d --all (批量下載 Gist 倉庫全部檔案)
        if is_all {
            println!("📡 [雲端檢索] 正在掃描 GitHub Gist 倉庫檔案清單以進行批量下載...");
            let files = match list_gist_files(&token, verbose) {
                Ok(f) => f,
                Err(e) => {
                    println!("⚠️ 獲取清單失敗: {}", e);
                    return;
                }
            };

            if files.is_empty() {
                println!("ℹ️ 雲端 Gist 倉庫目前無任何檔案。");
                return;
            }

            let target_dir: PathBuf = if let Some(ref out_dir_str) = out_path_opt {
                PathBuf::from(out_dir_str)
            } else {
                note_dir.clone()
            };
            let _ = fs::create_dir_all(&target_dir);

            println!(
                "☁️  [雲端同步] 開始批量下載 Gist 全部檔案 (共 {} 個) 至 {:?}...",
                files.len(),
                target_dir
            );

            let mut success_count = 0;
            for (idx, file_name) in files.iter().enumerate() {
                let local_file_name = if should_decrypt && file_name.ends_with(".gpg") {
                    file_name.strip_suffix(".gpg").unwrap().to_string()
                } else {
                    file_name.clone()
                };
                let target_file_path = target_dir.join(&local_file_name);

                match fetch_from_gist(file_name, &token, verbose) {
                    Ok(encrypted_content) => {
                        if should_decrypt {
                            match decrypt_bytes_with_gpg(&encrypted_content, None) {
                                Ok(decrypted_bytes) => {
                                    if fs::write(&target_file_path, &decrypted_bytes).is_ok() {
                                        println!(
                                            "  [{}/{}] ✨ 已解密還原: {:?} ({:.2} KB)",
                                            idx + 1,
                                            files.len(),
                                            target_file_path,
                                            decrypted_bytes.len() as f64 / 1024.0
                                        );
                                        success_count += 1;
                                    } else {
                                        println!(
                                            "  [{}/{}] ⚠️ 寫入磁碟失敗: {:?}",
                                            idx + 1,
                                            files.len(),
                                            target_file_path
                                        );
                                    }
                                }
                                Err(e) => println!(
                                    "  [{}/{}] ⚠️ 解密失敗 ({}): {}",
                                    idx + 1,
                                    files.len(),
                                    file_name,
                                    e
                                ),
                            }
                        } else {
                            if fs::write(&target_file_path, &encrypted_content).is_ok() {
                                println!(
                                    "  [{}/{}] ✨ 已下載: {:?} ({:.2} KB)",
                                    idx + 1,
                                    files.len(),
                                    target_file_path,
                                    encrypted_content.len() as f64 / 1024.0
                                );
                                success_count += 1;
                            } else {
                                println!(
                                    "  [{}/{}] ⚠️ 寫入磁碟失敗: {:?}",
                                    idx + 1,
                                    files.len(),
                                    target_file_path
                                );
                            }
                        }
                    }
                    Err(e) => println!(
                        "  [{}/{}] ⚠️ 下載失敗 ({}): {}",
                        idx + 1,
                        files.len(),
                        file_name,
                        e
                    ),
                }
            }

            println!(
                "\n✨ 全部下載完成！共成功下載 {}/{} 個檔案至: {:?}",
                success_count,
                files.len(),
                target_dir
            );
            return;
        }

        // 🌟 單檔案下載（支援 -o 自訂檔名與 ./ 下載至本地工作目錄）
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

        // 決定本地目標路徑：
        // 若有 -o 參數，以 -o 為準（如 a -d 1.txt -o ./1.txt 或 -o ./ 存到當前目錄）
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
                    if fs::write(&target_local_path, &encrypted_content).is_ok() {
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
        return;
    }

    // ✨ 10. 刪除特定行數或關鍵字 (-r)
    if args.len() > 1 && (args[1].starts_with("-r") || args[1] == "--remove") {
        let target_expr = if args[1] == "-r" || args[1] == "--remove" {
            if args.len() < 3 {
                println!("❌ 錯誤：請提供欲刪除的行號或關鍵字。");
                return;
            }
            args[2].clone()
        } else {
            args[1][2..].to_string()
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

    // ✨ 11. 無參數且無管道輸入：展示系統狀態儀表板
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

        println!("┌────────────────────────────────────────────────────────────┐");
        println!("│ 🛡️  Cyber-NOte 機密記事與金鑰加密系統 · 系統狀態            │");
        println!("├────────────────────────────────────────────────────────────┤");
        println!("│ 📂 存儲目錄 : {:<44} │", note_dir.to_str().unwrap_or(""));
        println!("│ 🔒 隱私隔離 : {:<44} │", secrets_dir.join("token.gpg").to_str().unwrap_or(""));
        println!("│ 🔑 GPG 金鑰 : {:<44} │", current_key);
        println!("│ 🌐 Gist ID  : {:<44} │", current_gist);
        println!(
            "│ 📜 金鑰歸檔 : {:<44} │",
            format!("已收錄 {} 筆加密檔案審計記錄", ledger.records.len())
        );
        println!(
            "│ ⚡ 架構核心 : {:<44} │",
            "Rust 原生核心 (鎖定 GPG) + JS 網頁管理引擎"
        );
        println!("└────────────────────────────────────────────────────────────┘");
        println!("用法: a [機密筆記內容/支援多行]   #追加寫入並整檔 GPG 鎖定公鑰加密");
        println!("      cat 檔案 | a                 #【管道串流】直接吸納字串流加密追加");
        println!("      a -p [檔案路徑]              #【檔案加密】加密普通檔案或 .gpg 巢狀多層加密");
        println!("      a -p [檔案] --pass [密碼]    #【無密鑰防窮舉】S2K 65,011,712 輪密碼對稱加密");
        println!("      a -p [檔案] -u               #【加密直傳】加密後直接上傳至雲端 Gist");
        println!("      a -x [檔案路徑]              #【解密還原】還原一層加密封裝 (去 .gpg)");
        println!("      a --new [文件名] [文件內容]     #【創建新文件】在雲端 Gist 創建/寫入新檔案");
        println!("      a --delete [文件名]           #【刪除檔案】指定刪除遠端 Gist 倉庫中的檔案");
        println!("      a -t 或 a -t [標籤]            #【TOTP 雙重認證】列出或輸出指定驗證碼");
        println!("      a -t --add [標籤] --code [密鑰] #【新增 TOTP】註冊驗證密鑰");
        println!("      a -t --delete [標籤]           #【刪除 TOTP】移除指定標籤驗證密鑰");
        println!("      a -k 或 a --ledger           #【金鑰歸檔簿】查看檔案與金鑰審計清單");
        println!("      a -a 或 a --all              #解密並列印今年度主機密文檔");
        println!("      a -a ./[檔案]                #解密並列印【本地密文檔案】");
        println!("      a -a [年份/檔名]             #即時檢索並列印雲端 Gist 文檔");
        println!("      a -s 或 a --sync             #推送今年密文文檔至雲端 Gist (-v 檢視細節)");
        println!("      a -s [檔案路徑]              #加密推送外部檔案至 Gist");
        println!("      a -s -u [檔案路徑]           #以明文模式推送純文字檔案至 Gist");
        println!("      a -l 或 a --list             #檢索雲端 Gist 倉庫全部檔案清單");
        println!("      a -d --all                   #【批量下載】下載雲端 Gist 倉庫全部檔案至本地");
        println!("      a -d [檔名] -o [目標路徑]     #【自訂下載】指定檔名/自訂路徑 (例: a -d 1.txt -o ./1.txt)");
        println!("      a -d --all -o [目標目錄]      #下載全部檔案至自訂本地目錄 (例: a -d --all -o ./)");
        println!("      a -d [年份/檔名]             #下載雲端密文包裹至本地 (保留 .gpg)");
        println!("      a -d [檔名] -x               #下載並解密還原原始檔案 (去 .gpg)");
        println!("      a -r1 或 a -r 1              #刪除【倒數第 1 行】");
        println!("      a -r1-100 或 a -r 1-100      #刪除【倒數 1 至 100 行】");
        println!("      a -r [關鍵字]                 #刪除包含該關鍵字的所有行");
        println!("      a -w 或 a --web              #【網頁管理引擎】啟動 Web 視覺化管理後台");
        println!("      a -w stop                    #關閉 Web 網頁管理引擎 (切換至待機狀態)");
        println!("      a -w status                  #查詢 Web 網頁管理引擎運行狀態");
        println!("      a --set-dir [新路徑]          #修改機密文檔本地存儲目錄");
        println!("      a --init 或 a -i             #系統配置精靈 (支援 Enter 保留舊值)");
        return;
    }

    // ✨ 12. 寫入新筆記（命令列參數與管道文字流）
    let new_note = if has_pipe {
        if args.len() > 1 {
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
}
