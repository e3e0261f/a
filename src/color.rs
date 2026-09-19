// src/color.rs
#[derive(Clone, Copy)]
pub enum TerminalColor {
    Normal,
    Gray,
    Dim,
}

pub fn paint_line(line: &str, color: TerminalColor) {
    match color {
        TerminalColor::Normal => println!("\x1b[1;37m{}\x1b[0m", line), // White
        TerminalColor::Gray   => println!("\x1b[0;37m{}\x1b[0m", line), // Gray
        TerminalColor::Dim    => println!("\x1b[2m{}\x1b[0m", line),    // Dim / Dark gray
    }
}
