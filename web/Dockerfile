FROM node:22-bookworm-slim AS builder

WORKDIR /app
COPY web/package.json web/package-lock.json ./
RUN npm ci

COPY web ./
ENV VITE_AURA_API_URL=/api
RUN npm run build

FROM nginx:1.27-alpine AS runtime

COPY web/nginx.conf /etc/nginx/conf.d/default.conf
COPY --from=builder /app/dist /usr/share/nginx/html

EXPOSE 80
