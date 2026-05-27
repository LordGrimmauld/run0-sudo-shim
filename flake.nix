{
  description = "a shim imitating sudo, but using run0 in the background";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable-small";
    polkit-stdin-agent = {
      # url = "git+https://codeberg.org/r-vdp/polkit-stdin-agent"; # original
      url = "git+https://git.grimmauld.de/mirrors/polkit-stdin-agent"; # original is on codeberg, but gets rate-limited causing GHA to fail.
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
      nix-github-actions,
      treefmt-nix,
      polkit-stdin-agent,
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
          coreutils,
          lib,
          rustPlatform,
          polkit-stdin-agent,
          systemd,
        }:
        rustPlatform.buildRustPackage {
          inherit name;
          inherit (cargo-toml) version;
          src = lib.cleanSource ./.;
          cargoLock.lockFile = ./Cargo.lock;

          strictDeps = true;
          __structuredAttrs = true;

          env = {
            POLKIT_STDIN_AGENT = lib.getExe polkit-stdin-agent;
            RUN0 = lib.getExe' systemd "run0";
            TRUE = lib.getExe' coreutils "true";
          };

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
          # use polkit-stdin-agent from nixpkgs once available
          # https://github.com/NixOS/nixpkgs/pull/512018
          ${name} = pkgs.callPackage package {
            polkit-stdin-agent =
              pkgs.polkit-stdin-agent or polkit-stdin-agent.packages."${system}".polkit-stdin-agent;
          };
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
              polkit-stdin-agent.packages."${system}".polkit-stdin-agent
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

      overlays.default = final: prev: {
        ${name} = final.callPackage package {
          # use polkit-stdin-agent from nixpkgs once available
          # https://github.com/NixOS/nixpkgs/pull/512018
          polkit-stdin-agent =
            prev.polkit-stdin-agent
              or polkit-stdin-agent.packages."${prev.stdenv.hostPlatform.system}".polkit-stdin-agent;
        };
      };

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
                  if (action.id == "org.freedesktop.systemd1.manage-units" && subject.local && subject.active) {
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
      nixosConfigurations.sudo-test-vm = nixpkgs.lib.nixosSystem {
        system = "x86_64-linux";
        modules = [
          (
            { modulesPath, ... }:
            {
              imports = [
                # "${modulesPath}/profiles/minimal.nix"
                "${modulesPath}/virtualisation/qemu-vm.nix"
              ];
              system.stateVersion = "26.05";
              virtualisation.graphics = false;
              users.users = {
                admin = {
                  isNormalUser = true;
                  extraGroups = [ "wheel" ];
                  password = "1234";
                };
                noadmin = {
                  isNormalUser = true;
                  password = "4321";
                };
              };
            }
          )
        ];
      };
    };
}
