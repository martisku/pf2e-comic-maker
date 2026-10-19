# Render Free and Neon: first deployment

Accounts are reported created by the user on 2026-10-19. The service URL is
https://pf2e-comic-maker.onrender.com. This guide wires the
existing service to the application. Live account access, bucket settings, and
successful deployment have not been verified from this workspace.

## 1. Neon database and private images

Use the existing Free project. PostgreSQL and Object Storage should be in the
same project/branch, preferably Frankfurt (`aws-eu-central-1`). Do not enable
Functions, AI Gateway, Realtime, or Neon Auth for this prototype.

In **Connect**, copy the standard PostgreSQL connection URI with pooling on for
Render's `DATABASE_URL`. Copy its direct/non-pooled variant with pooling off for
GitHub's `COMIC_MIGRATION_DATABASE_URL`. Keep TLS enabled. Copy the URI itself,
not a surrounding `psql '…'` command. The interactive `psql -h pg.neon.tech`
shortcut is not the runtime connection configuration used by this Rust service.

Create a dedicated **private** Object Storage bucket, for example `comic-images`.
Create a scoped branch credential with object read, PUT and DELETE permissions.
Use the endpoint and signing region shown by Neon. The credential names from
`neon env pull` map to the app as follows:

| Neon value | Render environment variable |
| --- | --- |
| `AWS_ENDPOINT_URL_S3` | `COMIC_S3_ENDPOINT` |
| `AWS_REGION` | `COMIC_S3_REGION` |
| `AWS_ACCESS_KEY_ID` / credential `token_id` | `COMIC_S3_ACCESS_KEY_ID` |
| `AWS_SECRET_ACCESS_KEY` / `s3_secret_access_key` | `COMIC_S3_SECRET_ACCESS_KEY` |
| Private bucket name | `COMIC_S3_BUCKET` |

See [storage configuration](neon-storage.md) for the full contract. Keep secrets
in provider settings; do not paste them into chat or commit them.

## 2. Configure the existing Render Web Service

Keep Docker, branch `main`, Dockerfile `./Dockerfile`, empty root directory,
Free instance, health path `/healthz`, and the default Docker command. No disk,
Render database, paid pre-deploy command, or shell is required.

Set **Settings → Auto-Deploy → Off**. GitHub Actions will trigger deploys after
verification and schema initialization; Render's independent on-commit deploy
must not run before that sequence. Saving environment changes can itself trigger
a deploy: if the initial database is empty it will fail until step 4 initializes
it. Do not change the Docker command to implicitly run migrations at startup.

Under **Environment**, add these values:

| Name | Value |
| --- | --- |
| `DATABASE_URL` | Pooled Neon PostgreSQL URI from step 1 |
| `COMIC_S3_ENDPOINT` | Branch HTTPS storage endpoint |
| `COMIC_S3_REGION` | Storage signing region from Neon |
| `COMIC_S3_BUCKET` | The private bucket name, e.g. `comic-images` |
| `COMIC_S3_ACCESS_KEY_ID` | Scoped storage access key/token ID |
| `COMIC_S3_SECRET_ACCESS_KEY` | Matching storage secret |
| `COMIC_USER` | Personal login name, e.g. `comic` |
| `COMIC_PASSWORD` | Unique random app login password, at least 16 bytes |
| `COMIC_ORIGIN` | `https://pf2e-comic-maker.onrender.com` (no trailing slash/path) |

Do not set `COMIC_INSECURE_LOCAL`. The Docker image supplies the listener on
`0.0.0.0:10000` and frontend directory. If you configured a different internal
port, align Render and `COMIC_BIND`. The application does not read `.env` itself.
The direct migration credential is not needed in the Render runtime.

Copy the service's secret **Deploy Hook** from Settings for the next step.
No account-wide Render API key is needed. A deploy hook authorizes deployments
of this service and must be treated as a secret.

## 3. Configure GitHub Actions

Repository **Settings → Secrets and variables → Actions → Secrets**:

| Repository secret | Value |
| --- | --- |
| `COMIC_MIGRATION_DATABASE_URL` | Direct/non-pooled Neon PostgreSQL URI with schema permissions |
| `RENDER_DEPLOY_HOOK_URL` | Complete unmodified Render deploy hook URL |

Under **Variables**, add `RENDER_DEPLOY_ENABLED` with the exact value `true`
**after** Render's environment and both secrets are configured.

Without that variable the workflow only verifies and clearly reports that
deployment is disabled. If enabled but a secret is missing or invalid, deployment
fails before database mutation. Pull requests never receive deployment secrets.

## 4. Run the first verified deployment

Go to **Actions → Verify durable storage → Run workflow**, choose **main**, and
run it. Subsequent pushes/merges to main run the same sequence automatically:

1. Format, unit tests, native/Wasm Clippy, and release Docker build.
2. PostgreSQL/S3 integration tests, desktop/phone browser checks, and comparison
   after replacing a container without a data volume.
3. Run `migrate` using the verified runtime image and direct database credential.
   This versions the schema and creates the fresh workspace without overwriting
   saved work. A migration failure prevents a deployment request.
4. POST the Render hook with `ref` set to the full tested commit SHA.

Main runs are serialized without cancelling an active migration/deployment
request; PR runs can cancel older PR checks. No object-storage or app login
secrets are needed in GitHub. Docker forwards only the migration URI to the
migration container. The hook URL and provider response body are not logged.

**An accepted or queued deploy request is not confirmation of a live release.**
The workflow reports Render's acceptance, then ends. In Render Events, confirm
the requested commit reaches **Live**. If a hook response is lost or invalid,
check Events before retrying: it may already have triggered a deploy.

The migration precedes the provider's build, so later schema changes must remain
compatible with the previous live application. A failed deploy does not roll
back its already-applied schema. Never add destructive schema changes casually.

## 5. Verify the running application

Open the public Render URL, log in with `COMIC_USER` / `COMIC_PASSWORD`, edit a
panel, upload an image, and reload. Confirm both are saved. Restart the service
and confirm they still exist. The first request after idle suspension may take
about a minute. If login works but an upload fails, verify the private bucket,
branch, credentials, endpoint, and signing region.

`/healthz` verifies process liveness. Login, editing and uploading check the
actual PostgreSQL and storage path. Do not run the destructive sample-data
browser smoke script against a workspace containing real work.

## References

Official provider documentation checked 2026-10-19:

- [Render deploy hooks and exact commit selection](https://render.com/docs/deploy-hooks)
- [Render deployment and auto-deploy settings](https://render.com/docs/deploys)
- [Free service limits](https://render.com/docs/free)
- [Neon S3 authentication](https://neon.com/docs/storage/authentication)
- [Neon connection pooling](https://neon.com/docs/connect/connection-pooling)
