FROM rust:slim-trixie AS builder
WORKDIR /app
RUN apt-get update && apt-get install -y pkg-config libssl-dev
# Dépendances d'abord (couche en cache tant que Cargo.toml et Cargo.lock ne changent pas) : seul le code du serveur
# est recompilé à chaque commit. La crate factice est nettoyée (cargo clean -p, indépendant de l'organisation de target/) pour que cargo reconstruise le vrai code.
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo 'fn main() {}' > src/main.rs \
 && cargo build --release --locked \
 && cargo clean --release -p backend \
 && rm -rf src
COPY . .
RUN cargo build --release --locked

FROM debian:trixie-slim
WORKDIR /app
# Compilation des projets LaTeX sur le serveur (TeX Live, ~1 Go).
# Image plus légère sans : docker build --build-arg WITH_LATEX=0
ARG WITH_LATEX=1
RUN apt-get update && apt-get install -y --no-install-recommends libssl3t64 ca-certificates \
 && if [ "$WITH_LATEX" = "1" ]; then apt-get install -y --no-install-recommends \
      latexmk texlive-latex-base texlive-latex-recommended texlive-latex-extra texlive-fonts-recommended \
      texlive-lang-french texlive-lang-english texlive-xetex texlive-luatex texlive-bibtex-extra texlive-science texlive-pictures biber lmodern; fi \
 && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/backend /app/backend
# `docker compose exec backend looty doctor` : diagnostic de l'installation.
RUN ln -s /app/backend /usr/local/bin/looty
EXPOSE 8080
CMD ["/app/backend"]
