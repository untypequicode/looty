# Module NixOS de Looty : services.looty. Voir docs/adr/0005 et le guide « NixOS » du site.
# Les trois services (serveur, interface, collaboration) sont les images publiées de la release, lancées par
# Podman (virtualisation.oci-containers) : mêmes binaires que la pile Docker Compose, mêmes variables.
# Les secrets ne sont jamais dans le dépôt Nix (donc pas dans /nix/store) : ils sont lus dans environmentFile.
{
  config,
  lib,
  pkgs,
  ...
}:

let
  inherit (lib)
    mkEnableOption
    mkIf
    mkMerge
    mkOption
    optionalAttrs
    types
    ;
  cfg = config.services.looty;
  bundled = cfg.auth == "bundled-oidc";
  authDomain = "auth.${cfg.domain}";
  baseUrl = "https://${cfg.domain}";
  runDir = "/run/looty";
  stateDir = "/var/lib/looty";
  dbUrl = "postgres://${cfg.database.user}:$DB_PASSWORD@127.0.0.1:${toString config.services.postgresql.settings.port}/${cfg.database.name}";

  # pocket-id-init.sh est le modèle de deploy/templates, sans ses lignes de garde (#@if / #@end).
  pocketIdInit = pkgs.writeShellScript "looty-pocket-id-init" (
    lib.concatStringsSep "\n" (
      lib.filter (l: builtins.match "[ \t]*#@.*" l == null) (
        lib.splitString "\n" (builtins.readFile ../templates/pocket-id-init.sh)
      )
    )
  );

  image = name: "ghcr.io/untypequicode/looty-${name}:${cfg.version}";
  common = {
    # Même réseau que l'hôte : PostgreSQL et le proxy sont sur 127.0.0.1.
    extraOptions = [
      "--network=host"
      "--cap-drop=ALL"
      "--security-opt=no-new-privileges"
    ];
  };
  envFiles = [ cfg.environmentFile ] ++ lib.optional cfg.database.createLocally "${runDir}/database.env";
