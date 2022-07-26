{
  inputs = {
    flake-utils.url = "github:numtide/flake-utils";
    naersk.url = "github:nix-community/naersk";
  };

  outputs = {
    self,
    nixpkgs,
    flake-utils,
    naersk,
  }:
    flake-utils.lib.eachDefaultSystem (
      system: let
        pkgs = nixpkgs.legacyPackages."${system}";
        naersk-lib = naersk.lib."${system}";
      in rec {
        # `nix build`
        packages.prometheus-edge-trigger = naersk-lib.buildPackage {
          pname = "prometheus-edge-trigger";
          root = ./.;

          buildInputs = with pkgs; [gcc];
        };
        defaultPackage = packages.prometheus-edge-trigger;
        defaultApp = packages.prometheus-edge-trigger;

        # `nix develop`
        devShell = pkgs.mkShell {
          nativeBuildInputs = with pkgs; [rustc cargo bacon cargo-edit cargo-outdated gcc];
        };
      }
    )
    // {
      nixosModule = {
        config,
        lib,
        pkgs,
        ...
      }:
        with lib; let
          cfg = config.services.prometheus-edge-trigger;
          format = pkgs.formats.toml {};
          configFile = format.generate "config.toml" {
            prometheus.url = cfg.prometheusAddress;
            mqtt = {
              inherit (cfg.mqtt) host username;
              password_file = cfg.mqtt.passwordFile;
            };
            trigger = map (trigger: {
              inherit (trigger) name delay condition;
              action = if (trigger.action.method == "MQTT") then {
                inherit (trigger.action) method topic payload params;
              } else {
                inherit (trigger.action) method params url;
              };
            }) cfg.triggers;
          };
        in {
          options.services.prometheus-edge-trigger = {
            enable = mkEnableOption "WiFi prometheus exporter";

            prometheusAddress = mkOption {
              type = types.str;
              description = "address of the prometheus server";
            };

            logLevel = mkOption {
              type = types.str;
              default = "INFO";
              description = "log level";
            };

            mqtt = mkOption {
              type = types.submodule {
                options = {
                  host = mkOption {
                    type = types.str;
                    description = "mqtt server hostname";
                  };
                  username = mkOption {
                    type = types.str;
                    description = "mqtt username";
                  };
                  passwordFile = mkOption {
                    type = types.str;
                    description = "path containing the mqtt password";
                  };
                };
              };
            };

            triggers = mkOption {
              description = "configured triggers";
              type = types.listOf (types.submodule {
                options = {
                  name = mkOption {
                    type = types.str;
                    description = "name of the trigger";
                  };
                  delay = mkOption {
                    type = types.int;
                    description = "delay in secconds";
                  };

                  condition = mkOption {
                    type = types.submodule {
                      options = {
                        query = mkOption {
                          type = types.str;
                          description = "prometheus query to trigger on";
                        };
                        from = mkOption {
                          type = types.int;
                          description = "start of the edge";
                        };
                        to = mkOption {
                          type = types.int;
                          description = "end of the edge";
                        };
                        params = mkOption {
                          type = types.attrs;
                          default = {};
                          description = "query substitution parameters";
                        };
                      };
                    };
                  };

                  action = mkOption {
                    type = types.submodule {
                      options = {
                        method = mkOption {
                          type = types.str;
                          description = "http method or 'MQTT'";
                        };
                        topic = mkOption {
                          type = types.null or types.str;
                          default = null;
                          description = "mqtt topic";
                        };
                        payload = mkOption {
                          type = types.null or types.str;
                          default = null;
                          description = "mqtt payload";
                        };
                        url = mkOption {
                          type = types.null or types.str;
                          default = null;
                          description = "mqtt url";
                        };
                        params = mkOption {
                          type = types.attrs;
                          default = {};
                          description = "http url substitution parameters";
                        };
                      };
                    };
                  };
                };
              });
            };
          };

          config = mkIf cfg.enable {
            systemd.services."prometheus-edge-trigger" = let
              pkg = self.defaultPackage.${pkgs.system};
            in {
              wantedBy = ["multi-user.target"];
              script = "${pkg}/bin/prometheus-edge-trigger ${configFile}";
              environment = {
                RUST_LOG = cfg.logLevel;
              };

              serviceConfig = {
                Restart = "on-failure";
                DynamicUser = true;
                PrivateTmp = true;
                ProtectSystem = "strict";
                ProtectHome = true;
                NoNewPrivileges = true;
                PrivateDevices = true;
                ProtectClock = true;
                CapabilityBoundingSet = true;
                ProtectKernelLogs = true;
                ProtectControlGroups = true;
                SystemCallArchitectures = "native";
                ProtectKernelModules = true;
                RestrictNamespaces = true;
                MemoryDenyWriteExecute = true;
                ProtectHostname = true;
                LockPersonality = true;
                ProtectKernelTunables = true;
                RestrictAddressFamilies = "AF_INET AF_INET6";
                RestrictRealtime = true;
                ProtectProc = "noaccess";
                SystemCallFilter = ["@system-service" "~@resources" "~@privileged"];
                IPAddressDeny = "any";
                IPAddressAllow = ["192.168.0.0/16" "localhost" "172.0.0.0/8"];
                PrivateUsers = true;
                ProcSubset = "pid";
              };
            };
          };
        };
    };
}
