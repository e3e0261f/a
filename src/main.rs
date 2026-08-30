// src/main.rs
use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::Path;
use std::time::Instant;
use chrono::Local;

// 🚢 諸侯引渡
use a::{GameConfig, color::{paint_line, TerminalColor}, storage::{write_encrypted_note, read_note}};
use a::encrypt::{encrypt_with_gpg, decrypt_with_gpg, decrypt_bytes_with_gpg};
use a::gist::{sync_to_gist, fetch_from_gist, list_gist_files};

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

// 🧙‍♂️ 智慧互動式引導精靈（自動回填現有值，按 Enter 保留）
fn run_init_wizard() {
    println!("\n╔══════════════════════════════════════════════════════════════╗");
    println!("║       🛡️  Cyber-Forge 賽博靈感管家 · 配置引導精靈        ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!("提示: 直接按下 Enter 可保留括號中的 [預設值/現有配置]。\n");

    let current_dir = GameConfig::get_note_dir();
    let current_dir_str = current_dir.to_str().unwrap_or("~/BOok/NOte");

    // 1. 目錄配置
    let note_dir_input = prompt_input("請確認/指定筆記本地存儲目錄", Some(current_dir_str));
    let note_dir = match GameConfig::set_persistent_dir(&note_dir_input) {
        Ok(d) => {
            println!("  ↳ 📂 存儲目錄已錨定: {:?}", d);
            d
        },
        Err(e) => {
            println!("  ⚠️ 寫入目錄失敗 ({})，使用原目錄", e);
            current_dir
        }
    };

    // 2. GPG 金鑰配置
    println!("\n--- [步驟 1/3: GPG 金鑰配置] ---");
    let existing_key = GameConfig::get_gpg_user_id().unwrap_or_default();
    let key_prompt_default = if existing_key.is_empty() { None } else { Some(existing_key.as_str()) };
    let key_id = prompt_input("請輸入 GPG 金鑰標識 (指紋/子金鑰ID/郵箱)", key_prompt_default);
    
    if !key_id.is_empty() {
        let key_file = note_dir.join("key_id");
        let _ = fs::write(&key_file, &key_id);
        println!("  ↳ 🔑 GPG 金鑰已保存至: {:?}", key_file);
    }

    // 3. Gist ID 配置
    println!("\n--- [步驟 2/3: 雲端 Gist 倉庫配置] ---");
    let existing_gist = GameConfig::get_gist_id().unwrap_or_default();
    let gist_prompt_default = if existing_gist.is_empty() { None } else { Some(existing_gist.as_str()) };
    let raw_gist = prompt_input("請輸入 Gist ID 或 URL", gist_prompt_default);
    let clean_gist_id = GameConfig::extract_clean_id(&raw_gist);

    if !clean_gist_id.is_empty() {
        let gist_file = note_dir.join("gist_id");
        let _ = fs::write(&gist_file, &clean_gist_id);
        println!("  ↳ 🌐 Gist ID [{}] 已保存至: {:?}", clean_gist_id, gist_file);
    }

    // 4. Token 憑證配置
    println!("\n--- [步驟 3/3: GitHub Token 憑證加密] ---");
    let token_file = note_dir.join("token.gpg");
    let has_token = token_file.exists();
    let token_default = if has_token { Some("保留現有加密憑證") } else { None };
    let token_input = prompt_input("請輸入 GitHub Personal Access Token", token_default);

    if token_input != "保留現有加密憑證" && !token_input.is_empty() {
        let active_key = if !key_id.is_empty() { key_id } else { existing_key };
        if active_key.is_empty() {
            println!("  ❌ 錯誤：未指定 GPG 金鑰，無法加密 Token");
        } else {
            print!("  ↳ 🔐 正在調用 GPG 密鑰 [{}] 封裝 token.gpg...", active_key);
            io::stdout().flush().unwrap();
            match encrypt_with_gpg(token_input.as_bytes(), &active_key) {
                Ok(encrypted_token) => {
                    if fs::write(&token_file, encrypted_token).is_ok() {
                        println!(" [成功]");
                        println!("  ↳ 🛡️ 憑證已安全加密落盤: {:?}", token_file);
                    }
                },
                Err(e) => println!(" [失敗: {}]", e),
            }
        }
    } else if has_token {
        println!("  ↳ 🛡️ 保持現有 token.gpg 不變。");
    }

    println!("\n✨ 配置更新圓滿完成！\n");
}

fn get_github_token(verbose: bool) -> Result<String, String> {
    let note_dir = GameConfig::get_note_dir();
    let token_path = note_dir.join("token.gpg");
    
    if verbose {
        println!("  🔑 [憑證] 正在讀取並解密本地 Token: {:?}", token_path);
    }

    let encrypted_token = fs::read_to_string(&token_path)
        .map_err(|e| format!("無法讀取加密 Token 檔 ({:?}): 請先執行 'a --init' 初始化 (底層錯誤: {})", token_path, e))?;
    
    let decrypted_token = decrypt_with_gpg(&encrypted_token)
        .map_err(|e| format!("解密 Token 失敗（請確認 GPG 私鑰已解鎖）: {}", e))?;
    
    let token = decrypted_token.trim().to_string();
    if token.is_empty() {
        Err("解密後的 Token 內容為空".to_string())
    } else {
        if verbose {
            println!("  ✅ [憑證] Token 解密成功 (長度: {} 字元)", token.len());
        }
        Ok(token)
    }
}

fn print_content_colored(raw_content: &str) {
    let printable_content = if raw_content.contains("-----BEGIN PGP MESSAGE-----") {
        match decrypt_with_gpg(raw_content) {
            Ok(dec) => dec,
            Err(e) => {
                println!("⚠️  [保密局] 解密失敗（可能需要輸入私鑰密碼）: {}", e);
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

fn main() {
    let args: Vec<String> = env::args().collect();
    let current_year = Local::now().format("%Y").to_string();
    let note_dir = GameConfig::get_note_dir();
    let default_file_path = note_dir.join(format!("{}.note.gpg", current_year));
    let default_file_str = default_file_path.to_str().unwrap();

    let verbose = args.iter().any(|arg| arg == "-v" || arg == "-vv" || arg == "--verbose");

    // ✨ 1. 單獨快速修改目錄：a --set-dir [路徑] 或 a -dir [路徑]
    if args.len() > 1 && (args[1] == "--set-dir" || args[1] == "-dir" || args[1] == "--dir") {
        if args.len() < 3 {
            println!("❌ 錯誤：請提供目標目錄路徑。範例: a --set-dir ~/MyNotes");
            return;
        }
        let target_dir = &args[2];
        match GameConfig::set_persistent_dir(target_dir) {
            Ok(p) => println!("✨ 筆記存儲目錄已成功切換為: {:?}", p),
            Err(e) => println!("❌ 切換目錄失敗: {}", e),
        }
        return;
    }

    // ✨ 2. 手動觸發配置精靈
    if args.len() > 1 && (args[1] == "--init" || args[1] == "-i" || args[1] == "init") {
        run_init_wizard();
        return;
    }

    // ✨ 3. 單獨輸入 a：展示當前環境儀表板與用法指南
    if args.len() < 2 {
        if !GameConfig::is_configured() {
            println!("👋 檢測到系統尚未完成基礎設定，正在啟動初始化引導...");
            run_init_wizard();
            return;
        }

        // 儀表板面板
        let current_key = GameConfig::get_gpg_user_id().unwrap_or_else(|_| "未配置".to_string());
        let current_gist = GameConfig::get_gist_id().unwrap_or_else(|_| "未配置".to_string());
        
        println!("┌────────────────────────────────────────────────────────────┐");
        println!("│ 🛡️  Cyber-Forge 賽博靈感管家 · 系統儀表板                   │");
        println!("├────────────────────────────────────────────────────────────┤");
        println!("│ 📂 存儲目錄 : {:<44} │", note_dir.to_str().unwrap_or(""));
        println!("│ 🔑 GPG 金鑰 : {:<44} │", current_key);
        println!("│ 🌐 Gist ID  : {:<44} │", current_gist);
        println!("└────────────────────────────────────────────────────────────┘");
        println!("用法: a [您的靈感創意/支援多行貼上] #自動解密拼接並整檔公鑰加密");
        println!("      a -a 或 a --all           #解密並列印今年本地筆記");
        println!("      a -a ./[檔案]              #解密並列印【本地檔案】");
        println!("      a -a [年份/檔名]           #【直連雲端】即時解密並列印遠端檔案");
        println!("      a -s 或 a --sync          #推送今年加密筆記至雲端 Gist (-v 檢視細節)");
        println!("      a -s [檔案路徑]            #【二進位/文字通用】加密推送外部檔案至 Gist");
        println!("      a -s -u [檔案路徑]         #【明文/不加密】推送純文字檔案至 Gist");
        println!("      a -l 或 a --list          #列出雲端 Gist 上存有的全部檔案清單");
        println!("      a -d [年份/檔名]           #下載雲端密文檔至本地 (保留 .gpg 密文)");
        println!("      a -d [檔名] -x 或 --decrypt #【下載並解密還原】原始檔案 (去 .gpg 後綴)");
        println!("      a -r1 或 a -r 1           #刪除【倒數第 1 行】");
        println!("      a -r1-100 或 a -r 1-100   #刪除【倒數 1 至 100 行】");
        println!("      a -r [關鍵字]              #刪除包含該關鍵字的所有行");
        println!("      a --set-dir [新路徑]       #【單獨修改】筆記本地存儲目錄");
        println!("      a --init 或 a -i          #配置引導精靈 (支援 Enter 保留舊值)");
        return;
    }

    // ✨ 4. 查看本地與雲端閱覽
    if args[1] == "-a" || args[1] == "--all" {
        if args.len() == 2 || (args.len() == 3 && verbose) {
            if let Ok(raw_content) = read_note(default_file_str) {
                print_content_colored(&raw_content);
            } else {
                println!("📂 今年本地還沒有任何靈感記錄哦！");
            }
        } else {
            let target_input = args.iter().skip(2).find(|&a| a != "-v" && a != "-vv" && a != "--verbose").unwrap();

            if target_input.starts_with("./") || target_input.starts_with("../") || target_input.starts_with("/") {
                let local_path = Path::new(target_input);
                if let Ok(raw_content) = fs::read_to_string(local_path) {
                    print_content_colored(&raw_content);
                } else {
                    println!("📂 本地找不到指定的檔案或為非純文字檔案：{}", target_input);
                }
            } else {
                let remote_filename = if target_input.len() == 4 && target_input.chars().all(|c| c.is_ascii_digit()) {
                    format!("{}.note.gpg", target_input)
                } else {
                    target_input.clone()
                };

                match get_github_token(verbose) {
                    Ok(token) => {
                        println!("☁️  [雲端雷達] 正在從 Gist 即時串流獲取【{}】...", remote_filename);
                        match fetch_from_gist(&remote_filename, &token, verbose) {
                            Ok(remote_content) => {
                                print_content_colored(&remote_content);
                            },
                            Err(e) => println!("⚠️ 雲端獲取失敗: {}", e),
                        }
                    },
                    Err(e) => println!("❌ 錯誤：{}", e),
                }
            }
        }
        return;
    }

    // ✨ 5. 雲端同步
    if args[1] == "-s" || args[1] == "--sync" {
        let timer = Instant::now();
        let is_raw = args.iter().any(|arg| arg == "--raw" || arg == "-u");
        let custom_path_opt = args.iter().skip(2).find(|&arg| arg != "--raw" && arg != "-u" && arg != "-v" && arg != "-vv" && arg != "--verbose");

        let (payload_to_send, remote_filename) = if let Some(custom_path) = custom_path_opt {
            let path_obj = Path::new(custom_path);

            if !path_obj.exists() {
                println!("❌ 錯誤：找不到指定的自訂檔案路徑 -> {}", custom_path);
                return;
            }

            println!("📦 [1/4 讀取] 正在讀取本地檔案【{}】...", custom_path);
            let custom_bytes = match fs::read(custom_path) {
                Ok(bytes) => {
                    println!("  ↳ 檔案讀取完畢，原始大小: {:.2} KB ({} Bytes)", bytes.len() as f64 / 1024.0, bytes.len());
                    bytes
                },
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
                        println!("📄 [2/4 模式] 以【明文直傳】模式打包【{}】...", file_stem);
                        (valid_text, file_stem.to_string())
                    },
                    Err(_) => {
                        println!("❌ 錯誤：該二進位檔案包含非 UTF-8 資料，無法以明文模式傳輸至 Gist。請去除 -u 旗標以 GPG 加密模式上傳！");
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

                println!("🔐 [2/4 加密] 正在調用 GPG (密鑰: {}) 封裝為 ASCII Armor 密文...", gpg_user_id);
                let gpg_start = Instant::now();
                let encrypted = match encrypt_with_gpg(&custom_bytes, &gpg_user_id) {
                    Ok(c) => {
                        println!("  ↳ GPG 封裝完成，耗時: {:?}，密文體積: {:.2} KB", gpg_start.elapsed(), c.len() as f64 / 1024.0);
                        c
                    },
                    Err(e) => {
                        println!("⚠️ 加密外部檔案失敗: {}", e);
                        return;
                    }
                };
                let remote_name = format!("{}.gpg", file_stem);
                (encrypted, remote_name)
            }
        } else {
            println!("📦 [1/4 讀取] 正在讀取本地年度筆記【{}】...", default_file_str);
            let encrypted_content = match read_note(default_file_str) {
                Ok(content) => {
                    println!("  ↳ 本地密文包裹讀取完畢 (體積: {:.2} KB)", content.len() as f64 / 1024.0);
                    content
                },
                Err(_) => {
                    println!("📂 本地空空如也，沒有什麼好同步的。");
                    return;
                }
            };
            let remote_name = format!("{}.note.gpg", current_year);
            (encrypted_content, remote_name)
        };

        println!("🔑 [3/4 提領] 正在取得通行證並校驗授權...");
        match get_github_token(verbose) {
            Ok(token) => {
                println!("🚀 [4/4 出海] 正在向 GitHub Gist 發射【{}】(超時保護: 30s)...", remote_filename);
                match sync_to_gist(&payload_to_send, &remote_filename, &token, verbose) {
                    Ok(_) => println!("☁️  [GitHub] 同步成功！全流程總耗時: {:?}", timer.elapsed()),
                    Err(e) => println!("⚠️  [GitHub] 傳輸失敗: {}", e),
                }
            },
            Err(e) => println!("❌ 錯誤：{}", e),
        }
        return;
    }

    // ✨ 6. 列出雲端檔案
    if args[1] == "-l" || args[1] == "--list" {
        match get_github_token(verbose) {
            Ok(token) => {
                println!("📡 [雲端雷達] 正在掃描 GitHub Gist 倉庫物資清單...");
                match list_gist_files(&token, verbose) {
                    Ok(files) => {
                        println!("📋 雲端現有密文包裹清單 (共 {} 個)：", files.len());
                        println!("------------------------------------");
                        for file in files {
                            paint_line(&format!("📦 {}", file), TerminalColor::Cyan);
                        }
                        println!("------------------------------------");
                        println!("💡 可使用 'a -d [檔名]' 下載，或 'a -d [檔名] -x' 下載並解密還原。");
                    },
                    Err(e) => println!("⚠️ 獲取清單失敗: {}", e),
                }
            },
            Err(e) => println!("❌ 錯誤：{}", e),
        }
        return;
    }

    // ✨ 7. 雲端下載與自動解密還原
    if args[1] == "-d" || args[1] == "--download" {
        let should_decrypt = args.iter().any(|arg| arg == "-x" || arg == "--decrypt");
        let raw_target_opt = args.iter().skip(2).find(|&a| a != "-x" && a != "--decrypt" && a != "-v" && a != "-vv" && a != "--verbose");
        
        let raw_target = match raw_target_opt {
            Some(t) => t.clone(),
            None => current_year.clone(),
        };

        let remote_file_name = if raw_target.len() == 4 && raw_target.chars().all(|c| c.is_ascii_digit()) {
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
                println!("☁️  [雲端雷達] 正在從 Gist 索取【{}】...", remote_file_name);
                match fetch_from_gist(&remote_file_name, &token, verbose) {
                    Ok(encrypted_content) => {
                        if should_decrypt {
                            println!("🔓 [保密局] 正在調用 GPG 私鑰進行破甲解密還原...");
                            match decrypt_bytes_with_gpg(&encrypted_content) {
                                Ok(decrypted_bytes) => {
                                    if fs::write(&target_local_path, &decrypted_bytes).is_ok() {
                                        println!("✨ 原始實體已成功破甲還原至本地：{} (大小: {:.2} KB)", target_local_str, decrypted_bytes.len() as f64 / 1024.0);
                                    } else {
                                        println!("⚠️ 寫入本地磁碟失敗");
                                    }
                                },
                                Err(e) => println!("⚠️  [保密局] 解密還原失敗: {}", e),
                            }
                        } else {
                            if fs::write(&target_local_path, &encrypted_content).is_ok() {
                                println!("✨ 密文包裹已成功下載至本地廠房：{}", target_local_str);
                            } else {
                                println!("⚠️ 寫入本地磁碟失敗");
                            }
                        }
                    },
                    Err(e) => println!("⚠️ 下載失敗: {}", e),
                }
            },
            Err(e) => println!("❌ 錯誤：{}", e),
        }
        return;
    }

    // ✨ 8. 行級刪除
    let is_remove_cmd = args[1].starts_with("-r") || args[1] == "--remove";
    if is_remove_cmd {
        let target_expr = if args[1] == "-r" || args[1] == "--remove" {
            if args.len() < 3 {
                println!("❌ 錯誤：請指定要刪除的倒數行號、區間或關鍵字。範例: a -r1, a -r1-5, a -r 買咖啡");
                return;
            }
            args[2].clone()
        } else {
            args[1][2..].to_string()
        };

        let encrypted_old = match read_note(default_file_str) {
            Ok(content) => content,
            Err(_) => {
                println!("📂 本地沒有找到任何筆記檔案。");
                return;
            }
        };

        let decrypted_old = match decrypt_with_gpg(&encrypted_old) {
            Ok(content) => content,
            Err(e) => {
                println!("⚠️  [保密局] 解密失敗: {}", e);
                return;
            }
        };

        let mut lines: Vec<String> = decrypted_old.lines().map(|s| s.to_string()).collect();
        let total_lines = lines.len();

        if total_lines == 0 {
            println!("📂 筆記內容本就為空，無需刪除。");
            return;
        }

        let is_range = target_expr.contains('-') && {
            let parts: Vec<&str> = target_expr.split('-').collect();
            parts.len() == 2 && parts[0].parse::<usize>().is_ok() && parts[1].parse::<usize>().is_ok()
        };

        let single_num_opt = target_expr.parse::<usize>().ok();

        if is_range {
            let parts: Vec<&str> = target_expr.split('-').collect();
            let start = parts[0].parse::<usize>().unwrap();
            let end = parts[1].parse::<usize>().unwrap();

            let (min_k, max_k) = if start <= end { (start, end) } else { (end, start) };

            if min_k == 0 {
                println!("❌ 錯誤：倒數行號從 1 開始計算（1 為最新一行）。");
                return;
            }

            let start_idx = if max_k >= total_lines { 0 } else { total_lines - max_k };
            let end_idx = if min_k > total_lines {
                0
            } else {
                total_lines - min_k
            };

            if start_idx <= end_idx && start_idx < total_lines {
                let remove_count = (end_idx - start_idx + 1).min(total_lines);
                lines.drain(start_idx..=end_idx);
                println!("✨ 已成功刪除倒數 {} 至 {} 行（共刪除 {} 行）！", min_k, max_k, remove_count);
            } else {
                println!("⚠️ 指定的倒數區間超出筆記總行數（目前共 {} 行）。", total_lines);
                return;
            }
        } else if let Some(k) = single_num_opt {
            if k == 0 {
                println!("❌ 錯誤：倒數行號從 1 開始計算（1 為最新一行）。");
                return;
            }

            if k > total_lines {
                println!("⚠️ 筆記僅有 {} 行，無法刪除倒數第 {} 行。", total_lines, k);
                return;
            }

            let target_idx = total_lines - k;
            let removed_text = lines.remove(target_idx);
            println!("✨ 已成功刪除倒數第 {} 行：{}", k, removed_text);
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
                println!("🔍 未找到包含「{}」的筆記行。", keyword);
                return;
            }

            lines = new_lines;
            println!("✨ 已成功刪除 {} 行包含「{}」的記錄！", removed_count, keyword);
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
                    println!("🔒 已完成本地單一密文封存。");
                }
            },
            Err(e) => println!("⚠️ 全局公鑰加密失敗: {}", e),
        }
        return;
    }

    // ✨ 9. 寫入新筆記
    let new_note = args[1..].join(" ");
    
    let mut existing_content = String::new();
    if let Ok(encrypted_old) = read_note(default_file_str) {
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
                println!("✨ 靈感已安全縫合並以【單一GPG密文包裹】加密封存於本地 {} 廠房！", current_year);
            }
        },
        Err(e) => println!("⚠️ 全局公鑰加密失敗: {}", e),
    }
}