// src/color.rs
pub enum TerminalColor {
    Green,
    Cyan,
}

pub fn paint_line(line: &str, color: TerminalColor) {
    match color {
        TerminalColor::Green => println!("\x1b[1;32m{}\x1b[0m", line),
        TerminalColor::Cyan  => println!("\x1b[1;36m{}\x1b[0m", line),
    }
}