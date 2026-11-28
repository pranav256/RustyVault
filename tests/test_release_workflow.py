"""Local release-name regression; uses only Python's standard library and Bash."""

import os
from pathlib import Path
import re
import shutil
import subprocess
import textwrap
import unittest
import xml.etree.ElementTree as ET


ROOT = Path(__file__).resolve().parents[1]
BASH = shutil.which("bash") or "bash"
if os.name == "nt" and (git := shutil.which("git")):
    # Windows may resolve bash to WSL rather than the Git Bash used by Actions.
    git_bash = Path(git).resolve().parent.parent / "bin" / "bash.exe"
    if git_bash.is_file():
        BASH = str(git_bash)
WORKFLOW = (ROOT / ".github/workflows/release.yml").read_text(encoding="utf-8")
POWERSHELL = shutil.which("pwsh") or shutil.which("powershell")
# Read only this workflow's scalar matrix, env, step and run-block conventions.
MATRIX = [
    dict(re.findall(r"^\s*(?:- )?(\w+): (.+)$", row, re.MULTILINE))
    for row in WORKFLOW.split("        include:\n", 1)[1]
    .split("\n\n", 1)[0].split("\n          - ")
]
STEPS = dict(
    step.split("\n", 1)
    for step in re.split(r"^      - name: ", WORKFLOW, flags=re.MULTILINE)[1:]
)


def scalar(block, key, default=None):
    match = re.search(rf"^\s+{key}: (.+)$", block, re.MULTILINE)
    return match[1] if match else default


def environment(block):
    return dict(re.findall(r"^ +([A-Z_]+): (.+)$", block, re.MULTILINE))


def expand(value, context):
    return re.sub(r"\$\{\{\s*([^{}]+?)\s*\}\}", lambda m: context[m[1]], value)


def run_block(step):
    return textwrap.dedent(step.split("        run: |\n", 1)[1]).strip()


def bash(script, env):
    return subprocess.run(
        [BASH, "--noprofile", "--norc", "-euo", "pipefail", "-c", script],
        cwd=ROOT, env={**os.environ, **env}, capture_output=True, text=True,
        timeout=10,
    )


