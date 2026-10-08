FROM node:24-slim AS builder
WORKDIR /app
COPY package.json ./
RUN npm install --legacy-peer-deps
COPY . .
RUN npm run build

FROM node:24-slim
WORKDIR /app
COPY --from=builder /app/.output /app/.output
EXPOSE 3000
CMD ["node", ".output/server/index.mjs"]
