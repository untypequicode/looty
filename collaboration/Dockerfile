FROM node:24-slim

WORKDIR /app
COPY package.json package-lock.json ./
RUN npm ci --omit=dev --no-audit --no-fund
COPY . .

EXPOSE 8082
ENV NODE_OPTIONS=--max-old-space-size=384
# Sonde de vie (pas de curl dans node:slim). Traefik retire les conteneurs non sains : courte et peu coûteuse ; ne pas l'espacer, il attend qu'elle passe pour router /collab.
HEALTHCHECK --interval=10s --timeout=3s --start-period=30s --start-interval=2s --retries=3 \
  CMD node -e "fetch('http://127.0.0.1:'+(process.env.PORT||8082)+'/health').then(r=>process.exit(r.ok?0:1),()=>process.exit(1))"
CMD ["node", "index.js"]
