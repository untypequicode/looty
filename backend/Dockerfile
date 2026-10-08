FROM rust:slim-bookworm as builder
WORKDIR /app
RUN apt-get update && apt-get install -y pkg-config libssl-dev
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
WORKDIR /app
# Compilation des projets LaTeX sur le serveur (TeX Live, ~1 Go).
# Image plus légère sans : docker build --build-arg WITH_LATEX=0
ARG WITH_LATEX=1
RUN apt-get update && apt-get install -y --no-install-recommends libssl3 ca-certificates \
 && if [ "$WITH_LATEX" = "1" ]; then apt-get install -y --no-install-recommends \
      latexmk texlive-latex-base texlive-latex-recommended texlive-latex-extra texlive-fonts-recommended \
      texlive-lang-french texlive-lang-english texlive-xetex texlive-luatex texlive-bibtex-extra texlive-science texlive-pictures biber lmodern; fi \
 && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/backend /app/backend
EXPOSE 8080
CMD ["/app/backend"]
