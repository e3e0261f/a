#!/bin/bash
echo "🚀 正在启动 Rust 零成本抽象极致编译..."
cargo build --release

# 🛑 核心防護：檢查上一條命令是否成功 (Exit Code != 0)
if [ $? -ne 0 ]; then
    echo "❌ [警報] 编译失败！二进制作战战舰原地待命，拒绝空降。"
    exit 1
fi

echo "📦 正在将二进制作战战舰自动空降到 /usr/bin/a..."
sudo cp target/release/a /usr/bin/a

if [ $? -ne 0 ]; then
    echo "❌ [警報] 权限不足或复制失败！"
    exit 1
fi

echo "✨ 自动化安装全线功德圆满！"