# Site de Looty : pages (Astro, statiques) + serveur Rust. Image finale vide (« scratch ») : un binaire
# statique et les pages, quelques Mo de mémoire. Les fichiers publiés vivent dans le volume /data.
FROM node:22-alpine AS web
WORKDIR /w
COPY web/package.json web/package-lock.json ./
RUN npm ci --no-audit --no-fund
COPY web/ ./
RUN npm run build

FROM rust:1-alpine AS server
RUN apk add --no-cache musl-dev
WORKDIR /s
COPY server/ ./
RUN cargo build --release --locked && mkdir /data

FROM scratch
COPY --from=server /s/target/release/looty-site /looty-site
COPY --from=web /w/dist /web
COPY --from=server --chown=10001:10001 /data /data
ENV WEB_DIR=/web DATA_DIR=/data PORT=8080
USER 10001:10001
EXPOSE 8080
VOLUME /data
ENTRYPOINT ["/looty-site"]
