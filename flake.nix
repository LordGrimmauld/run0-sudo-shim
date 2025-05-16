{
  description = "a shim imitating sudo, but using run0 in the background";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable-small";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    nix-github-actions = {
      url = "github:nix-community/nix-github-actions";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    treefmt-nix = {
      url = "github:numtide/treefmt-nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      flake-utils,
      rust-overlay,
      nix-github-actions,
      treefmt-nix,
      ...
    }:
    let
      cargo-toml = (builtins.fromTOML (builtins.readFile ./Cargo.toml)).package;
      inherit (cargo-toml) name;

      build-pkg =
        pkgs:
        let
          inherit (pkgs) lib;
        in
        pkgs.rustPlatform.buildRustPackage {
          inherit name;
          inherit (cargo-toml) version;
          src = lib.cleanSource ./.;
          cargoLock.lockFile = ./Cargo.lock;

          postInstall = ''
            ln -s $out/bin/${name} $out/bin/sudo
          '';

          meta = {
            inherit (cargo-toml) description;
            mainProgram = name;
            license = lib.getLicenseFromSpdxId cargo-toml.license;
            maintainers = with lib.maintainers; [ grimmauld ];
          };
        };

      outputs = flake-utils.lib.eachDefaultSystem (
        system:
        let
          overlays = [ (import rust-overlay) ];
          pkgs = import nixpkgs {
            inherit system overlays;
          };
          rustToolchain = pkgs.pkgsBuildHost.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
          treefmtEval = treefmt-nix.lib.evalModule pkgs ./treefmt.nix;
        in
        {
          packages.${name} = build-pkg pkgs;
          packages.default = self.packages.${system}.${name};

          devShells.default = pkgs.mkShell {
            buildInputs = [
              rustToolchain
              pkgs.rust-analyzer
            ];
          };

          formatter = treefmtEval.config.build.wrapper;

          checks = {
            formatting = treefmtEval.config.build.check self;
          } // self.packages.${system};
        }
      );
    in
    outputs
    // {

      githubActions = nix-github-actions.lib.mkGithubMatrix {
        checks = nixpkgs.lib.getAttrs [ "x86_64-linux" ] outputs.checks;
      };

      overlays.default = final: prev: { ${name} = build-pkg prev; };

      nixosModules.default = {
        nixpkgs.overlays = [ self.overlays.default ];
      };
    };
}
