use std::env;
use chrono::Local;

// 🚢 諸侯引渡：把 a_note 改成你項目真正的集裝箱名字 `a`！
use a::color::{paint_line, TerminalColor};
use a::storage::{append_note, read_note};

fn main() {
    // 1. 收集火車：args 是整條參數火車 [Vec<String>]
    let args: Vec<String> = env::args().collect();
    
    // 2. 獲取今年是哪一年
    let current_year = Local::now().format("%Y").to_string();
    let file_path = format!("/home/lee/BOok/NOte/{}.note", current_year);

    // 3. 安全檢查：如果火車裡只有 1 節車廂（代表玩家只敲了 a，後面啥都沒寫）
    if args.len() < 2 {
        println!("用法: a [您的靈感創意/支援多行貼上]");
        println!("      a -a 或 a --all #列印今年全部筆記內容（隔行換色版）");
        return;
    }

    // 4. ✨ 終極修復：拿火車的【第二節車廂 args[1]】去和單個卡片比對！
    if args[1] == "-a" || args[1] == "--all" {
        // ➔ 把物流標籤 ::<String> 焊在 read_note 函數體的屁股後面！
        if let Ok(content) = read_note(&file_path) { 
            // 完美解包！content 被安全鎖定為 String，流水線暢通無阻！
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

    // 5. 打包多行文字：把第二節車廂之後的所有文字用空格連起來
    let note_content = args[1..].join(" ");

    // 6. 物理追加寫入硬碟
    if append_note(&file_path, &note_content).is_ok() {
        println!("✨ 靈感已安全送達外部諸侯廠房！");
    }
}

