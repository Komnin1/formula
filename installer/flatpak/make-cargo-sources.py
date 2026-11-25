#!/usr/bin/env python3
"""Write cargo-sources.json for the Flatpak build from Cargo.lock.

Flatpak builds have no internet access, so every Rust library (crate) Formula uses is listed
here with its download address and checksum. flatpak-builder downloads them beforehand and
Cargo then builds offline from that copy. Run it again whenever Cargo.lock changes:

    python3 installer/flatpak/make-cargo-sources.py
"""
import json
import pathlib
import tomllib

HERE = pathlib.Path(__file__).resolve().parent
ROOT = HERE.parent.parent
CRATES_IO = "registry+https://github.com/rust-lang/crates.io-index"


def main():
    lock = tomllib.loads((ROOT / "Cargo.lock").read_text())
    sources = []
    for pkg in lock["package"]:
        src = pkg.get("source")
        if src is None:
            continue  # Formula itself
        if src != CRATES_IO:
            raise SystemExit(f"{pkg['name']}: only crates.io libraries are handled, not {src}")
        name, version, sha = pkg["name"], pkg["version"], pkg["checksum"]
        dest = f"cargo/vendor/{name}-{version}"
        sources.append({
            "type": "archive",
            "archive-type": "tar-gzip",
            "url": f"https://static.crates.io/crates/{name}/{name}-{version}.crate",
            "sha256": sha,
            "dest": dest,
        })
        sources.append({
            "type": "inline",
            "contents": json.dumps({"package": sha, "files": {}}),
            "dest": dest,
            "dest-filename": ".cargo-checksum.json",
        })
    sources.append({
        "type": "inline",
        "contents": '[source.vendored-sources]\ndirectory = "cargo/vendor"\n\n'
                    '[source.crates-io]\nreplace-with = "vendored-sources"\n',
        "dest": "cargo",
        "dest-filename": "config.toml",
    })
    out = HERE / "cargo-sources.json"
    out.write_text(json.dumps(sources, indent=2) + "\n")
    print(f"{out}: {(len(sources) - 1) // 2} crates")


if __name__ == "__main__":
    main()
