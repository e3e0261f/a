🛡️ Project a (Cyber-Forge 靈感法典管家)
極簡、去中心化、端到端 GPG 公鑰加密的本地日誌與雲端同步終端引擎。
專為終端極客設計的日誌與靈感管理工具。本地落盤即密文，公鑰加密寫入零阻礙，私鑰受控解密閱覽；雲端無縫對接 GitHub Gist，實現多檔案隔離與版本存儲。
✨ 核心特性
端到端加密（Zero-Knowledge at Rest）：本地不留明文。寫入時直接透過 GPG 公鑰封裝為單一 ASCII Armor 密文，防範實體偷窺。
平滑寫入體驗：常規紀錄無需輸入私鑰密碼（公鑰單向加密），極速寫入，流暢無阻。
Gist 雲端安全引渡：透過 GitHub REST API 進行局部 PATCH 更新，不同年份與外部檔案獨立並存，絕不互相覆蓋。
金鑰憑證防護：Token 自身經 GPG 加密為 token.gpg 儲存，徹底杜絕 Shell 變數與設定檔明文落盤。
高對比終端著色：自適應奇偶行雙色渲染（Green / Cyan），提升長文閱讀體驗。
🛠️ 指令合約 (Command Contract)
指令	說明
a [靈感內容...]	寫入筆記：自動解密歷史包裹、拼接新筆記，並以 GPG 公鑰整檔加密封存。
a -a / a --all	查看今年：解密並以雙色交替列印今年度（如 2026.note.gpg）全部內容。
a -a [年份/檔名]	查看指定檔案：自動解密並列印指定年份或倉庫內的任意密文/明文檔案。
a -s / a --sync	主動同步：將當前年份的密文包裹推送至 GitHub Gist 雲端。
a -s [檔案路徑]	加密外傳：以 GPG 公鑰加密指定外部檔案，並以 .gpg 後綴推送至雲端。
a -s [路徑] --raw	明文外傳：不進行 GPG 加密，以原始格式直傳外部檔案（支援 -u 參數）。
a -l / a --list	雲端雷達：掃描並列出 GitHub Gist 倉庫中存有的所有檔案清單。
a -d [年份/檔名]	雲端下載：精準下載指定年份（自動補全 .note.gpg）或具體檔名至本地倉庫。
a -r [關鍵字]	行級剔除：解密筆記、過濾並刪除所有包含指定關鍵字的行，重新加密存盤。
🏗️ 目錄與架構
code
Text
src/
├── main.rs      # 命令解析、工作流調度與業務狀態機
├── lib.rs       # 全局常數宣告 (GPG 指紋、Gist 端點、本地倉庫路徑)
├── gist.rs      # GitHub REST API 傳輸引擎 (GET, PATCH, List)
├── encrypt.rs   # GPG 系統子行程調度管道 (Encrypt, Decrypt)
├── storage.rs   # 本地密文檔案 IO 讀寫介面
└── color.rs     # ANSI 終端色彩渲染模組
⚙️ 快速上手
1. 前置依賴
Rust Toolchain (cargo, rustc)
GnuPG (gpg, gpg-agent)
2. 環境配置 (src/lib.rs)
修改 src/lib.rs 中的設定常數：
code
Rust
pub const NOTE_DIR: &'static str = "/home/your_user/BOok/NOte";
pub const GPG_USER_ID: &'static str = "你的_GPG_金鑰指紋";
pub const GIST_URL: &'static str = "https://api.github.com/gists/你的_GIST_ID";
3. 憑證初始化 (token.gpg)
將你的 GitHub Personal Access Token (需具備 gist 權限) 加密存放至筆記倉庫：
code
Fish
echo "ghp_your_personal_access_token" > token.txt
gpg --encrypt --recipient "你的_GPG_指紋" --armor --output /home/your_user/BOok/NOte/token.gpg token.txt
rm token.txt
4. 編譯與安裝
code
Bash
./install.sh
🔒 安全模型與邊界
公私鑰分離：日常寫入與外部檔案加密僅調用公鑰（零私鑰依賴）；閱覽與行刪除需透過私鑰授權。
記憶體快取建議：建議於 ~/.gnupg/gpg-agent.conf 設定合適的 default-cache-ttl，兼顧頻繁調用之流暢度與密鑰離線安全。


# a


环境需要 调用用户gpg密钥完成加密动作。 
sudo apt install libsecret-tools
