// src/color.rs
#[derive(Clone, Copy)]
pub enum TerminalColor {
    Normal,
    Gray,
    Dim,
    LightRedBg,
    LightBlueBg,
}

pub fn paint_line(line: &str, color: TerminalColor) {
    match color {
        TerminalColor::Normal => println!("\x1b[1;37m{}\x1b[0m", line), // White
        TerminalColor::Gray => println!("\x1b[0;37m{}\x1b[0m", line),   // Gray
        TerminalColor::Dim => println!("\x1b[2m{}\x1b[0m", line),      // Dim / Dark gray
        // 🌸 淺紅背景交替列 (Light Red Background, White Text)
        TerminalColor::LightRedBg => println!("\x1b[48;2;90;28;38;97m {}\x1b[0m", line),
        // 🌊 淺藍背景交替列 (Light Blue Background, White Text)
        TerminalColor::LightBlueBg => println!("\x1b[48;2;22;52;92;97m {}\x1b[0m", line),
    }
}

