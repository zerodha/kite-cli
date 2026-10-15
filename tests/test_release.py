"""Behavior checks for release metadata and build gates."""

from __future__ import annotations

import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import tomllib
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location("release", ROOT / "scripts/release.py")
assert SPEC is not None and SPEC.loader is not None
release = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(release)


class ReleaseTests(unittest.TestCase):
    def test_rlsr_build_targets_match_release_build_gates(self) -> None:
        config = tomllib.loads((ROOT / "rlsr.toml").read_text())
        builds = config["releases"][0]["builds"]
        self.assertEqual({build["target"] for build in builds}, set(release.TARGETS))
        for build in builds:
            windows = build["target"] == release.WINDOWS_TARGET
            self.assertEqual(build["bin_name"], "kite.exe" if windows else "kite")
            self.assertTrue(build["artifact"].endswith("/release/kite.exe" if windows else "/release/kite"))
        windows = next(build for build in builds if build["target"] == release.WINDOWS_TARGET)
        self.assertEqual(windows["archive_format"], "zip")
        self.assertEqual(windows["type"], "rust")
        self.assertEqual(release.binary_path(release.WINDOWS_TARGET).name, "kite.exe")

    def test_windows_archive_verification_rejects_other_targets(self) -> None:
        with self.assertRaisesRegex(ValueError, "Windows MSVC target"):
            release.verify_windows_archive(release.TARGETS[0])

    def test_binary_verification_requires_all_compiled_settings(self) -> None:
        with patch.dict(os.environ, {}, clear=True):
            with self.assertRaisesRegex(ValueError, "missing release settings"):
                release.verify_built(release.WINDOWS_TARGET)

    def test_release_workflow_uses_arm_runner_for_intel_cross_build(self) -> None:
        workflow = (ROOT / ".github/workflows/release.yml").read_text()
        self.assertIn("runner: macos-15", workflow)
        self.assertNotIn("runner: macos-15-intel", workflow)
        publish_guard = "if: github.event_name == 'push' && startsWith(github.ref, 'refs/tags/v')"
        self.assertIn(publish_guard, workflow)
        self.assertIn("workflow_dispatch:", workflow)

    def test_tag_release_verifies_windows_install_before_publish(self) -> None:
        workflow = (ROOT / ".github/workflows/release.yml").read_text()
        install_job = workflow.split("  verify-windows-install:", 1)[1].split("\n  publish:", 1)[0]
        publish_job = workflow.split("  publish:", 1)[1]
        self.assertNotIn("if: github.event_name == 'workflow_dispatch'", install_job)
        self.assertIn("name: kite-x86_64-pc-windows-msvc", install_job)
        self.assertIn("Cargo.toml", install_job)
        self.assertNotIn("0.1.0-alpha.1", install_job)
        self.assertIn("needs: [build, verify-windows-install]", publish_job)

    def test_preview_tag_is_local_and_uses_package_version(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "Cargo.toml").write_text('[package]\nversion = "0.1.0"\n')
            subprocess.run(["git", "init", "-q"], cwd=root, check=True)
            (root / "file").write_text("preview\n")
            subprocess.run(["git", "add", "file"], cwd=root, check=True)
            subprocess.run(
                [
                    "git", "-c", "user.name=Test", "-c",
                    "user.email=test@example.invalid", "commit", "-qm", "init",
                ],
                cwd=root,
                check=True,
            )
            original_root = release.ROOT
            try:
                release.ROOT = root
                release.preview_tag()
                tags = subprocess.run(
                    ["git", "tag", "--list"], cwd=root, check=True,
                    capture_output=True, text=True,
                ).stdout.splitlines()
                self.assertEqual(tags, ["v0.1.0-preview"])
                with self.assertRaisesRegex(ValueError, "already exists"):
                    release.preview_tag()
            finally:
                release.ROOT = original_root

    def test_release_notes_reject_wrong_tag(self) -> None:
        with self.assertRaisesRegex(ValueError, "match the Cargo.toml"):
            release.release_notes("v9.9.9")

    def test_release_notes_reject_undated_changelog(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "Cargo.toml").write_text('[package]\nversion = "0.1.0"\n')
            (root / "CHANGELOG.md").write_text(
                "## [0.1.0] - YYYY-MM-DD\n\nRelease text.\n"
            )
            original_root = release.ROOT
            try:
                release.ROOT = root
                with self.assertRaisesRegex(ValueError, "missing dated changelog"):
                    release.release_notes("v0.1.0")
            finally:
                release.ROOT = original_root

    def test_release_notes_rejects_impossible_calendar_date(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "Cargo.toml").write_text('[package]\nversion = "0.1.0"\n')
            (root / "CHANGELOG.md").write_text(
                "## [0.1.0] - 2026-02-30\n\nRelease text.\n"
            )
            original_root = release.ROOT
            try:
                release.ROOT = root
                with self.assertRaisesRegex(ValueError, "valid YYYY-MM-DD"):
                    release.release_notes("v0.1.0")
            finally:
                release.ROOT = original_root

    def test_release_notes_selects_only_the_tagged_section(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "Cargo.toml").write_text('[package]\nversion = "0.1.0"\n')
            (root / "CHANGELOG.md").write_text(
                "## [Unreleased]\n\nDraft text.\n\n"
                "## [0.1.0] - 2026-10-15\n\nRelease text.\n\n"
                "## [0.0.9] - 2026-01-01\n\nOld text.\n"
            )
            original_root = release.ROOT
            try:
                release.ROOT = root
                notes = release.release_notes("v0.1.0")
            finally:
                release.ROOT = original_root
        self.assertEqual(notes, "Release text.\n")

    def test_build_fails_before_cargo_when_settings_are_missing(self) -> None:
        env = {key: value for key, value in os.environ.items() if key not in release.SETTINGS}
        result = subprocess.run(
            [sys.executable, str(ROOT / "scripts/release.py"), "build", release.TARGETS[0]],
            cwd=ROOT,
            env=env,
            capture_output=True,
            text=True,
            check=False,
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("missing release settings", result.stderr)
        self.assertNotIn("cargo", result.stderr.lower())

    def test_publish_config_uses_repository_and_draft_without_credentials(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "release.json"
            release.publish_config("example/kite", output)
            serialized = output.read_text()
            config = json.loads(serialized)
        target = config["releases"][0]["targets"]["github"]
        self.assertEqual(target, {"owner": "example", "repo": "kite", "draft": True})
        self.assertNotIn("GITHUB_TOKEN", serialized)

    def test_publish_config_rejects_invalid_repository(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "release.json"
            with self.assertRaisesRegex(ValueError, "OWNER/REPO"):
                release.publish_config("https://example/kite", output)
            self.assertFalse(output.exists())

    def test_split_manifest_paths_work_on_the_linux_publisher(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            dist = root / "dist"
            dist.mkdir()
            manifest = dist / "rlsr-split-kite-windows.yml"
            manifest.write_text(
                "release: kite\narchives:\n"
                "- ./dist\\kite-windows.zip\n"
                "- ./dist/kite-linux.tar.gz\nimage_tags: []\n"
            )
            original_root = release.ROOT
            try:
                release.ROOT = root
                release.normalize_split_manifests()
                release.normalize_split_manifests()
            finally:
                release.ROOT = original_root
            self.assertEqual(
                manifest.read_text(),
                "release: kite\narchives:\n"
                "- ./dist/kite-windows.zip\n"
                "- ./dist/kite-linux.tar.gz\nimage_tags: []\n",
            )


if __name__ == "__main__":
    unittest.main()
