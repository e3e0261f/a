{ lib
, stdenv
, rustPlatform
, pkg-config
, openssl
, gnupg
, coreutils
, which
, xdg-utils
, nodejs
, makeWrapper
, darwin ? null
}:

let
  # 過濾不必要的建置暫存與巨大目錄，加速 Nix Store 拷貝
  cleanSrc = lib.cleanSourceWith {
    src = ./..;
    filter = path: type:
      let baseName = baseNameOf (toString path);
      in !(type == "directory" && (
            baseName == "target" ||
            baseName == "node_modules" ||
            baseName == ".git" ||
            baseName == "dist" ||
            baseName == ".cache"
          ))
         && !(type == "regular" && (
            lib.hasSuffix ".sock" baseName ||
            lib.hasSuffix ".pid" baseName ||
            lib.hasSuffix ".state" baseName
          ));
  };
in
rustPlatform.buildRustPackage {
  pname = "cyber-note";
  version = "0.0.4";
  src = cleanSrc;

  cargoLock = {
    lockFile = ../Cargo.lock;
  };

  nativeBuildInputs = [
    pkg-config
    makeWrapper
  ];

  buildInputs = [
    openssl
  ] ++ lib.optionals stdenv.isDarwin (
    if darwin != null && darwin ? apple_sdk then [
      darwin.apple_sdk.frameworks.Security
      darwin.apple_sdk.frameworks.SystemConfiguration
    ] else [ ]
  );

  # 執行建置後的 NixOS 100% 適配與環境打包
  postInstall = ''
    # 確保主二進位檔存在
    if [ ! -f "$out/bin/a" ]; then
      echo "❌ 找不到編譯產出之 a 二進位檔"
      exit 1
    fi

    # 🛡️ 核心：在 NixOS 上自動包裹運行時必要工具鏈
    # 杜絕 NixOS 因純淨環境缺少 gpg, kill, which, xdg-utils, nodejs 導致命令失敗
    wrapProgram "$out/bin/a" \
      --prefix PATH : ${lib.makeBinPath [
        gnupg
        coreutils
        which
        xdg-utils
        nodejs
      ]}

    # 建立 cyber-note 友善別名軟連結
    ln -sf "$out/bin/a" "$out/bin/cyber-note"
  '';

  meta = with lib; {
    description = "Cyber-NOte: 零成本抽象、鎖定 GPG 安全審計的終端與網頁雙引擎筆記系統";
    longDescription = ''
      專為資安攻防與密碼學保密設計的筆記工具。
      具備 GPG 鎖定審計、S2K 65,011,712 輪防量子窮舉對稱加密、
      GitHub Gist 雲端在位原子套殼（In-Place Encapsulate）與多層防禦能力。
    '';
    homepage = "https://github.com/e3e0261f/a";
    license = licenses.mit;
    mainProgram = "a";
    platforms = platforms.unix;
  };
}
