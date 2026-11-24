# Site de Looty : pages (Astro, statiques) + serveur Rust. Image finale vide (« scratch ») : un binaire
# statique et les pages, quelques Mo de mémoire. Les fichiers publiés vivent dans le volume /data.
# Le contexte est la racine du dépôt (docker build -f site/Dockerfile .) : la page /install/ embarque les modèles de
# deploy/ (les mêmes que render.sh et install.sh) et le module deploy/generate.mjs.
FROM node:24-alpine AS web
WORKDIR /w/site/web
COPY site/web/package.json site/web/package-lock.json ./
RUN npm ci --no-audit --no-fund
COPY deploy/generate.mjs /w/deploy/generate.mjs
COPY deploy/templates/ /w/deploy/templates/
COPY site/web/ ./
RUN npm run build

FROM rust:1-alpine AS server
RUN apk add --no-cache musl-dev
WORKDIR /s
COPY site/server/ ./
RUN cargo build --release --locked && mkdir /data

FROM scratch
COPY --from=server /s/target/release/looty-site /looty-site
COPY --from=web /w/site/web/dist /web
COPY --from=server --chown=10001:10001 /data /data
ENV WEB_DIR=/web DATA_DIR=/data PORT=8080
USER 10001:10001
EXPOSE 8080
VOLUME /data
ENTRYPOINT ["/looty-site"]
