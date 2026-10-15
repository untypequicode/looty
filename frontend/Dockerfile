FROM node:24-slim AS builder
WORKDIR /app
COPY package.json package-lock.json .npmrc ./
RUN npm ci --no-audit --no-fund
COPY . .
# Le build (Mermaid, éditeurs, Pyodide…) dépasse les ~2 Go de mémoire par défaut de Node.
ENV NODE_OPTIONS=--max-old-space-size=4096
RUN npm run build

FROM node:24-slim
WORKDIR /app
COPY --from=builder /app/.output /app/.output
EXPOSE 3000
CMD ["node", ".output/server/index.mjs"]