in
{
  options.services.looty = {
    enable = mkEnableOption "Looty (tasks and collaborative wiki)";

    domain = mkOption {
      type = types.str;
      example = "looty.example.org";
      description = "Public domain of the instance (HTTPS). With auth = \"bundled-oidc\", Pocket ID is served on auth.<domain>.";
    };

    instanceName = mkOption {
      type = types.str;
      default = "Looty";
      example = "My association";
      description = "Name of the instance, used for the e-mail sender and the proxy.";
    };

    version = mkOption {
      type = types.str;
      default = "latest";
      example = "0.44.1";
      description = "Version of the release images (ghcr.io/untypequicode/looty-*). Pin it for reproducible deployments.";
    };

    auth = mkOption {
      type = types.enum [
        "oidc"
        "bundled-oidc"
        "local"
      ];
      default = "oidc";
      description = ''
        How people sign in. `oidc`: your own OpenID Connect provider (OIDC_ISSUER, OIDC_CLIENT_ID and
        OIDC_CLIENT_SECRET in the environment file). `bundled-oidc`: Pocket ID from nixpkgs, on auth.<domain>,
        with the Looty client created automatically. `local`: accounts stored by Looty (not available yet).
      '';
    };

    environmentFile = mkOption {
      type = types.path;
      example = "/run/secrets/looty.env";
      description = ''
        File of `KEY=value` lines read by systemd (agenix, sops-nix or a file you write; never in the Nix store).
        Required: `DB_PASSWORD` (letters and digits, with the local database), `LOOTY_SECRET_KEY`
        (`openssl rand -base64 32`). With `oidc`: `OIDC_ISSUER`, `OIDC_CLIENT_ID`, `OIDC_CLIENT_SECRET`
        (and optionally `OIDC_PROVIDER_ID`). With `bundled-oidc`: `OIDC_CLIENT_SECRET` (`openssl rand -hex 32`),
        `OIDC_PROVIDER_ID` (a UUID), `POCKET_ID_ENCRYPTION_KEY` and `POCKET_ID_STATIC_API_KEY` (`openssl rand -hex 32`).
        Any other Looty variable (SMTP_*, CALDAV_ALLOW_PRIVATE…) can be added here.
        With `database.createLocally = false`, add `DATABASE_URL` instead of `DB_PASSWORD`.
      '';
    };

    database = {
      createLocally = mkOption {
        type = types.bool;
        default = true;
        description = "Run PostgreSQL on this machine (services.postgresql) and create the Looty database. Otherwise, give DATABASE_URL in the environment file.";
      };
      name = mkOption {
        type = types.str;
        default = "looty_db";
        description = "Name of the local database.";
      };
      user = mkOption {
        type = types.str;
        default = "looty";
        description = "Name of the local database role.";
      };
    };

    proxy = mkOption {
      type = types.enum [
        "caddy"
        "nginx"
        "none"
      ];
      default = "caddy";
      description = "Reverse proxy configured through the NixOS options (HTTPS certificates included). `none`: you route the ports below yourself.";
    };

    openFirewall = mkOption {
      type = types.bool;
      default = true;
      description = "Open ports 80 and 443 when a proxy is configured.";
    };

    ports = {
      backend = mkOption {
        type = types.port;
        default = 8080;
        description = "Local port of the server (API, uploads, CalDAV).";
      };
      frontend = mkOption {
        type = types.port;
        default = 3000;
        description = "Local port of the web interface.";
      };
      collaboration = mkOption {
        type = types.port;
        default = 8082;
        description = "Local port of the live editing server.";
      };
    };

    backup = {
      enable = mkOption {
        type = types.bool;
        default = true;
        description = "Daily dump of the local database (services.postgresqlBackup, in /var/backup/postgresql). Uploaded files are in /var/lib/looty/uploads: include them in your own backups.";
      };
    };

    imageFiles = mkOption {
      type = types.attrsOf types.path;
      default = { };
      example = lib.literalExpression "{ backend = ./looty-backend.tar; }";
      description = "Optional image archives (keys: backend, frontend, collaboration) loaded instead of pulling. Mostly for tests and offline hosts.";
    };
  };

  config = mkIf cfg.enable (mkMerge [
    {
      assertions = [
        {
          assertion = cfg.auth != "local";
          message = "services.looty.auth = \"local\" is not available yet (local accounts are not part of Looty); use \"oidc\" or \"bundled-oidc\".";
        }
      ];

      virtualisation.oci-containers.backend = lib.mkDefault "podman";
      virtualisation.oci-containers.containers = {
        looty-backend = common // {
          image = image "backend";
          imageFile = cfg.imageFiles.backend or null;
          environment = {
            PORT = toString cfg.ports.backend;
            BASE_URL = baseUrl;
            SMTP_FROM_NAME = cfg.instanceName;
            BACKUP_DIR = "/app/backups";
          }
          // optionalAttrs bundled {
            OIDC_ISSUER = "https://${authDomain}";
            OIDC_CLIENT_ID = "looty";
          };
          environmentFiles = envFiles;
          volumes = [
            "${stateDir}/uploads:/app/uploads"
            "${stateDir}/backups:/app/backups"
          ];
          extraOptions = common.extraOptions ++ [ "--memory=1024m" ];
        };
        looty-collaboration = common // {
          image = image "collaboration";
          imageFile = cfg.imageFiles.collaboration or null;
          environment = {
            PORT = toString cfg.ports.collaboration;
            BACKEND_URL = "http://127.0.0.1:${toString cfg.ports.backend}";
          };
          environmentFiles = envFiles;
          extraOptions = common.extraOptions ++ [ "--memory=512m" ];
        };
        looty-frontend = common // {
          image = image "frontend";
          imageFile = cfg.imageFiles.frontend or null;
          environment = {
            PORT = toString cfg.ports.frontend;
            NITRO_PORT = toString cfg.ports.frontend;
            NITRO_HOST = "127.0.0.1";
          };
          extraOptions = common.extraOptions ++ [ "--memory=384m" ];
        };
      };

      systemd.tmpfiles.rules = [
        "d ${stateDir} 0750 root root -"
        "d ${stateDir}/uploads 0750 root root -"
        "d ${stateDir}/backups 0750 root root -"
      ];

      systemd.services.podman-looty-backend = {
        after = [ "network-online.target" ] ++ lib.optional cfg.database.createLocally "looty-database.service";
        requires = lib.optional cfg.database.createLocally "looty-database.service";
        wants = [ "network-online.target" ];
      };
      systemd.services.podman-looty-collaboration = {
        after = [ "podman-looty-backend.service" ] ++ lib.optional cfg.database.createLocally "looty-database.service";
        requires = lib.optional cfg.database.createLocally "looty-database.service";
      };
    }

    (mkIf cfg.database.createLocally {
      services.postgresql = {
        enable = true;
        ensureDatabases = [ cfg.database.name ];
        ensureUsers = [
          {
            name = cfg.database.user;
            ensureDBOwnership = cfg.database.name == cfg.database.user;
          }
        ];
        # Connexion par mot de passe, depuis la machine seulement (les conteneurs sont sur son réseau).
        authentication = lib.mkAfter ''
          host ${cfg.database.name} ${cfg.database.user} 127.0.0.1/32 scram-sha-256
        '';
      };

      # Donne à l'utilisateur de la base le mot de passe DB_PASSWORD de environmentFile, et écrit
      # DATABASE_URL (avec ce mot de passe) dans un fichier de /run, lisible par root seulement.
      systemd.services.looty-database = {
        description = "Looty: database password and connection string";
        after = [
          "postgresql.service"
          "postgresql-setup.service"
        ];
        requires = [ "postgresql.service" ];
        wantedBy = [ "multi-user.target" ];
        path = [ config.services.postgresql.package ];
        serviceConfig = {
          Type = "oneshot";
          RemainAfterExit = true;
          EnvironmentFile = cfg.environmentFile;
          RuntimeDirectory = "looty";
          RuntimeDirectoryPreserve = true;
          UMask = "0077";
        };
        script = ''
          case "$DB_PASSWORD" in
            "" | *[!A-Za-z0-9]*) echo "looty: DB_PASSWORD must be set in the environment file, letters and digits only" >&2; exit 1 ;;
          esac
          ${pkgs.util-linux}/bin/runuser -u postgres -- psql -v ON_ERROR_STOP=1 -c "ALTER ROLE \"${cfg.database.user}\" PASSWORD '$DB_PASSWORD'"
          printf 'DATABASE_URL=%s\n' "${dbUrl}" > ${runDir}/database.env
        '';
      };
    })

    (mkIf cfg.backup.enable {
      services.postgresqlBackup = mkIf cfg.database.createLocally {
        enable = true;
        databases = [ cfg.database.name ];
      };
    })

    (mkIf (cfg.proxy == "caddy") {
      services.caddy.enable = true;
      # Même routage que deploy/templates/Caddyfile.
      services.caddy.virtualHosts.${cfg.domain}.extraConfig = ''
        handle_path /collab* {
          reverse_proxy 127.0.0.1:${toString cfg.ports.collaboration}
        }
        @backend path /api* /uploads* /pages* /dav* /.well-known/caldav
        handle @backend {
          reverse_proxy 127.0.0.1:${toString cfg.ports.backend}
        }
        handle {
          reverse_proxy 127.0.0.1:${toString cfg.ports.frontend}
        }
      '';
      services.caddy.virtualHosts.${authDomain} = mkIf bundled {
        extraConfig = "reverse_proxy 127.0.0.1:${toString config.services.pocket-id.settings.PORT}";
      };
      networking.firewall.allowedTCPPorts = mkIf cfg.openFirewall [
        80
        443
      ];
      networking.firewall.allowedUDPPorts = mkIf cfg.openFirewall [ 443 ];
    })

    (mkIf (cfg.proxy == "nginx") {
      services.nginx = {
        enable = true;
        recommendedProxySettings = true;
        recommendedTlsSettings = true;
        virtualHosts.${cfg.domain} = {
          enableACME = true;
          forceSSL = true;
          # Téléversements de fichiers et WebSockets.
          extraConfig = "client_max_body_size 0;";
          locations =
            let
              backend = {
                proxyPass = "http://127.0.0.1:${toString cfg.ports.backend}";
                proxyWebsockets = true;
              };
            in
            {
              "/".proxyPass = "http://127.0.0.1:${toString cfg.ports.frontend}";
              "/".proxyWebsockets = true;
              "/api" = backend;
              "/uploads" = backend;
              "/pages" = backend;
              "/dav" = backend;
              "= /.well-known/caldav" = backend;
              "/collab" = {
                proxyPass = "http://127.0.0.1:${toString cfg.ports.collaboration}";
                proxyWebsockets = true;
                extraConfig = "rewrite ^/collab/?(.*)$ /$1 break;";
              };
            };
        };
        virtualHosts.${authDomain} = mkIf bundled {
          enableACME = true;
          forceSSL = true;
          locations."/".proxyPass = "http://127.0.0.1:${toString config.services.pocket-id.settings.PORT}";
        };
      };
      networking.firewall.allowedTCPPorts = mkIf cfg.openFirewall [
        80
        443
      ];
    })

    (mkIf bundled {
      services.pocket-id = {
        enable = true;
        settings = {
          APP_URL = "https://${authDomain}";
          TRUST_PROXY = true;
          ANALYTICS_DISABLED = true;
          PORT = lib.mkDefault 1411;
          HOST = lib.mkDefault "127.0.0.1";
        };
        # ENCRYPTION_KEY et STATIC_API_KEY (POCKET_ID_*) sont renommées pour Pocket ID par le service ci-dessous.
        environmentFile = "${runDir}/pocket-id.env";
      };
      # Looty joint https://auth.<domaine> depuis l'hôte : ce nom mène à la machine elle-même.
      networking.hosts."127.0.0.1" = [ authDomain ];

      systemd.services.looty-pocket-id-env = {
        description = "Looty: Pocket ID secrets from the environment file";
        before = [ "pocket-id.service" ];
        requiredBy = [ "pocket-id.service" ];
        serviceConfig = {
          Type = "oneshot";
          RemainAfterExit = true;
          EnvironmentFile = cfg.environmentFile;
          RuntimeDirectory = "looty";
          RuntimeDirectoryPreserve = true;
          UMask = "0077";
        };
        script = ''
          : "''${POCKET_ID_ENCRYPTION_KEY:?POCKET_ID_ENCRYPTION_KEY must be set in the environment file}"
          printf 'ENCRYPTION_KEY=%s\nSTATIC_API_KEY=%s\n' "$POCKET_ID_ENCRYPTION_KEY" "''${POCKET_ID_STATIC_API_KEY:-}" > ${runDir}/pocket-id.env
          chmod 0644 ${runDir}/pocket-id.env
        '';
      };

      # Crée le client OIDC de Looty (même script que le service pocket-id-init de Docker Compose).
      systemd.services.looty-pocket-id-init = {
        description = "Looty: create the OIDC client in Pocket ID";
        after = [ "pocket-id.service" ];
        requires = [ "pocket-id.service" ];
        before = [ "podman-looty-backend.service" ];
        wantedBy = [ "multi-user.target" ];
        path = [
          pkgs.curl
          pkgs.coreutils
          pkgs.gnugrep
          pkgs.gnused
        ];
        environment = {
          POCKET_ID_URL = "http://127.0.0.1:${toString config.services.pocket-id.settings.PORT}";
          OIDC_CLIENT_ID = "looty";
          BASE_URL = baseUrl;
          AUTH_DOMAIN = authDomain;
        };
        serviceConfig = {
          Type = "oneshot";
          RemainAfterExit = true;
          EnvironmentFile = cfg.environmentFile;
          # Pocket ID démarre avant d'écouter : réessayer jusqu'à ce qu'il réponde.
          ExecStartPre = "${pkgs.runtimeShell} -c 'for i in $(seq 60); do ${pkgs.curl}/bin/curl -sf -o /dev/null http://127.0.0.1:${toString config.services.pocket-id.settings.PORT}/healthz && exit 0; sleep 1; done; exit 1'";
          ExecStart = pocketIdInit;
          PrivateTmp = true;
          NoNewPrivileges = true;
          ProtectSystem = "strict";
          ProtectHome = true;
        };
      };
    })
  ]);
}
