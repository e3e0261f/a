// src/color.rs
pub enum TerminalColor {
    Green,
    Cyan,
    Yellow,
    Magenta,
    Blue,
    BrightGreen,
    BrightCyan,
}

pub fn paint_line(line: &str, color: TerminalColor) {
    match color {
        TerminalColor::Green => println!("\x1b[1;32m{}\x1b[0m", line),
        TerminalColor::Cyan  => println!("\x1b[1;36m{}\x1b[0m", line),
        TerminalColor::Yellow => println!("\x1b[1;33m{}\x1b[0m", line),
        TerminalColor::Magenta => println!("\x1b[1;35m{}\x1b[0m", line),
        TerminalColor::Blue => println!("\x1b[1;34m{}\x1b[0m", line),
        TerminalColor::BrightGreen => println!("\x1b[1;92m{}\x1b[0m", line),
        TerminalColor::BrightCyan => println!("\x1b[1;96m{}\x1b[0m", line),
    }
}