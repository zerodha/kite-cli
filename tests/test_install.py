"""Behavior checks for the release installer using isolated command fixtures."""

from __future__ import annotations

import hashlib
from pathlib import Path
import subprocess
import tarfile
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
INSTALLER = ROOT / "scripts/install.sh"
TAG = "v0.1.0"


class InstallTests(unittest.TestCase):
    def run_installer(
        self,
        *,
        version: str | None = None,
        bad_checksum: bool = False,
        system: str = "Linux",
        machine: str = "x86_64",
        managed_symlink: bool = False,
        use_default_destination: bool = False,
    ) -> tuple[subprocess.CompletedProcess[str], Path, tempfile.TemporaryDirectory]:
        class KeepTemp(tempfile.TemporaryDirectory):
            def __exit__(self, exc_type, exc_value, traceback):
                return False

        temporary = KeepTemp()
        with temporary as directory:
            root = Path(directory)
            commands = root / "commands"
            commands.mkdir()
            home = root / "home"
            home.mkdir()
            install_dir = root / "home" / ".local" / "bin"
            install_dir.mkdir(parents=True)
            fixture_releases = root / "releases"
            fixture_releases.mkdir()
            payload = root / "kite"
            payload.write_text("#!/bin/sh\nprintf 'kite fixture\\n'\n")
            payload.chmod(0o755)
            target_arch = {"arm64": "aarch64", "amd64": "x86_64"}.get(machine, machine)
            target = f"{target_arch}-{'apple-darwin' if system == 'Darwin' else 'unknown-linux-musl'}"
            archive_name = f"kite-{TAG.removeprefix('v')}-{target}.tar.gz"
            archive_path = fixture_releases / archive_name
            with tarfile.open(archive_path, "w:gz") as archive:
                archive.add(payload, arcname="kite")
            digest = hashlib.sha256(archive_path.read_bytes()).hexdigest()
            if bad_checksum:
                digest = "0" * 64
            (fixture_releases / "checksums.txt").write_text(f"{digest}  {archive_name}\n")

            curl = commands / "curl"
            curl.write_text(
                "#!/bin/sh\n"
                "out=; url=\n"
                "while [ $# -gt 0 ]; do case $1 in -o) out=$2; shift 2;; -w) shift 2;; -*) shift;; *) url=$1; shift;; esac; done\n"
                "case $url in\n"
                f"  */releases/latest) printf '%s' \"${{url%/latest}}/tag/{TAG}\" ;;\n"
                "  */releases/download/*/checksums.txt) cp \"$FIXTURE_RELEASES/checksums.txt\" \"$out\" ;;\n"
                f"  */releases/download/{TAG}/{archive_name}) cp \"$FIXTURE_RELEASES/{archive_name}\" \"$out\" ;;\n"
                "  *) exit 22 ;;\n"
                "esac\n"
            )
            curl.chmod(0o755)
            uname = commands / "uname"
            uname.write_text(
                "#!/bin/sh\ncase \"$1\" in -s) printf '%s\\n' \"$UNAME_SYSTEM\";; -m) printf '%s\\n' \"$UNAME_MACHINE\";; esac\n"
            )
            uname.chmod(0o755)
            shasum = commands / "shasum"
            shasum.write_text("#!/bin/sh\n[ \"$1\" = -a ] && shift 2\nexec sha256sum \"$@\"\n")
            shasum.chmod(0o755)
            if managed_symlink:
                (install_dir / "kite").symlink_to(root / "managed-kite")
            env = {
                "PATH": f"{commands}:{install_dir}:/usr/bin:/bin" if use_default_destination else f"{commands}:/usr/bin:/bin",
                "HOME": str(home),
                "TMPDIR": str(root),
                "FIXTURE_RELEASES": str(fixture_releases),
                "UNAME_SYSTEM": system,
                "UNAME_MACHINE": machine,
            }
            if not use_default_destination:
                env["KITE_INSTALL_DIR"] = str(install_dir)
            if version is not None:
                env["KITE_VERSION"] = version
            result = subprocess.run(
                ["sh", str(INSTALLER)], cwd=ROOT, env=env,
                capture_output=True, text=True, check=False,
            )
            return result, install_dir, temporary

    def test_installs_latest_release(self) -> None:
        result, install_dir, temporary = self.run_installer()
        self.addCleanup(temporary.cleanup)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn(f"Installed kite {TAG}", result.stdout)
        self.assertTrue((install_dir / "kite").is_file())

    def test_installs_pinned_version_with_or_without_v(self) -> None:
        for version in ("0.1.0", "v0.1.0"):
            with self.subTest(version=version):
                result, install_dir, temporary = self.run_installer(version=version)
                self.addCleanup(temporary.cleanup)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertIn(f"Installed kite {TAG}", result.stdout)

    def test_missing_pinned_version_fails(self) -> None:
        result, install_dir, temporary = self.run_installer(version="9.9.9")
        self.addCleanup(temporary.cleanup)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("could not download kite-9.9.9-", result.stderr)
        self.assertFalse((install_dir / "kite").exists())

    def test_uses_writable_personal_bin_on_path_by_default(self) -> None:
        result, install_dir, temporary = self.run_installer(use_default_destination=True)
        self.addCleanup(temporary.cleanup)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn(str(install_dir), result.stdout)
        self.assertTrue((install_dir / "kite").is_file())

    def test_rejects_checksum_mismatch_before_install(self) -> None:
        result, install_dir, temporary = self.run_installer(bad_checksum=True)
        self.addCleanup(temporary.cleanup)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("SHA-256 check failed", result.stderr)
        self.assertFalse((install_dir / "kite").exists())

    def test_macos_arm_uses_aarch64_target(self) -> None:
        result, install_dir, temporary = self.run_installer(system="Darwin", machine="arm64")
        self.addCleanup(temporary.cleanup)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertTrue((install_dir / "kite").is_file())

    def test_macos_intel_uses_x86_64_target(self) -> None:
        result, install_dir, temporary = self.run_installer(system="Darwin", machine="x86_64")
        self.addCleanup(temporary.cleanup)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertTrue((install_dir / "kite").is_file())

    def test_refuses_to_replace_managed_symlink(self) -> None:
        result, install_dir, temporary = self.run_installer(managed_symlink=True)
        self.addCleanup(temporary.cleanup)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("refusing to replace symlink", result.stderr)
        self.assertTrue((install_dir / "kite").is_symlink())

    def test_sh_installer_points_windows_to_powershell_installer(self) -> None:
        result, _, temporary = self.run_installer(system="Windows_NT")
        self.addCleanup(temporary.cleanup)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("scripts/install.ps1", result.stderr)


if __name__ == "__main__":
    unittest.main()
