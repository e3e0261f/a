// src/lib.rs
// 🏭 中央總廠房：寫入 Gist 物理領地常數

pub mod color;
pub mod storage;
pub mod gist;
pub mod encrypt;

pub struct GameConfig;

impl GameConfig {
    // 📂 法律一：本地筆記本存放的絕對總目錄坑位
    pub const NOTE_DIR: &'static str = "/home/lee/BOok/NOte";
    
    // 🔑 法律二：死死鎖定你長期使用的、絕對唯一的 GPG 密鑰鋼鐵指紋！
    pub const GPG_USER_ID: &'static str = "31C81A9DE1AB870A8EDC3486D7C2DF9FA0283056";

    // 🌐 法律三：修正為標準 GitHub REST API 終端位址
    pub const GIST_URL: &'static str = "https://api.github.com/gists/af785d31440231cb78787d9cdf1bbba5";
}