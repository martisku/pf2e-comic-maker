# 0001: PostgreSQL and object storage on Neon

Date: 2026-10-15
Status: Accepted; prototype implementation available, live provider validation pending.

## Context

The app must preserve project data and images across deployments. The user wants
free services without providing a payment card. The existing SQLite and local
image files require a persistent server disk that the free demo host lacks.

## Decision

Use SQLx with PostgreSQL on Neon for project data and Neon Object Storage for
image files. Keep the native Rust/Dioxus stack and a Docker application service
whose local filesystem can be discarded. Use standard PostgreSQL and supported
S3 operations to retain portability. Select the application host separately
after confirming free deployment without card verification.

## Consequences

Database and image persistence must be migrated, with explicit coordination
between database commits and object uploads. Free quotas, transfer, backup,
access protection, and provider availability need operational handling. No
application migration or hosting activation is performed by this decision.

Details: [hosting and storage target architecture](../architecture/hosting-and-storage.md).

Implementation: see [storage setup and recovery](../development/neon-storage.md).
