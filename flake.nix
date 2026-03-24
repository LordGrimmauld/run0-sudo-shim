{
  description = "a shim imitating sudo, but using run0 in the background";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable-small";
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
      nix-github-actions,
      treefmt-nix,
      ...
    }:
    let
      inherit (nixpkgs) lib;
      cargo-toml = (lib.importTOML ./Cargo.toml).package;
      inherit (cargo-toml) name;
      forEachSystem =
        f:
        builtins.listToAttrs (
          map
            (system: {
              name = system;
              value = f {
                inherit system;
                pkgs = nixpkgs.legacyPackages.${system};
              };
            })
            [
              "x86_64-linux"
              "x86_64-darwin"
              "aarch64-linux"
              "aarch64-darwin"
            ]
        );

      package =
        {
          lib,
          rustPlatform,
          ...
        }:
        rustPlatform.buildRustPackage {
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

      treefmtEval = (lib.flip treefmt-nix.lib.evalModule) ./treefmt.nix;
    in
    {
      packages = forEachSystem (
        { pkgs, system }:
        {
          ${name} = pkgs.callPackage package { };
          default = self.packages.${system}.${name};
        }
      );

      devShells = forEachSystem (
        { pkgs, system }:
        {
          default = pkgs.mkShell {
            inputsFrom = [ self.packages.${system}.default ];
            packages = [
              pkgs.clippy
              pkgs.rust-analyzer
              pkgs.rustfmt
            ];
          };
        }
      );

      formatter = forEachSystem ({ pkgs, ... }: (treefmtEval pkgs).config.build.wrapper);

      checks = forEachSystem (
        { pkgs, system }:
        {
          formatting = (treefmtEval pkgs).config.build.check self;
          vm = pkgs.testers.runNixOSTest {
            name = "run0-sudo-shim-vm-test";
            nodes.machine = {
              imports = [ self.nixosModules.default ];
              services.dbus.implementation = "broker";
              security = {
                polkit.persistentAuthentication = true;
                run0-sudo-shim.enable = true;
              };

              users.users = {
                admin = {
                  isNormalUser = true;
                  extraGroups = [ "wheel" ];
                };
                noadmin = {
                  isNormalUser = true;
                };
              };
            };
            testScript = ''
              # machine.succeed('su - admin -c "sudo -v"') # can't yet give password, needs hacks to never ask for password in the test or enter the password
              machine.fail('su - noadmin -c "sudo -v"')
            '';
          };
        }
        // self.packages.${system}
      );
      githubActions = nix-github-actions.lib.mkGithubMatrix {
        checks = { inherit (self.checks) x86_64-linux; };
      };

      overlays.default = final: _: { ${name} = final.callPackage package { }; };

      nixosModules.default =
        {
          pkgs,
          lib,
          config,
          ...
        }:
        let
          cfg = config.security.run0-sudo-shim;
        in
        {
          options.security = {
            polkit.persistentAuthentication = lib.mkEnableOption "patching polkit to allow persistent authentication and adding rules";
            run0-sudo-shim = {
              enable = lib.mkEnableOption "run0-sudo-shim instead of sudo";
              package = lib.mkPackageOption pkgs "run0-sudo-shim" { } // {
                # should be removed when upstreaming to nixpkgs
                default = pkgs.run0-sudo-shim or self.packages.${pkgs.stdenv.system}.default;
              };
            };
          };

          config = lib.mkMerge [
            (lib.mkIf cfg.enable {
              environment.systemPackages = [ cfg.package ];
              security.sudo.enable = false;
              security.polkit.enable = true;
            })
            (lib.mkIf config.security.polkit.persistentAuthentication {
              assertions =
                let
                  mkMessage = package: minVer: ''
                    To provide persistent authentication, Polkit requires `pidfd` support when fetching process details from D-Bus, which is only available in `${package}` version ${minVer} or later.

                    Please update the package or switch `services.dbus.implementation` in the configuration.
                  '';
                in
                [
                  (lib.mkIf (config.services.dbus.implementation == "dbus") {
                    assertion = lib.versionAtLeast config.services.dbus.dbusPackage.version "1.15.7";
                    message = mkMessage "dbus" "1.15.7";
                  })
                  (lib.mkIf (config.services.dbus.implementation == "broker") {
                    assertion = lib.versionAtLeast config.services.dbus.brokerPackage.version "34";
                    message = mkMessage "dbus-broker" "34";
                  })
                ];

              security.polkit.extraConfig = ''
                polkit.addRule(function(action, subject) {
                  if (action.id == "org.freedesktop.policykit.exec" ||
                      action.id.indexOf("org.freedesktop.systemd1.") {
                      return polkit.Result.AUTH_ADMIN_KEEP;
                  }
                });
              '';

              # don't apply patch starting version 127, where persistent auth is supported upstream
              security.polkit.package = lib.mkIf (lib.versionOlder pkgs.polkit.version "127") (
                pkgs.polkit.overrideAttrs (old: {
                  patches = old.patches or [ ] ++ [
                    (pkgs.fetchpatch {
                      url = "https://github.com/polkit-org/polkit/pull/533.patch?full_index=1";
                      hash = "sha256-i8RkHDGdSwO6/kueVhMVefqUqC38lQmEBSKtminDlN8=";
                    })
                  ];
                })
              );
            })
          ];
        };
    };
}
