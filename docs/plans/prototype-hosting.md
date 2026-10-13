# Temporary prototype deployment

Status: Approved for implementation by the user's explicit request, 2026-10-13.

Requirement: [prototype hosting](../requirements/prototype-hosting.md).

1. Package the existing Axum server and Dioxus 0.7.10 frontend in a multistage
   Docker image. Run it as an unprivileged UID with writable demo storage.
2. Declare a Render Free Docker service, listening on port 10000 with an API
   readiness check. Disable independent auto-deploys.
3. Run the documented Rust verification commands while building the image in
   GitHub Actions, then smoke test its HTTP API and frontend startup.
4. On `main` only, POST to a secret Render deploy hook with the verified commit
   SHA. Serialize runs, fail clearly when the hook is missing, and distinguish
   request acceptance from successful live deployment.
5. Document activation, public shared access, idle startup, and ephemeral data.

Render Free and Koyeb Free were checked against their official documentation;
neither provides an attached persistent disk on its free web instance. Render
was selected for its Docker support, declarative blueprint, and documented
GitHub Actions deploy hook. GitHub Pages cannot run this native Rust API.

Review: the existing application can be packaged without changing its data
model or adding dependencies. The substantial limitation is that this free
configuration resets stored data; it is suitable only for disposable testing.
No hosting account is connected to this environment. Account activation and
the deploy hook remain external prerequisites.

Verification: build the image, run its HTTP startup check, validate YAML and
workflow syntax, and inspect the diff. A live deploy cannot be claimed until
Render account activation and successful service deployment are observed.
