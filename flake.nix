{
  description = "fix Rust devShell";

  inputs = {
    nixpkgs.url      = "github:NixOS/nixpkgs/23.11";
    rust-overlay.url = "github:oxalica/rust-overlay/9f3d63d569536cd661a4adcf697e32eb08d61e31";
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
            rust-bin.stable."1.81.0".default  
            rust-analyzer
          ];
        };
      };
    };
}
