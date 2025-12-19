FROM node:20-alpine AS deps
WORKDIR /repo

# Monorepo workspaces install (cached layer)
COPY package.json package-lock.json ./
COPY inventiv-frontend/package.json inventiv-frontend/package.json
COPY inventiv-ui/ia-widgets/package.json inventiv-ui/ia-widgets/package.json
RUN npm ci

FROM node:20-alpine AS builder
WORKDIR /repo
ENV NODE_ENV=production

COPY --from=deps /repo/node_modules ./node_modules
COPY package.json package-lock.json ./
COPY inventiv-frontend ./inventiv-frontend
COPY inventiv-ui/ia-widgets ./inventiv-ui/ia-widgets

RUN npm -w inventiv-frontend run build

FROM node:20-alpine AS runner
WORKDIR /app
ENV NODE_ENV=production
ENV PORT=3000

# Minimal runtime layout:
# - /app/node_modules (root workspace deps)
# - /app/inventiv-frontend (built Next app)
COPY --from=builder /repo/node_modules ./node_modules
COPY --from=builder /repo/inventiv-frontend/public ./inventiv-frontend/public
COPY --from=builder /repo/inventiv-frontend/.next ./inventiv-frontend/.next
COPY --from=builder /repo/inventiv-frontend/next.config.ts ./inventiv-frontend/next.config.ts
COPY --from=builder /repo/inventiv-frontend/package.json ./inventiv-frontend/package.json

EXPOSE 3000

WORKDIR /app/inventiv-frontend
CMD ["node", "/app/node_modules/next/dist/bin/next", "start", "--port", "3000", "--hostname", "0.0.0.0"]
