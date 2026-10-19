#!/usr/bin/env python3
"""Migrate with the verified image, then request the exact main commit on Render."""
import json
import os
import re
import subprocess
import sys
import urllib.error
import urllib.parse
import urllib.request


class DeploymentError(Exception):
    pass


def configuration(env):
    if env.get("GITHUB_REF") != "refs/heads/main" or env.get("GITHUB_EVENT_NAME") not in {
        "push", "workflow_dispatch"
    }:
        raise DeploymentError("Deployment requires a push or manual run on main.")
    sha = env.get("GITHUB_SHA", "")
    if not re.fullmatch(r"[0-9a-f]{40}", sha):
        raise DeploymentError("A full verified Git commit SHA is required.")
    database = env.get("COMIC_MIGRATION_DATABASE_URL", "")
    hook = env.get("RENDER_DEPLOY_HOOK_URL", "")
    if not database or not hook:
        raise DeploymentError(
            "Set COMIC_MIGRATION_DATABASE_URL and RENDER_DEPLOY_HOOK_URL in GitHub Actions secrets."
        )
    try:
        db = urllib.parse.urlsplit(database)
        url = urllib.parse.urlsplit(hook)
        query = urllib.parse.parse_qsl(url.query, keep_blank_values=True)
        if db.scheme not in {"postgres", "postgresql"} or not db.hostname or "-pooler" in db.hostname:
            raise ValueError()
        if (
            url.scheme != "https" or url.hostname != "api.render.com"
            or url.port not in {None, 443} or url.username or url.password or url.fragment
            or not re.fullmatch(r"/deploy/srv-[A-Za-z0-9_-]+", url.path)
            or len(query) != 1 or query[0][0] != "key" or not query[0][1]
        ):
            raise ValueError()
    except ValueError:
        raise DeploymentError("Use a direct PostgreSQL URI and an unmodified Render HTTPS deploy hook.") from None
    return sha, database, urllib.parse.urlunsplit(
        (url.scheme, url.netloc, url.path, urllib.parse.urlencode(query + [("ref", sha)]), "")
    )


def deploy(env):
    sha, database, hook = configuration(env)
    # No URI on the command line, no S3/application secrets in the migration container.
    try:
        subprocess.run(
            ["docker", "run", "--rm", "-e", "COMIC_MIGRATION_DATABASE_URL", "comic-runtime:ci", "migrate"],
            env={**os.environ, "COMIC_MIGRATION_DATABASE_URL": database},
            check=True, timeout=180,
        )
    except (OSError, subprocess.SubprocessError):
        raise DeploymentError("Schema migration failed. No Render deployment was requested.") from None
    request = urllib.request.Request(hook, data=b"", method="POST")
    # Do not retry: the provider may have accepted a request whose response was lost.
    try:
        with urllib.request.urlopen(request, timeout=30) as response:
            status = response.status
            if status == 200:
                body = json.loads(response.read(65536))
                deploy_id = body.get("deploy", {}).get("id")
                if not isinstance(deploy_id, str) or not re.fullmatch(r"dep-[A-Za-z0-9_-]+", deploy_id):
                    raise DeploymentError("Render accepted the request but returned no valid deploy ID. Check Render Events.")
                message = f"Render accepted deployment {deploy_id} for commit {sha}."
            elif status == 202:
                message = f"Render queued deployment for commit {sha}."
            else:
                raise DeploymentError("Unexpected Render response. Check Render Events before retrying.")
    except (OSError, urllib.error.URLError, ValueError, AttributeError):
        raise DeploymentError("Render request failed or its result is unknown. Check Render Events before retrying.") from None
    message += " Check Render Events for Live; acceptance is not proof that the new release is serving."
    print(message)
    summary = env.get("GITHUB_STEP_SUMMARY")
    if summary:
        with open(summary, "a", encoding="utf-8") as handle:
            handle.write(message + "\n")


if __name__ == "__main__":
    try:
        deploy(os.environ)
    except DeploymentError as error:
        print(str(error), file=sys.stderr)
        sys.exit(1)
