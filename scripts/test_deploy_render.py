"""Deployment barriers and provider outcomes, without live credentials or services."""
import importlib.util
import io
import json
from pathlib import Path
import subprocess
import unittest
from unittest.mock import patch
import urllib.error

spec = importlib.util.spec_from_file_location("deploy_render", Path(__file__).with_name("deploy-render.py"))
deployment = importlib.util.module_from_spec(spec)
spec.loader.exec_module(deployment)


class DeploymentTests(unittest.TestCase):
    def setUp(self):
        self.env = {
            "GITHUB_REF": "refs/heads/main", "GITHUB_EVENT_NAME": "push", "GITHUB_SHA": "a" * 40,
            "COMIC_MIGRATION_DATABASE_URL": "postgres://owner:private-password@db.example/project?sslmode=verify-full",
            "RENDER_DEPLOY_HOOK_URL": "https://api.render.com/deploy/srv-example?key=private-hook",
        }
        self.run = patch.object(deployment.subprocess, "run").start()
        self.open = patch.object(deployment.urllib.request, "urlopen").start()
        self.out = patch("sys.stdout", new_callable=io.StringIO).start()
        self.addCleanup(patch.stopall)
        response = self.open.return_value.__enter__.return_value
        response.status = 200
        response.read.return_value = json.dumps({"deploy": {"id": "dep-example"}}).encode()

    def test_migration_precedes_exact_commit_request(self):
        events = []
        self.run.side_effect = lambda *a, **k: events.append("migration")
        self.open.side_effect = lambda *a, **k: (events.append("request") or self.open.return_value)
        deployment.deploy(self.env)
        self.assertEqual(events, ["migration", "request"])
        args, kwargs = self.run.call_args
        self.assertEqual(args[0][-2:], ["comic-runtime:ci", "migrate"])
        self.assertNotIn(self.env["COMIC_MIGRATION_DATABASE_URL"], args[0])
        self.assertEqual(kwargs["env"]["COMIC_MIGRATION_DATABASE_URL"], self.env["COMIC_MIGRATION_DATABASE_URL"])
        request = self.open.call_args.args[0]
        self.assertEqual(request.method, "POST")
        self.assertIn("ref=" + "a" * 40, request.full_url)
        self.assertNotIn("private", self.out.getvalue())
        self.assertIn("not proof", self.out.getvalue())

    def test_migration_failure_never_triggers_render(self):
        self.run.side_effect = subprocess.CalledProcessError(1, "docker")
        with self.assertRaisesRegex(deployment.DeploymentError, "No Render deployment"):
            deployment.deploy(self.env)
        self.open.assert_not_called()

    def test_unsafe_or_missing_configuration_has_no_side_effects(self):
        for change in [
            {"GITHUB_REF": "refs/pull/3/merge"}, {"GITHUB_EVENT_NAME": "pull_request"},
            {"GITHUB_SHA": "main"}, {"COMIC_MIGRATION_DATABASE_URL": ""},
            {"COMIC_MIGRATION_DATABASE_URL": "postgres://user:secret@ep-pooler.example/db"},
            {"RENDER_DEPLOY_HOOK_URL": "https://other.example/deploy/srv-example?key=secret"},
            {"RENDER_DEPLOY_HOOK_URL": self.env["RENDER_DEPLOY_HOOK_URL"] + "&ref=other"},
        ]:
            with self.subTest(change=list(change)):
                with self.assertRaises(deployment.DeploymentError):
                    deployment.deploy({**self.env, **change})
                self.run.assert_not_called()
                self.open.assert_not_called()

    def test_queued_response_is_reported_without_live_claim(self):
        self.open.return_value.__enter__.return_value.status = 202
        deployment.deploy(self.env)
        self.assertIn("queued", self.out.getvalue())
        self.assertIn("not proof", self.out.getvalue())

    def test_ambiguous_or_invalid_response_is_not_retried_or_leaked(self):
        for outcome in [urllib.error.URLError("private-hook"), ValueError("private-password")]:
            self.open.side_effect = outcome
            self.open.reset_mock()
            with self.assertRaises(deployment.DeploymentError) as caught:
                deployment.deploy(self.env)
            self.assertNotIn("private", str(caught.exception))
            self.assertEqual(self.open.call_count, 1)
        self.open.side_effect = None
        self.open.return_value.__enter__.return_value.read.return_value = b'{}'
        with self.assertRaisesRegex(deployment.DeploymentError, "no valid deploy ID"):
            deployment.deploy(self.env)


if __name__ == "__main__":
    unittest.main()
