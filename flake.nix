{
  inputs = {
    nixpkgs.url = "nixpkgs/nixos-23.11";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = {
    self,
    nixpkgs,
    flake-utils,
  }:
    flake-utils.lib.eachDefaultSystem (
      system: let
        overlays = [
          (import ./overlay.nix)
        ];
        pkgs = (import nixpkgs) {
          inherit system overlays;
        };
      in rec {
        packages = rec {
          prometheus-edge-trigger = pkgs.prometheus-edge-trigger;
          default = prometheus-edge-trigger;
        };

        devShell = pkgs.mkShell {
          nativeBuildInputs = with pkgs; [rustc cargo bacon cargo-edit cargo-outdated clippy cargo-audit];
        };
      }
    )
    // {
      overlays.default = import ./overlay.nix;
      nixosModules.default = {
        pkgs,
        config,
        lib,
        ...
      }: {
        imports = [./module.nix];
        config = lib.mkIf config.services.prometheus-edge-trigger.enable {
          nixpkgs.overlays = [self.overlays.default];
          services.prometheus-edge-trigger.package = lib.mkDefault pkgs.prometheus-edge-trigger;
        };
      };
    };
}
