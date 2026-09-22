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

    # Frontend (Node.js/Nuxt)
    nodejs_26
    pnpm

    # Base de données & Infra
    postgresql_15
    docker-compose
  ];

  shellHook = ''
    echo "========================================"
    echo "  App Collaborative - Dev Environment   "
    echo "========================================"
    echo "Rust: $(rustc --version)"
    echo "Node: $(node --version)"
    echo "Prêt pour le développement !"
  '';
}
