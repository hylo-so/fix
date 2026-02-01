{
  description = "fix Rust devShell";

  inputs = {
    nixpkgs.url      = "github:NixOS/nixpkgs/25.11";
    rust-overlay.url = "github:oxalica/rust-overlay/bc00300f010275e46feb3c3974df6587ff7b7808";
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
            (rust-bin.stable."1.82.0".default.override {
              extensions = [ "rust-analyzer" "rust-src" ];
            })
          ];
        };
      };
    };
}
