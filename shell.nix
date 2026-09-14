{ pkgs ? import <nixpkgs> { } }:

pkgs.mkShell {
  name = "cyber-note-shell";

  inputsFrom = [ (pkgs.callPackage ./nix/package.nix { }) ];

  nativeBuildInputs = [
    pkgs.cargo
    pkgs.rustc
    pkgs.rustfmt
    pkgs.clippy
    pkgs.pkg-config
  ];

  buildInputs = [
    pkgs.openssl
    pkgs.gnupg
    pkgs.pinentry-curses
    pkgs.git
    pkgs.nodejs_22
    pkgs.nodePackages.npm
    pkgs.xdg-utils
  ];

  shellHook = ''
    echo "🛡️ Cyber-NOte Nix 開發環境已就緒 (cargo / rustc / gpg / node / openssl)"
  '';
}
