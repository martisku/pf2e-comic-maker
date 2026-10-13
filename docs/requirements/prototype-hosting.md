# Temporary prototype hosting

The user requested a free hosting service and GitHub Actions deployment on
2026-10-13, explicitly authorizing implementation of the deployment setup.

## Scope and acceptance criteria

- Host both the browser UI and existing Rust API on one origin.
- Declare a free service without provisioning paid infrastructure.
- Verify tests, formatting, linting, release builds, and runtime startup before
  automatically requesting deployment on pushes to `main`.
- Pull requests must verify without deploying or accessing deploy credentials.
- Deploy the verified commit, not an arbitrary newer branch tip.
- Keep hosting credentials outside the repository.
- Document one-time account setup and hosting limits, including data retention
  and the existing prototype's lack of authentication.

External account activation is required to verify a live deployment. Durable
hosting for actual comic projects is not part of the disposable demo setup.
