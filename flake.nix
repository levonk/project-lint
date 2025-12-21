{
  description = "project-lint dev environment";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };
      in
      {
        devShells.default = pkgs.mkShell {
          packages = [
            pkgs.cargo
            pkgs.rustc
            pkgs.rust-analyzer
            pkgs.clippy
            pkgs.rustfmt

            pkgs.universal-ctags
            pkgs.git

            pkgs.pkg-config
            pkgs.openssl
            pkgs.libgit2
            pkgs.zlib

            pkgs.clang
          ];

          shellHook = "";
        };
      }
    );
}
