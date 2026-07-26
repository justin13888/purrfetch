{
  # DORMANT PROTOTYPE: Nix distribution is deferred pending demand and
  # end-to-end validation. This flake may be stale and is not release-tested.
  description = "purr — fast, neofetch-compatible system information tool";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs = { self, nixpkgs }:
    let
      # The binary is self-contained (links only libc), so Darwin builds use the
      # same prototype derivation with no extra inputs.
      systems = [ "x86_64-linux" "aarch64-linux" "x86_64-darwin" "aarch64-darwin" ];
      forAllSystems = nixpkgs.lib.genAttrs systems;
      cargoToml = builtins.fromTOML (builtins.readFile ./Cargo.toml);

      purrFor = system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
        in
        pkgs.rustPlatform.buildRustPackage {
          pname = "purr";
          version = cargoToml.package.version;
          src = self;
          cargoLock.lockFile = ./Cargo.lock;

          # build.rs reads .git for the commit hash, which is absent in the Nix
          # sandbox; it degrades to "unknown" (no failure) and takes the build
          # date from SOURCE_DATE_EPOCH, which nixpkgs sets for reproducibility.

          # Ship the man page and shell completions alongside the binary, using
          # the same standard paths as the deb/rpm/AUR/Alpine packages.
          postInstall = ''
            install -Dm644 man/purr.1 "$out/share/man/man1/purr.1"
            install -Dm644 completions/purr.bash "$out/share/bash-completion/completions/purr"
            install -Dm644 completions/_purr "$out/share/zsh/site-functions/_purr"
            install -Dm644 completions/purr.fish "$out/share/fish/vendor_completions.d/purr.fish"
          '';

          meta = {
            description = "Fast, neofetch-compatible system information tool";
            homepage = "https://github.com/justin13888/purrfetch";
            license = pkgs.lib.licenses.mit;
            mainProgram = "purr";
            platforms = pkgs.lib.platforms.unix;
          };
        };
    in
    {
      packages = forAllSystems (system: rec {
        purr = purrFor system;
        default = purr;
      });

      apps = forAllSystems (system: rec {
        purr = {
          type = "app";
          program = "${purrFor system}/bin/purr";
        };
        default = purr;
      });

      devShells = forAllSystems (system:
        let pkgs = nixpkgs.legacyPackages.${system}; in {
          default = pkgs.mkShell {
            inputsFrom = [ (purrFor system) ];
            packages = [ pkgs.cargo pkgs.rustc pkgs.clippy pkgs.rustfmt ];
          };
        });

      # `nix flake check` builds this on the current system.
      checks = forAllSystems (system: {
        purr = purrFor system;
      });

      overlays.default = final: prev: {
        purr = purrFor final.stdenv.hostPlatform.system;
      };

      formatter = forAllSystems (system: nixpkgs.legacyPackages.${system}.nixpkgs-fmt);
    };
}
