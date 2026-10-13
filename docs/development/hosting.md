# Temporary free hosting

Render Free runs the existing Dioxus/Wasm frontend and Axum server together in
one Docker web service. GitHub Actions checks formatting, tests, Clippy, release
builds, and runtime startup before requesting deployment of the verified commit.
Pull requests run verification only. Pushes to `main` and manual runs on `main`
also request deployment. Render builds the same commit using the Dockerfile.

## Limits

This configuration is a disposable demo, not durable project storage. Render's
free service has an ephemeral filesystem: SQLite edits and uploaded images are
lost on deploy, restart, or idle spin-down. It sleeps after 15 minutes without
traffic, and the next visitor waits for startup. There is no authentication in
this prototype: anyone with the public URL can read and edit the same workspace
and upload images. Use sample data only.

GitHub Actions usage and Render usage are subject to their account limits. No
paid services or disks are declared. Preserving real projects requires separate
durable storage or a persistent server; that is outside this demo configuration.

## One-time activation

1. Push the deployment files to the repository's `main` branch.
2. Sign in to Render, choose **New > Blueprint**, and connect
   `martisku/pf2e-comic-maker`. Select the branch containing `render.yaml`.
   Confirm the service uses the **Free** plan. Initial creation starts a deploy.
3. Leave service Auto-Deploy **Off** so GitHub Actions controls subsequent
   deploys. Disable Blueprint **Auto Sync** too, so blueprint changes cannot
   deploy independently of CI; sync intentional infrastructure changes manually.
4. Under the service's **Settings > Deploy Hook**, copy the secret URL.
5. In GitHub **Settings > Secrets and variables > Actions**, create repository
   secret `RENDER_DEPLOY_HOOK_URL` with that URL. Do not commit it or paste it
   into chat.
6. Run **Actions > Verify and deploy to Render > Run workflow** on `main`.
   Future pushes to `main` request deploys automatically after verification.
7. Check the Render Events page for a successful deployment and open the
   assigned HTTPS `onrender.com` URL. A green Actions deploy job means Render
   accepted the request, not that Render's build has finished or is live.

The deploy hook includes `ref=<verified commit SHA>` to avoid deploying a newer,
untested commit. Runs are serialized per branch and are not cancelled mid-deploy.

## Local container verification

```sh
docker build -t comic-prototype .
docker run --rm -p 10000:10000 comic-prototype
```

Open `http://localhost:10000`. To retain local test data across container
restarts, mount a writable directory owned by UID 10001 at `/app/data`.

Official references (checked 2026-10-13):

- [Render free-service limits](https://render.com/docs/free)
- [Render Docker services](https://render.com/docs/docker)
- [Deploy hooks and commit selection](https://render.com/docs/deploy-hooks)
- [Blueprint specification](https://render.com/docs/blueprint-spec)
