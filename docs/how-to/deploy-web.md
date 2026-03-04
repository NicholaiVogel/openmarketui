# How to Deploy the Landing Page

The `web/` package is an Astro 5 site deployed to Cloudflare Pages. This guide covers local development and production deployment.

---

## Local development

```bash
bun run web
# or
just web
```

The dev server starts at `http://localhost:4321` (Astro's default). Changes to files in `web/src/` hot-reload automatically.

---

## Building for production

```bash
bun run build
```

This runs Turbo across all JS packages. The web package output lands in `web/dist/`. Verify the build output looks correct before deploying:

```bash
ls web/dist/
```

---

## Deploying to Cloudflare Pages

The site deploys to Cloudflare Pages. You need the Cloudflare `wrangler` CLI authenticated to the Cloudflare account that owns the Pages project.

**First-time setup** (if the Pages project doesn't exist):

```bash
cd web
bunx wrangler pages project create openmarketui-web
```

**Deploying**:

From the repo root:

```bash
bun run build
cd web
bunx wrangler pages deploy dist --project-name openmarketui-web --branch main
```

Or if a `bun deploy` script is configured in `web/package.json`:

```bash
bun deploy
```

Production URL is configured in the Cloudflare Pages dashboard. Custom domains are set there as well.

---

## Environment variables

The landing page is static — there are no server-side environment variables. Any dynamic content (market data, live stats) would require client-side fetching from the engine's REST API.

If you add dynamic features that need environment variables at build time, configure them in:
1. Local: `.env` file in `web/`
2. CI/Production: Cloudflare Pages dashboard → Settings → Environment variables

---

## Updating the navigation

The navigation component is at `web/src/layouts/Layout.astro` (or `web/src/components/` depending on structure). It's a static list of links — edit the anchors directly.

---

## What the web package is not

The `web/` package is a **marketing landing page**, not the trading dashboard. It doesn't connect to the trading engine, display live positions, or require authentication.

The terminal UI for live monitoring is **Watchtower** (`watchtower/`). The REST API and WebSocket for the engine are served by `pm-server` (port 3030 during paper trading).
