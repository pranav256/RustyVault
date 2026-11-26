"""Local release-name regression; uses only Python's standard library and Bash."""

import os
from pathlib import Path
import re
import subprocess
import textwrap
import unittest


ROOT = Path(__file__).resolve().parents[1]
WORKFLOW = (ROOT / ".github/workflows/release.yml").read_text(encoding="utf-8")
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
        ["bash", "--noprofile", "--norc", "-euo", "pipefail", "-c", script],
        cwd=ROOT, env={**os.environ, **env}, capture_output=True, text=True,
        timeout=10,
    )


class ReleaseWorkflowTests(unittest.TestCase):
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
                    self.assertEqual(result.returncode, 0, result.stderr)
                    windows = row["os"] == "windows-latest"
                    packaging = run_block(STEPS["Package Windows Archive" if windows else "Package Unix Archive"])
                    self.assertIn('"$ARCHIVE_NAME"', packaging)
                    self.assertNotIn("github.ref", packaging)
                    # Stub only the existing copy/archiver commands; execute the actual shell arguments.
                    stubs = 'cp() { :; }; 7z() { printf "%s\\n" "$@"; }; tar() { printf "%s\\n" "$@"; };\n'
                    result = bash(stubs + expand(packaging, context), env)
                    self.assertEqual(result.returncode, 0, result.stderr)
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
