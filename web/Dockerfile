# syntax=docker/dockerfile:1

# Build the static postmortem site. Build context is the repo root: the
# build reads web/ and the published RCAs under rcas/. Only incidents
# flagged `published = true` — and only their client-safe sections —
# reach the output, so the image is safe to ship publicly.
#
#   docker build -f web/Dockerfile -t beagle-web .
#
# Content is baked in at build time; rebuild to publish new incidents.
FROM node:22-alpine AS build
WORKDIR /src
COPY web/package.json web/package-lock.json ./web/
RUN cd web && npm ci
COPY web/ ./web/
COPY rcas/ ./rcas/
ARG BEAGLE_SITE_URL=""
ENV BEAGLE_SITE_URL=${BEAGLE_SITE_URL}
RUN cd web && npm run build

# Serve the static output with nginx on :8080 (unprivileged port).
FROM nginx:1.27-alpine AS serve
COPY web/nginx.conf /etc/nginx/conf.d/default.conf
COPY --from=build /src/web/dist /usr/share/nginx/html
EXPOSE 8080
HEALTHCHECK --interval=30s --timeout=3s \
  CMD wget -q -O /dev/null http://localhost:8080/ || exit 1
