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

      nixosModules.default =
        {
          pkgs,
          lib,
          config,
          ...
        }:
        {
          options.security = {
            polkit.persistentAuthentication = lib.mkEnableOption "patch polkit to allow persistent authentication and add rules";
            run0-sudo-shim = lib.mkEnableOption "enable run0-sudo-shim instead of sudo";
          };

          config = lib.mkMerge [
            {
              nixpkgs.overlays = [ self.overlays.default ];
            }
            (lib.mkIf config.security.run0-sudo-shim {
              environment.systemPackages = [ pkgs.run0-sudo-shim ];
              security.sudo.enable = false;
            })
            (lib.mkIf config.security.polkit.persistentAuthentication {
              security.polkit.extraConfig = ''
                polkit.addRule(function(action, subject) {
                  if (action.id == "org.freedesktop.policykit.exec") {
                    return polkit.Result.AUTH_ADMIN_KEEP;
                  }
                });

                polkit.addRule(function(action, subject) {
                  if (action.id.indexOf("org.freedesktop.systemd1.") == 0) {
                    return polkit.Result.AUTH_ADMIN_KEEP;
                  }
                });
              '';

              # replaceDependencies to avoid mass rebuild
              system.replaceDependencies.replacements =
                let
                  polkit' = pkgs.polkit.overrideAttrs (old: {
                    patches = old.patches or [ ] ++ [
                      (pkgs.fetchpatch {
                        url = "https://github.com/polkit-org/polkit/pull/533.patch?full_index=1";
                        hash = "sha256-i8RkHDGdSwO6/kueVhMVefqUqC38lQmEBSKtminDlN8=";
                      })
                    ];
                  });
                in
                builtins.concatMap
                  (
                    { oldDependency, newDependency }:
                    assert oldDependency.outputs == newDependency.outputs;
                    builtins.map (out: {
                      oldDependency = oldDependency.${out};
                      newDependency = newDependency.${out};
                    }) oldDependency.outputs
                  )
                  (
                    lib.singleton {
                      oldDependency = pkgs.polkit;
                      newDependency = polkit';
                    }
                  );
            })
          ];
        };
    };
}
