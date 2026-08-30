function fish_command_not_found
    # 当命令找不到时，自动尝试用 apt 帮你安装它
    echo "正在尝试自动安装 $argv..."
    sudo apt install $argv
end
