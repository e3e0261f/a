// src/color.rs
// 🎨 這裡只負責管顏色！與檔案讀寫完全解耦

pub enum TerminalColor {
    Green,
    Cyan,
}

// 函數體：給文字穿外衣的專用公式
pub fn paint_line(line: &str, color: TerminalColor) {
    match color {
        TerminalColor::Green => println!("\x1b[1;32m{}\x1b[0m", line),
        TerminalColor::Cyan  => println!("\x1b[1;36m{}\x1b[0m", line),
    }
}

