# Deploy verified changes to Render Free

Status: Approved by the original request for automatic GitHub Actions deployment
and standing authorization to plan, implement, and push. Accounts reported set up
by the user on 2026-10-19; service configuration and live access remain unverified.

- Deploy only a verified commit from `main`, automatically after its checks pass.
- Initialize/version the PostgreSQL schema explicitly before triggering deploy.
  Failure must prevent a new deployment request.
- Use the existing Docker runtime and Neon PostgreSQL/private object storage.
  Do not provision paid resources, add disks, or require payment cards.
- Keep credentials in Render/GitHub secrets and out of build layers and logs.
- Pull requests must never receive production credentials or deploy.
- Activation requires the existing service's configuration; verification must
  remain usable before those external settings are supplied.
- Distinguish a deploy request accepted/queued by Render from a service confirmed
  live. Initial live login/save/upload/restart verification remains required.
