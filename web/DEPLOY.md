# Deploying the beagle postmortem site

The site is a static build, so you can deploy it three ways — as raw
files, as a container, or on Kubernetes — and gate it with GitHub SSO
whenever you want. Content (published RCAs) is baked in at build time;
rebuild to publish new incidents.

## 1. Static files (simplest)

```sh
cd web
npm ci
BEAGLE_SITE_URL=https://postmortems.example.com npm run build
# → web/dist/  — upload anywhere: Vercel, Netlify, Pages, S3, nginx, …
```

`BEAGLE_RCAS_DIR` points the build at a store other than the repo's
`../rcas` (e.g. an oncall checkout).

## 2. Docker

Build from the repo root (the build reads `web/` and `rcas/`):

```sh
docker build -f web/Dockerfile -t beagle-web .
docker run -p 8080:8080 beagle-web        # → http://localhost:8080
```

Or use Compose. **Public:**

```sh
cd web
docker compose up --build                 # → http://localhost:8080
```

**Gated by GitHub SSO** (bring your own OAuth app):

```sh
cd web
cp .env.example .env                       # fill in your GitHub OAuth app
docker compose --profile auth up --build   # → http://localhost:4180
```

With auth on, reach the site through the proxy on `:4180`; do not expose
the un-gated web container (`:8080`) to the internet.

### Creating the GitHub OAuth app

GitHub → Settings → Developer settings → **OAuth Apps** → New OAuth App.
Set the **Authorization callback URL** to
`https://<your-host>/oauth2/callback` (or
`http://localhost:4180/oauth2/callback` locally). Put the client ID and
secret in `.env`, and generate a cookie secret with
`openssl rand -base64 32`. Restrict who can view with `GITHUB_ORG` /
`GITHUB_TEAM`.

## 3. Kubernetes (Helm)

The chart lives at `deploy/helm/beagle-web`. It deploys the image (built
above and pushed to a registry), an ingress, and — optionally — the
GitHub-SSO proxy.

**Public:**

```sh
helm install postmortems deploy/helm/beagle-web \
  --set image.repository=ghcr.io/you/beagle-web --set image.tag=0.1.0 \
  --set ingress.enabled=true --set ingress.host=postmortems.example.com
```

**Gated by GitHub SSO:**

```sh
helm install postmortems deploy/helm/beagle-web \
  --set image.repository=ghcr.io/you/beagle-web --set image.tag=0.1.0 \
  --set ingress.enabled=true --set ingress.host=postmortems.example.com \
  --set ingress.tlsSecret=postmortems-tls \
  --set auth.enabled=true \
  --set auth.github.clientId=<client-id> \
  --set auth.github.org=<your-org> \
  --set auth.redirectUrl=https://postmortems.example.com/oauth2/callback \
  --set auth.clientSecret=<client-secret> \
  --set auth.cookieSecret=$(openssl rand -base64 32)
```

When `auth.enabled=true` the ingress routes to oauth2-proxy, which
authenticates against GitHub and forwards to the site. Prefer keeping
secrets out of `--set`: put `client-secret` and `cookie-secret` in your
own Secret and pass `--set auth.existingSecret=<name>`. See
`deploy/helm/beagle-web/values.yaml` for every knob.

## Publishing the image (GHCR)

Pushing a `web-v*` tag builds and publishes the image to
`ghcr.io/<owner>/beagle-web` (`.github/workflows/web-image.yml`) — the
web analog of the `v*` (CLI) and `desktop-v*` (desktop) release flows.
