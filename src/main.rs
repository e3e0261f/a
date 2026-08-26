use std::env;
use chrono::Local;

// 🚢 諸侯引渡
use a::{GameConfig, color::{paint_line, TerminalColor}, storage::{append_note, read_note}};
use a::encrypt::encrypt_with_gpg;
use a::gist::sync_to_gist;

fn main() {
    // 1. 收集終端傳過來的參數大火車 [Vec<String>]
    let args: Vec<String> = env::args().collect();
    
    // 2. 獲取今年是哪一年
    let current_year = Local::now().format("%Y").to_string();
    
    // 🏛️ 直接從 lib.rs 的總法律常數裡提取總路徑
    let file_path = format!("{}/{}.note", GameConfig::NOTE_DIR, current_year);

    // 3. 安全檢查：如果火車裡只有 1 節車廂（代表沒寫筆記內容，也沒帶 -a / -s）
    if args.len() < 2 {
        println!("用法: a [您的靈感創意/支援多行貼上] #只寫入本地，極速流暢");
        println!("      a -a 或 a --all           #列印今年全部本地內容");
        println!("      a -s 或 a --sync          #主動觸發 GPG 加密並同步至雲端");
        return;
    }

    // 4. ✨ 終極修復一：拿火車的【第二節車廂 args[1]】去和單個卡片比對！
    if args[1] == "-a" || args[1] == "--all" {
        if let Ok(content) = read_note(&file_path) { 
            for (index, line) in content.lines().enumerate() {
                if index % 2 == 0 {
                    paint_line(line, TerminalColor::Green); // 🟢 偶數行
                } else {
                    paint_line(line, TerminalColor::Cyan);  // 🔵 奇數行
                }
            }
        } else {
            println!("📂 今年還沒有任何靈感記錄哦！");
        }
        return;
    }

    // 5. ✨ 終极修復二：同樣拿火車的【第二節車厢 args[1]】去比對同步指令！
    if args[1] == "-s" || args[1] == "--sync" {
        if let Ok(full_content) = read_note(&file_path) {
            // 從 Fish 系統外殼索要鑰匙
            if let Ok(token) = env::var("GITHUB_GIST_TOKEN") {
                println!("🔐 [主動同步] 正在調用您的 GPG 密鑰進行高維時空加密...");
                
                match encrypt_with_gpg(&full_content, GameConfig::GPG_USER_ID) {
                    Ok(encrypted_content) => {
                        println!("🔄 [主動同步] 正在將【GPG加密暗號包裹】發射至 GitHub Gist...");
                        
                        // 🎨 實現你的宏大戰略格局：以當前年份命名！
                        // 2026 年就叫 2026.note.gpg，2027 年就自動生成 2027.note.gpg！
                        let file_name = format!("{}.note.gpg", current_year);
                        
                        // 乾乾淨淨地只遞 3 個參數進去，不拆不組，一步到位！
                        match sync_to_gist(&encrypted_content, &file_name, &token) {
                            Ok(_) => println!("☁️  [GitHub] GPG加密雲端備份同步完美全線打通！"),
                            Err(e) => println!("⚠️  [GitHub] 傳輸失敗: {}", e),
                        }
                    },
                    Err(e) => println!("⚠️  [保密局] GPG加密失敗: {}", e),
                }
            } else {
                println!("❌ 錯誤：未檢測到系統環境變數 GITHUB_GIST_TOKEN，無法同步。");
            }
        } else {
            println!("📂 本地空空如也，沒有什麼好同步的。");
        }
        return;
    }

    // ✨ 核心對齊：如果是普通打字，先把第二節車廂之後的所有參數打包成物質！
    let note_content = args[1..].join(" ");

    // 6. 預設流水線：普通輸入 ➔ 【只追加寫入本地硬碟，絕不上傳】
    if append_note(&file_path, &note_content).is_ok() {
        println!("✨ 靈感已安全鎖進本地 {} 廠房！", current_year);
    }
}
