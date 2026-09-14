#!/usr/bin/env bash

echo "🚀 正在启动 Rust 零成本抽象极致编译..."
cargo build --release

# 🛑 核心防護：檢查上一條命令是否成功 (Exit Code != 0)
if [ $? -ne 0 ]; then
    echo "❌ [警報] 编译失败！二进制作战战舰原地待命，拒绝空降。"
    exit 1
fi

# 🎯 创建用户本地 bin 目录（如果不存在的话）
INSTALL_DIR="$HOME/.local/bin"
mkdir -p "$INSTALL_DIR"

echo "📦 正在将二进制作战战舰自动空降到 $INSTALL_DIR/a..."
cp target/release/a "$INSTALL_DIR/a"

if [ $? -ne 0 ]; then
    echo "❌ [警報] 复制失败！请检查目标路径权限。"
    exit 1
fi

echo "✨ 自动化安装全线功德圆满！"
