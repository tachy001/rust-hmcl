"""Prepare five minimal release assets from verified native CI distributions.

Keeps Unix permissions, symlinks and macOS signatures intact; never extracts apps
onto the packaging host. Development reports and documentation stay in source/CI.
"""
import argparse
import hashlib
import json
from pathlib import Path, PurePosixPath
import shutil
import tarfile
import zipfile


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def prepare(source, proofs, output, revision):
    short = revision[:7]
    assert len(revision) == 40 and all(c in "0123456789abcdef" for c in revision)
    assets = {}
    for label, host in (
        ("windows-x64", "win32-x64"),
        ("macos-arm64", "darwin-arm64"),
        ("macos-x64", "darwin-x64"),
        ("linux-x64", "linux-x64"),
    ):
        proof_root = proofs / f"platform-proof-{label}"
        proof = json.loads((proof_root / "platform-ui-proof" / host / "proof.json").read_text())
        java = json.loads((proof_root / "java-online-proof.json").read_text())
        assert proof["sourceRevision"] == revision and len(proof["passed"]) == 9
        assert proof["native"]["visible"] is False and java["runtime"]["compatible"] is True
        stem = f"aster-launcher-{label}-{short}"
        archive = source / (stem + (".zip" if label == "windows-x64" else ".tar.gz"))
        checksum = (source / f"SHA256SUMS-{label}.txt").read_text().splitlines()
        for line in checksum:
            expected, name = line.split()
            assert Path(name).name == name
            assert sha256((source / name).read_bytes()) == expected
        if label == "windows-x64":
            with zipfile.ZipFile(archive) as bundle:
                manifest = json.loads(bundle.read("build-info.json"))
                binary = bundle.read("AsterLauncher.exe")
            target = output / "Aster-Windows-x64.exe"
            target.write_bytes(binary)
        else:
            mac = label.startswith("macos")
            selected = "Aster Launcher.app/" if mac else "AsterLauncher"
            executable = "Aster Launcher.app/Contents/MacOS/rust-hmcl-desktop" if mac else selected
            friendly = {"macos-arm64": "macOS-AppleSilicon", "macos-x64": "macOS-Intel", "linux-x64": "Linux-x64"}[label]
            target = output / f"Aster-{friendly}.tar.gz"
            with tarfile.open(archive, "r:gz") as bundle:
                members = {item.name.removeprefix("./"): item for item in bundle.getmembers()}
                manifest = json.load(bundle.extractfile(members["build-info.json"]))
                binary = bundle.extractfile(members[executable]).read()
                with tarfile.open(target, "w:gz") as minimal:
                    for name, item in members.items():
                        if (mac and (name == selected.rstrip("/") or name.startswith(selected))) or (not mac and name == selected):
                            assert not PurePosixPath(name).is_absolute() and ".." not in PurePosixPath(name).parts
                            minimal.addfile(item, bundle.extractfile(item) if item.isfile() else None)
            with tarfile.open(target, "r:gz") as check:
                names = {item.name.removeprefix("./"): item for item in check.getmembers()}
                assert executable in names and names[executable].mode & 0o111
                assert sha256(check.extractfile(names[executable]).read()) == proof["executableSha256"]
            if not mac:
                deb = output / "Aster-Linux-x64.deb"
                shutil.copyfile(source / (stem + ".deb"), deb)
                assets[deb.name] = sha256(deb.read_bytes())
        assert manifest["source_revision"] == revision and manifest["source_dirty"] is False
        assert sha256(binary) == proof["executableSha256"]
        assets[target.name] = sha256(target.read_bytes())
    assert len(assets) == 5
    assert {p.name for p in output.iterdir()} == set(assets), "Release directory must contain only the five assets"
    print(json.dumps(assets, indent=2))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, required=True)
    parser.add_argument("--proofs", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--revision", required=True)
    args = parser.parse_args()
    assert args.output.resolve() != args.source.resolve()
    args.output.mkdir(parents=True, exist_ok=True)
    prepare(args.source, args.proofs, args.output, args.revision)
