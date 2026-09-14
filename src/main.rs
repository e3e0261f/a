// src/main.rs
// Cyber-NOte 機密記事與金鑰加密系統 (Project a)
// 核心架構：嚴格鎖定 GPG 金鑰隔離體系（嚴禁 SSH 金鑰混用）、多層巢狀加密封裝、高迭代 S2K 防窮舉加固與金鑰歸檔簿審計。

use chrono::Local;
use std::env;
use std::fs;
use std::io::{self, IsTerminal, Read, Write};
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
                let key_file = note_dir.join("key_id");
                let _ = fs::write(&key_file, &key_id);
                println!("  ↳ 🔑 GPG 金鑰已鎖定並存檔: {:?}", key_file);
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
        let gist_file = note_dir.join("gist_id");
        let _ = fs::write(&gist_file, &clean_gist_id);
        println!(
            "  ↳ 🌐 Gist ID [{}] 已存檔: {:?}",
            clean_gist_id, gist_file
        );
    }

    // 4. Token 憑證配置 (隱私數據隔離至 ~/.config/cyber-note/secrets/token.gpg)
    println!("\n--- [步驟 3/3: GitHub 存取憑證封裝與隱私隔離] ---");
    let secrets_dir = GameConfig::get_secrets_dir();
    let secret_token_file = secrets_dir.join("token.gpg");
    let legacy_token_file = note_dir.join("token.gpg");
    let has_token = secret_token_file.exists() || legacy_token_file.exists();
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
                    // 寫入隔離目錄 (~/.config/cyber-note/secrets/token.gpg)
                    if fs::write(&secret_token_file, encrypted_token.as_bytes()).is_ok() {
                        #[cfg(unix)]
                        {
                            use std::os::unix::fs::PermissionsExt;
                            let _ = fs::set_permissions(&secret_token_file, fs::Permissions::from_mode(0o600));
                        }
                        // 亦備份一份至 note_dir 維持舊工具相容
                        let _ = fs::write(&legacy_token_file, encrypted_token.as_bytes());

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

// 🚀 完全刪除/抹除倉庫修改歷史記錄：創建全新 Gist 倉庫並遷移
fn handle_migrate_repo_command(args: &[String], verbose: bool) {
    println!("\n╔══════════════════════════════════════════════════════════════╗");
    println!("║       🚀 Cyber-NOte · 無痕抹除歷史與新倉庫無縫遷移           ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!("⚠️  【安全審查說明】");
    println!("   GitHub Gist 會保留每一次編輯修訂的完整歷史紀錄。");
    println!("   若要徹底銷毀、抹除以往所有版本的修訂歷史，必須建立全新 Gist，");
    println!("   將本地所有機密文檔遷移後，重定向本地倉庫 ID 並銷毀舊倉庫！\n");

    let delete_old = args.iter().any(|a| a == "--delete-old" || a == "-d");

    let token = match get_github_token(verbose) {
        Ok(t) => t,
        Err(e) => {
            println!("❌ 無法取得 GitHub Token: {}", e);
            println!("💡 請先執行 'a --init' 配置並封裝 GPG 憑證。");
            return;
        }
    };

    let note_dir = GameConfig::get_note_dir();
    let old_gist_id = GameConfig::get_gist_url().ok().and_then(|url| {
        url.split('/').last().map(|s| s.to_string())
    }).unwrap_or_default();

    println!("📂 正在掃描本地加密倉庫目錄: {:?}", note_dir);
    let mut files_to_migrate = std::collections::HashMap::new();

    if let Ok(entries) = fs::read_dir(&note_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                let fname = path.file_name().and_then(|n| n.to_str()).unwrap_or_default();
                // 排除本地設定與敏感憑證檔，只遷移加密筆記與文檔
                if fname != "gist_id" && fname != "key_id" && fname != "token.gpg" && fname != "dir" {
                    if let Ok(content) = fs::read_to_string(&path) {
                        files_to_migrate.insert(fname.to_string(), content);
                        println!(
                            "  📦 已裝箱待遷移檔案: {} ({:.2} KB)",
                            fname,
                            path.metadata().map(|m| m.len()).unwrap_or(0) as f64 / 1024.0
                        );
                    }
                }
            }
        }
    }

    if files_to_migrate.is_empty() {
        println!("ℹ️  本地倉庫暫無額外文檔，將初始化乾淨保險庫基準檔。");
    }

    println!("\n🚀 正在向 GitHub 發起乾淨新倉庫建立請求 (歷史版本計數將徹底歸零)...");
    let desc = format!("Cyber-NOte Vault [Clean Slate - Purged History @ {}]", Local::now().format("%Y-%m-%d %H:%M:%S"));
    match create_clean_slate_gist(&files_to_migrate, &desc, &token, false, verbose) {
        Ok(new_gist_id) => {
            println!("✅ 全新 Gist 倉庫創建成功！");
            println!("  🆕 新倉庫 ID : {}", new_gist_id);
            if !old_gist_id.is_empty() {
                println!("  🏛️  舊倉庫 ID : {}", old_gist_id);
            }

            // 更新本地 gist_id
            let gist_file = note_dir.join("gist_id");
            let _ = fs::write(&gist_file, &new_gist_id);
            let app_gist = GameConfig::get_app_config_dir().join("gist_id");
            let _ = fs::write(&app_gist, &new_gist_id);
            println!("  🎯 本地同步管道已自動重新錨定至新倉庫！");

            if delete_old && !old_gist_id.is_empty() && old_gist_id != new_gist_id {
                println!("🗑️  正在銷毀帶有舊修改歷史記錄的舊倉庫: {}...", old_gist_id);
                match delete_gist(&old_gist_id, &token, verbose) {
                    Ok(_) => println!("  💥 舊倉庫已在 GitHub 上徹底銷毀，歷史編輯紀錄完全抹除！"),
                    Err(e) => println!("  ⚠️  舊倉庫刪除失敗 (請確認 Token 具備 gist 刪除權限): {}", e),
                }
            } else if !old_gist_id.is_empty() {
                println!("💡 提示：若需在 GitHub 上徹底刪除舊倉庫，可加上 --delete-old 參數。");
            }

            println!("\n✨ 遷移完成！歷史修訂痕跡已完全抹除切斷。");
        }
        Err(e) => {
            println!("❌ 創建新倉庫失敗: {}", e);
        }
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

// 🌐 網頁端管理引擎控制邏輯 (Cyber-NOte Web Engine)
fn handle_web_command(sub_action: Option<&str>, port_opt: Option<&str>) {
    let port = port_opt.unwrap_or("3000");
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
            let state = fs::read_to_string(&state_file).unwrap_or_else(|_| "standby".to_string());
            let is_active = state.trim() == "active"
                && std::net::TcpStream::connect(format!("127.0.0.1:{}", port)).is_ok();

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
                "│ 安全體系 : {:<47} │",
                "Rust 原生核心 (鎖定 GPG) + JS 網頁管理引擎"
            );
            println!("└────────────────────────────────────────────────────────────┘");
            if !is_active {
                println!("💡 說明：網頁端為可選元件，預設處於待機關閉狀態。");
                println!("👉 若要啟動網頁端管理介面，請執行: a --web");
            } else {
                println!("👉 若要關閉網頁端管理介面，請執行: a --web stop");
            }
        }
        _ => {
            let _ = fs::write(&state_file, "active");

            println!("\n╔══════════════════════════════════════════════════════════════╗");
            println!("║          🛡️  Cyber-NOte 系統 · Web 網頁端管理引擎            ║");
            println!("╚══════════════════════════════════════════════════════════════╝");
            println!("🚀 Web 網頁端管理後台已喚醒！");
            println!("🌐 存取位址: http://localhost:{}", port);
            println!("📊 架構核心: Rust 原生後台 (鎖定 GPG 安全審計) + JS 網頁管理引擎");
            println!("💡 提示: 執行 'a --web stop' 可將網頁端切換回待機狀態。\n");

            let dev_server_running =
                std::net::TcpStream::connect(format!("127.0.0.1:{}", port)).is_ok();

            if !dev_server_running {
                // 依賴檢測: 驗證 tsx 與 express 是否可用
                let has_tsx = std::process::Command::new("which")
                    .arg("tsx")
                    .output()
                    .map(|o| o.status.success())
                    .unwrap_or(false)
                    || std::process::Command::new("npx")
                        .args(["tsx", "--version"])
                        .output()
                        .map(|o| o.status.success())
                        .unwrap_or(false);

                let has_express = std::process::Command::new("node")
                    .args(["-e", "require('express')"])
                    .output()
                    .map(|o| o.status.success())
                    .unwrap_or(false);

                if !has_tsx || !has_express {
                    print_web_dependencies_guide(!has_tsx, !has_express);
                }

                println!("⏳ 正在載入 JS 網頁管理引擎服務...");
                let child = std::process::Command::new("npm")
                    .args(["run", "dev"])
                    .stdout(std::process::Stdio::inherit())
                    .stderr(std::process::Stdio::inherit())
                    .spawn();

                match child {
                    Ok(mut proc) => {
                        let pid = proc.id();
                        let _ = fs::write(&pid_file, pid.to_string());
                        println!("✨ JS 網頁端管理引擎已啟動 (PID: {})！", pid);
                        let _ = std::process::Command::new("xdg-open")
                            .arg(format!("http://localhost:{}", port))
                            .stdout(std::process::Stdio::null())
                            .stderr(std::process::Stdio::null())
                            .spawn();
                        let _ = proc.wait();
                        let _ = fs::remove_file(&pid_file);
                    }
                    Err(e) => {
                        println!("❌ 啟動失敗: {}", e);
                        print_web_dependencies_guide(true, true);
                    }
                }
            } else {
                println!("🟢 JS 網頁端管理引擎目前已在埠號 {} 正常運行中！", port);
                let _ = std::process::Command::new("xdg-open")
                    .arg(format!("http://localhost:{}", port))
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .spawn();
            }
        }
    }
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

    // ✨ 2.5 倉庫修改歷史記錄抹除與全新遷移 (--migrate-repo / --clean-slate / --new-repo)
    if args.len() > 1
        && (args[1] == "--migrate-repo"
            || args[1] == "--clean-slate"
            || args[1] == "--new-repo")
    {
        handle_migrate_repo_command(&args, verbose);
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
        let custom_path_opt = args.iter().skip(2).find(|&arg| {
            arg != "--raw" && arg != "-u" && arg != "-v" && arg != "-vv" && arg != "--verbose"
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

    // ✨ 9. 雲端下載與自動解密還原 (-d / -x / --decrypt)
    if args.len() > 1 && (args[1] == "-d" || args[1] == "--download") {
        let should_decrypt = args.iter().any(|arg| arg == "-x" || arg == "--decrypt");
        let raw_target_opt = args.iter().skip(2).find(|&a| {
            a != "-x" && a != "--decrypt" && a != "-v" && a != "-vv" && a != "--verbose"
        });

        let raw_target = match raw_target_opt {
            Some(t) => t.clone(),
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

        let target_local_path = note_dir.join(&local_file_name);
        let target_local_str = target_local_path.to_str().unwrap();

        match get_github_token(verbose) {
            Ok(token) => {
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
                                        println!("⚠️ 寫入本地磁碟失敗");
                                    }
                                }
                                Err(e) => println!("⚠️ 解密失敗: {}", e),
                            }
                        } else {
                            if fs::write(&target_local_path, &encrypted_content).is_ok() {
                                println!("✨ 密文包裹已成功下載至: {}", target_local_str);
                            } else {
                                println!("⚠️ 寫入本地磁碟失敗");
                            }
                        }
                    }
                    Err(e) => println!("⚠️ 下載失敗: {}", e),
                }
            }
            Err(e) => println!("❌ 錯誤：{}", e),
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
        println!("      a --migrate-repo [--delete-old] #【抹除歷史】建立全新 Gist 倉庫徹底銷毀舊歷史版本");
        println!("      a -k 或 a --ledger           #【金鑰歸檔簿】查看檔案與金鑰審計清單");
        println!("      a -a 或 a --all              #解密並列印今年度主機密文檔");
        println!("      a -a ./[檔案]                #解密並列印【本地密文檔案】");
        println!("      a -a [年份/檔名]             #即時檢索並列印雲端 Gist 文檔");
        println!("      a -s 或 a --sync             #推送今年密文文檔至雲端 Gist (-v 檢視細節)");
        println!("      a -s [檔案路徑]              #加密推送外部檔案至 Gist");
        println!("      a -s -u [檔案路徑]           #以明文模式推送純文字檔案至 Gist");
        println!("      a -l 或 a --list             #檢索雲端 Gist 倉庫全部檔案清單");
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
