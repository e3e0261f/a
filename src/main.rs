use std::env;
use std::fs::{OpenOptions, read_to_string};
use std::io::Write;
use chrono::Local; // ⏳ 引入時間箭頭（需要 cargo add chrono）

fn main() {
    // 1. 進入貨物口：抓取終端輸入的所有參數（大型活動廠房）
    let args: Vec<String> = env::args().collect();
    
    // 2. 獲取今年是哪一年（比如 2026）
    let current_year = Local::now().format("%Y").to_string();
    let file_path = format!("/home/lee/BOok/NOte/{}.note", current_year);

    // 3. 發動火眼金睛判定：如果沒有傳參數，或者傳了幫助提示
    if args.len() < 2 {
        println!("用法: a [您的靈感創意/支援多行貼上]");
        println!("      a -a 或 a --all #列印今年全部筆記內容");
        return;
    }

    // 4. 單選題判定：是不是要看全部內容？
    if args[1] == "-a" || args[1] == "--all" {
        if let Ok(content) = read_to_string(&file_path) {
            print!("{}", content); // 🚀 完美原樣列印，格式絕對不亂
        } else {
            println!("📂 今年還沒有任何靈感記錄哦！");
        }
        return;
    }

    // 5. 破除 Bash 扁平化妖法：把所有參數用空格連起來，並保持換行格式
    // 跳過第一個參數（程序名自己），把剩下的多行文字原封不動打包
    let note_content = args[1..].join(" ");

    // 6. 物理內存與硬碟打通：開啟追加模式打開檔案
    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&file_path)
    {
        // 寫入靈感的同時，自動在末尾補上換行符，死死鎖定格式
        if writeln!(file, "{}", note_content).is_ok() {
            println!("✨ 靈感已安全鎖進 {} 廠房！", current_year);
        }
    }
}