class ReleaseWorkflowTests(unittest.TestCase):
    def test_windows_signing_covers_every_executable_and_installer_before_packaging(self):
        files = {
            "rusty-vault.exe", "rustyvault-bridge.exe",
            "browser-extension/install-native-host.ps1", "browser-extension/install-brave.ps1",
        }
        ns = {"s": "http://signpath.io/artifact-configuration/v1"}
        config = ET.parse(ROOT / ".github/signpath-artifact-configuration.xml")
        targets = config.findall("s:zip-file/*", ns)
        self.assertEqual({target.attrib["path"] for target in targets}, files)
        self.assertEqual(len(targets), len(files))
        for target in targets:
            self.assertEqual(target.find("s:authenticode-sign", ns).attrib["hash-algorithm"], "sha256")
            if target.tag.endswith("}pe-file"):
                self.assertEqual(target.attrib["product-name"], "RustyVault")
                self.assertEqual(target.attrib["product-version"], "${version}")
        stages = [
            "Validate Windows signing configuration", "Build release binary",
            "Prepare Windows signing files", "Upload Windows signing files",
            "Sign Windows files", "Verify Windows signatures", "Package Windows Archive",
            "Publish Release Assets",
        ]
        order = [list(STEPS).index(stage) for stage in stages]
        self.assertEqual(order, sorted(order))
        for stage in stages:
            self.assertNotIn("continue-on-error", STEPS[stage])
        for stage in stages[2:-1]:
            self.assertEqual(scalar(STEPS[stage], "if"), "matrix.os == 'windows-latest'")
        for path in files:
            self.assertIn(path, STEPS["Upload Windows signing files"])
            self.assertIn(path, STEPS["Verify Windows signatures"])
            self.assertIn(f"cp signed-windows/{path}", STEPS["Package Windows Archive"])
        self.assertNotIn("cp target/", STEPS["Package Windows Archive"])
        self.assertEqual(scalar(STEPS["Sign Windows files"], "wait-for-completion"), "true")
        self.assertEqual(scalar(STEPS["Sign Windows files"], "output-artifact-directory"), "signed-windows")

    @unittest.skipUnless(POWERSHELL, "PowerShell is needed to exercise the Windows signature gate")
    def test_windows_signature_gate_rejects_unsigned_or_untimestamped_files(self):
        mock = """
$ErrorActionPreference = 'Stop'
function Get-AuthenticodeSignature {
    param([string]$LiteralPath)
    $status = 'Valid'
    $timestamp = $true
    if ($env:TEST_INVALID_FILE -and $LiteralPath.Replace('\\', '/').EndsWith($env:TEST_INVALID_FILE)) {
        $status = $env:TEST_SIGNATURE_STATUS
        $timestamp = $null
    }
    [pscustomobject]@{Status=$status; TimeStamperCertificate=$timestamp}
}
"""
        files = ("rusty-vault.exe", "rustyvault-bridge.exe",
                 "browser-extension/install-native-host.ps1", "browser-extension/install-brave.ps1")
        cases = [("", "Valid"), *((path, "NotSigned") for path in files), (files[-1], "Valid")]
        for path, status in cases:
            with self.subTest(path=path, status=status):
                result = subprocess.run(
                    [POWERSHELL, "-NoProfile", "-NonInteractive", "-Command",
                     mock + run_block(STEPS["Verify Windows signatures"])],
                    cwd=ROOT, env={**os.environ, "TEST_INVALID_FILE": path, "TEST_SIGNATURE_STATUS": status},
                    capture_output=True, text=True, timeout=15,
                )
                if path:
                    self.assertNotEqual(result.returncode, 0, result.stdout)
                    self.assertIn("valid timestamped Authenticode signature", result.stderr)
                else:
                    self.assertEqual(result.returncode, 0, result.stderr)
                    self.assertEqual(len(result.stdout.splitlines()), len(files))

    def test_release_stays_draft_until_every_platform_build_succeeds(self):
        publish_job = WORKFLOW.split("\n  publish:\n", 1)[1]
        self.assertEqual(scalar(publish_job, "needs"), "publish-release")
        self.assertEqual(scalar(STEPS["Publish Release Assets"], "draft"), "true")
        self.assertIn(
            'gh release edit "$RELEASE_TAG" --draft=false --latest',
            scalar(STEPS["Publish completed release"], "run"),
        )

    def test_versioned_names_match_packaging_and_upload_for_every_platform(self):
        expected = {
            "windows-latest": ("x86_64-pc-windows-msvc", "rusty-vault.exe", "Windows-x86_64.zip"),
            "ubuntu-latest": ("x86_64-unknown-linux-gnu", "rusty-vault", "Linux-x86_64.tar.gz"),
            "macos-latest": ("x86_64-apple-darwin", "rusty-vault", "macOS-x86_64.tar.gz"),
        }
        self.assertEqual({row["os"] for row in MATRIX}, set(expected))
        self.assertEqual(len(MATRIX), 3)
        job_env = environment(WORKFLOW.split("    steps:", 1)[0])
        for tag in ("v0.2.0", "v1.2.3-rc.1+build.7"):
            for row in MATRIX:
                with self.subTest(tag=tag, platform=row["os"]):
                    target, binary, suffix = expected[row["os"]]
                    self.assertEqual((row["target"], row["binary"]), (target, binary))
                    context = {"github.ref_name": tag, **{f"matrix.{k}": v for k, v in row.items()}}
                    env = {k: expand(v, context) for k, v in job_env.items()}
                    context.update({f"env.{k}": v for k, v in env.items()})
                    filename = f"RustyVault-{tag}-{suffix}"
                    upload = expand(scalar(STEPS["Publish Release Assets"], "files"), context)
                    self.assertEqual(upload, filename)
                    validation = STEPS["Validate release tag"]
                    result = bash(run_block(validation), {k: expand(v, context) for k, v in environment(validation).items()})
                    self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                    windows = row["os"] == "windows-latest"
                    packaging = run_block(STEPS["Package Windows Archive" if windows else "Package Unix Archive"])
                    self.assertIn('"$ARCHIVE_NAME"', packaging)
                    self.assertNotIn("github.ref", packaging)
                    # Stub only the existing copy/archiver commands; execute the actual shell arguments.
                    stubs = 'cp() { :; }; 7z() { printf "%s\\n" "$@"; }; tar() { printf "%s\\n" "$@"; };\n'
                    result = bash(stubs + expand(packaging, context), env)
                    self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                    contents = [binary] + (["rustyvault-bridge.exe"] if windows else [])
                    contents += ["README.md", "LICENSE", "DESIGN.md", "assets", "docs"]
                    contents += ["browser-extension"] if windows else []
                    self.assertEqual(result.stdout.splitlines(), ["a" if windows else "-czvf", upload, *contents])

    def test_unsafe_tags_are_rejected_before_build_or_packaging(self):
        self.assertEqual(next(iter(STEPS)), "Validate release tag")
        validation = STEPS["Validate release tag"]
        script = run_block(validation)
        self.assertNotIn("${{", script)
        for tag in ("", "1.2.3", "v1/other", "v1\\other", "v1 bad", "v1\nother", "v1*", "v1?", "v1[0]", "v1; exit 0", "v$(printf unsafe)", "v`printf unsafe`"):
            with self.subTest(tag=tag):
                env = {k: expand(v, {"github.ref_name": tag}) for k, v in environment(validation).items()}
                result = bash(script, env)
                self.assertNotEqual(result.returncode, 0, result.stdout)


if __name__ == "__main__":
    unittest.main()
