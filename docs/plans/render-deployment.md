# Render deployment plan

Status: Implemented; activation and live deployment verification pending.
Implementation authorized through the user's standing instruction.
Requirement: [Render deployment](../requirements/render-deployment.md).

## Inspected baseline and review

The current workflow builds/tests a Docker image and exercises fresh-container
persistence, but does not deploy. `comic-server migrate` works without S3 or UI
credentials and uses a direct PostgreSQL URI. Normal server startup only checks
an existing workspace. Render Free has neither pre-deploy commands nor a shell.
The application accounts are not connected to this execution environment.

A scoped deploy hook is sufficient; do not add an account-wide Render API key.
Run the migration using the same locally verified runtime image and send the
full tested SHA as the hook's `ref`. Serialise main workflows without cancelling
an in-progress migration/deploy. PR verification remains cancellable.

## Implementation

1. Add opt-in main-only migration/deploy after all existing verification steps.
   Use repository variable `RENDER_DEPLOY_ENABLED=true` once configured, and
   repository secrets `COMIC_MIGRATION_DATABASE_URL` and `RENDER_DEPLOY_HOOK_URL`.
2. Validate trusted event/ref, SHA, direct database URI, and Render hook before
   any mutation. Pass the database URL through Docker's environment forwarding;
   suppress provider response bodies and secret-bearing exceptions.
3. Run `comic-runtime:ci migrate`; stop on failure. POST the hook with the tested
   SHA without retrying an ambiguous deployment request. Report HTTP 200/202 as
   accepted/queued, not proof of a live new release.
4. Document exact Render environment names, private bucket credentials, disabling
   Render auto-deploy so it cannot race ahead of tests, first activation, and live
   verification. Keep runtime secrets out of GitHub except the migration URI.
5. Test migration-before-trigger ordering, failure barriers, event guards, exact
   commit selection, queued responses, and secret-free failures with mocks. Run
   workflow linting and Python checks; CI retains full Docker/browser validation.

No runtime application behavior or SQL schema changes are needed. Later schema
changes must remain compatible with the currently live release: the migration
can succeed even if the subsequent deploy fails. No automatic database rollback.

## Verification

Five credential-free Python tests passed for ordering, migration failure, trusted
events, invalid configuration, commit pinning, queued responses, and ambiguous
provider results without secret leakage or retries. Actionlint passed. Existing
Docker/Postgres/S3/browser checks remain in the workflow. No live schema change
or provider request has been made; account secrets are not available here.
