# Test NixOS du module services.looty (nix flake check, ou nix build .#checks.x86_64-linux.nixos-module).
# La machine virtuelle n'a pas de réseau : les trois images de Looty sont remplacées par de petites images locales
# qui répondent sur les mêmes ports (services.looty.imageFiles). Le test vérifie ce que fait le module :
# PostgreSQL et son mot de passe, les unités, les secrets lus dans le fichier d'environnement, et le routage du proxy.
# L'API et la collaboration réelles sont vérifiées par deploy/test/stack.sh (Docker Compose, en CI).
{ pkgs, lib, ... }:
let
  stub =
    name: body:
    pkgs.dockerTools.buildImage {
      name = "ghcr.io/untypequicode/looty-${name}";
      tag = "latest";
      copyToRoot = pkgs.buildEnv {
        name = "stub-${name}";
        paths = [
          pkgs.busybox
          (pkgs.runCommand "www-${name}" { } ''
            mkdir -p $out/www/api
            echo ${body} > $out/www/index.html
            echo ${body} > $out/www/api/health
            echo ${body} > $out/www/health
          '')
        ];
      };
      config.Cmd = [
        "/bin/sh"
        "-c"
        "exec httpd -f -p $PORT -h /www"
      ];
    };
in
{
  name = "looty-module";

  nodes.bundled =
    { pkgs, ... }:
    {
      virtualisation.memorySize = 2048;
      services.looty = {
        enable = true;
        domain = "looty.test";
        auth = "bundled-oidc";
        proxy = "nginx";
        environmentFile = pkgs.writeText "looty.env" ''
          DB_PASSWORD=dummydbpassword
          LOOTY_SECRET_KEY=dummy-looty-key
          OIDC_CLIENT_SECRET=dummy-client-secret-for-test
          OIDC_PROVIDER_ID=3f1c1f6e-8a52-4a4c-9d0e-6a1b2c3d4e5f
          POCKET_ID_ENCRYPTION_KEY=dummy-encryption-key-for-the-test-only
          POCKET_ID_STATIC_API_KEY=dummy-static-api-key-for-the-test-only
        '';
        imageFiles = {
          backend = stub "backend" "backend";
          frontend = stub "frontend" "frontend";
          collaboration = stub "collaboration" "collab";
        };
      };
      # Certificats : pas d'accès à Let's Encrypt dans le test.
      security.acme.acceptTerms = true;
      security.acme.defaults.email = "test@looty.test";
      security.acme.certs = lib.mkForce { };
      services.nginx.virtualHosts."looty.test" = { enableACME = lib.mkForce false; forceSSL = lib.mkForce false; };
      services.nginx.virtualHosts."auth.looty.test" = { enableACME = lib.mkForce false; forceSSL = lib.mkForce false; };
    };

  nodes.machine =
    { pkgs, ... }:
    {
      virtualisation.memorySize = 2048;
      services.looty = {
        enable = true;
        domain = "looty.test";
        instanceName = "Test";
        environmentFile = pkgs.writeText "looty.env" ''
          DB_PASSWORD=dummydbpassword
          LOOTY_SECRET_KEY=dummy-looty-key
          OIDC_ISSUER=https://idp.test
          OIDC_CLIENT_ID=looty
          OIDC_CLIENT_SECRET=dummy-client-secret-for-test
        '';
        imageFiles = {
          backend = stub "backend" "backend";
          frontend = stub "frontend" "frontend";
          collaboration = stub "collaboration" "collab";
        };
      };
      services.caddy.globalConfig = "local_certs";
      networking.hosts."127.0.0.1" = [ "looty.test" ];
    };

  testScript = ''
    start_all()
    bundled.wait_for_unit("looty-pocket-id-init.service")
    # Le client OIDC de Looty existe dans Pocket ID, avec la bonne URL de redirection.
    client = bundled.succeed("curl -sf -H 'X-API-Key: dummy-static-api-key-for-the-test-only' http://127.0.0.1:1411/api/oidc/clients/looty")
    assert "https://looty.test/api/auth/callback/3f1c1f6e-8a52-4a4c-9d0e-6a1b2c3d4e5f" in client, client
    bundled.wait_for_unit("podman-looty-backend.service")
    env = bundled.succeed("podman inspect looty-backend --format '{{.Config.Env}}'")
    assert "OIDC_ISSUER=https://auth.looty.test" in env, env
    # Relancé, il ne change rien.
    assert "existe déjà" in bundled.succeed("systemctl restart looty-pocket-id-init.service; journalctl -u looty-pocket-id-init -n 20 --no-pager")

    machine.wait_for_unit("looty-database.service")
    machine.wait_for_unit("podman-looty-backend.service")
    machine.wait_for_unit("podman-looty-collaboration.service")
    machine.wait_for_unit("podman-looty-frontend.service")
    machine.wait_for_unit("caddy.service")

    # PostgreSQL : la base existe et le mot de passe du fichier d'environnement fonctionne par TCP.
    machine.succeed("PGPASSWORD=dummydbpassword ${pkgs.postgresql}/bin/psql -h 127.0.0.1 -U looty looty_db -c 'select 1'")
    machine.fail("PGPASSWORD=wrong ${pkgs.postgresql}/bin/psql -h 127.0.0.1 -U looty looty_db -c 'select 1'")

    # Les secrets sont dans /run, pas dans le magasin, et l'URL de la base les contient.
    machine.succeed("grep -q 'postgres://looty:dummydbpassword@127.0.0.1' /run/looty/database.env")
    machine.succeed("[ \"$(stat -c %a /run/looty/database.env)\" = 600 ]")
    machine.fail("grep -rl dummydbpassword /nix/store --include=*.service 2>/dev/null | grep .")

    # Le conteneur du serveur reçoit les variables.
    env = machine.succeed("podman inspect looty-backend --format '{{.Config.Env}}'")
    assert "BASE_URL=https://looty.test" in env, env

    # Routage du proxy, comme deploy/templates/Caddyfile.
    def get(path):
        return machine.succeed(f"curl -sk --resolve looty.test:443:127.0.0.1 https://looty.test{path}").strip()

    assert get("/") == "frontend", get("/")
    assert get("/api/health") == "backend", get("/api/health")
    assert get("/collab/health") == "collab", get("/collab/health")
  '';
}
