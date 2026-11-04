{ pkgs ? import <nixpkgs> {} }:

pkgs.mkShell {
  buildInputs = with pkgs; [
    # Backend (Rust)
    rustc
    cargo
    rustfmt
    clippy
    rust-analyzer
    pkg-config
    openssl

    # Frontend (Node.js/Nuxt). Node 26.10's ARM64 V8 build currently misses
    # <climits>; keep the fix local so both supported Nix architectures build.
    (nodejs_26.overrideAttrs (old: {
      postPatch = (old.postPatch or "") + ''
        substituteInPlace deps/v8/src/base/memcopy.h \
          --replace-fail '#include <arm_neon.h>' '#include <arm_neon.h>
#include <climits>'
      '';
    }))

    # Base de données & Infra
    postgresql_15
    docker-compose
  ];

  shellHook = ''
    echo "========================================"
    echo "  Looty - Dev Environment   "
    echo "========================================"
    echo "Rust: $(rustc --version)"
    echo "Node: $(node --version)"
    echo "Prêt pour le développement !"
  '';
}
