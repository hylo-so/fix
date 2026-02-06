{
  description = "fix Rust devShell";

  inputs = {
    nixpkgs.url      = "github:NixOS/nixpkgs/25.11";
    rust-overlay.url = "github:oxalica/rust-overlay/2859683cd9ef7858d324c5399b0d8d6652bf4044";
    flake-parts.url  = "github:hercules-ci/flake-parts";
  };

  outputs = inputs@{ nixpkgs, rust-overlay, flake-parts, ... }:
    flake-parts.lib.mkFlake { inherit inputs; } {
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "aarch64-darwin"
      ];
      perSystem = { system, pkgs, ... }:  {
        _module.args.pkgs = import inputs.nixpkgs {
          inherit system;
          overlays = [
            inputs.rust-overlay.overlays.default
          ];
        };
        devShells.default = with pkgs; mkShell {
          buildInputs = [
            openssl
            pkg-config
            (rust-bin.stable."1.88.0".default.override {
              extensions = [ "rust-analyzer" "rust-src" ];
            })
          ];
        };
      };
    };
}
