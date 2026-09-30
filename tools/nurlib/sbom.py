"""SPDX 2.3 SBOM generation from the package inventories recorded by the image finalize scripts
(nur.packages/v1) and the Cargo dependency graph.

License information for distribution packages is recorded as NOASSERTION: the authoritative
license texts ship inside the image (/usr/share/doc/*/copyright) but are not machine-parsed here.
This is recorded honestly rather than guessed (see docs/LICENSING_POLICY.md)."""

from __future__ import annotations

import hashlib
import json
import re
import subprocess
from pathlib import Path
from typing import Any

from .common import REPO, Base, NurError, read_json, write_json

SPDX_ID_RE = re.compile(r"[^A-Za-z0-9.-]")


def _spdx_id(prefix: str, name: str, version: str) -> str:
    return f"SPDXRef-{prefix}-{SPDX_ID_RE.sub('-', name)}-{SPDX_ID_RE.sub('-', version)}"


def _deb_packages(inventory: Path, base: Base, component: str) -> list[dict[str, Any]]:
    data = read_json(inventory)
    if data.get("schema") != "nur.packages/v1":
        raise NurError(f"{inventory} is not a nur.packages/v1 inventory")
    packages = []
    for pkg in data["packages"]:
        name, version, arch = pkg["name"], pkg["version"], pkg["architecture"]
        packages.append({
            "SPDXID": _spdx_id(f"{component}-deb", name, version),
            "name": name,
            "versionInfo": version,
            "supplier": "NOASSERTION",
            "downloadLocation": "NOASSERTION",
            "filesAnalyzed": False,
            "licenseConcluded": "NOASSERTION",
            "licenseDeclared": "NOASSERTION",
            "copyrightText": "NOASSERTION",
            "comment": f"Binary package from {base.distribution}:{base.release}, installed in the {component}.",
            "externalRefs": [{
                "referenceCategory": "PACKAGE-MANAGER",
                "referenceType": "purl",
                "referenceLocator": f"pkg:deb/{base.distribution}/{name}@{version}?arch={arch}&distro={base.release}",
            }],
        })
    return packages


def _cargo_packages() -> list[dict[str, Any]]:
    try:
        out = subprocess.run(["cargo", "metadata", "--format-version", "1", "--locked", "--offline"],
                             cwd=REPO, capture_output=True, text=True, check=True).stdout
    except (OSError, subprocess.CalledProcessError) as exc:
        raise NurError(f"cargo metadata failed: {exc}") from exc
    meta = json.loads(out)
    workspace = set(meta["workspace_members"])
    packages = []
    for pkg in meta["packages"]:
        ours = pkg["id"] in workspace
        packages.append({
            "SPDXID": _spdx_id("cargo", pkg["name"], pkg["version"]),
            "name": pkg["name"],
            "versionInfo": pkg["version"],
            "supplier": "Organization: ADM Tech Holding MChJ" if ours else "NOASSERTION",
            "downloadLocation": "NOASSERTION" if ours else f"https://crates.io/crates/{pkg['name']}/{pkg['version']}",
            "filesAnalyzed": False,
            "licenseConcluded": "NOASSERTION",
            "licenseDeclared": (pkg.get("license") or "NOASSERTION") if not ours else "NOASSERTION",
            "copyrightText": "NOASSERTION",
            "comment": "NUR-owned crate (license not yet decided, D3)." if ours else "Rust dependency of NUR components.",
            "externalRefs": [{"referenceCategory": "PACKAGE-MANAGER", "referenceType": "purl",
                              "referenceLocator": f"pkg:cargo/{pkg['name']}@{pkg['version']}"}],
        })
    return packages


def write_spdx(out: Path, version: str, base: Base, manifests: list[Path]) -> Path:
    for manifest in manifests:
        if not manifest.is_file():
            raise NurError(f"package inventory {manifest} missing; cannot produce SBOM")
    packages = _deb_packages(manifests[0], base, "image")
    packages += _deb_packages(manifests[1], base, "initrd")
    packages += _cargo_packages()
    root_id = "SPDXRef-nur-os-image"
    doc_hash = hashlib.sha256(f"{version}{base.name}".encode()).hexdigest()[:16]
    document = {
        "spdxVersion": "SPDX-2.3",
        "dataLicense": "CC0-1.0",
        "SPDXID": "SPDXRef-DOCUMENT",
        "name": f"NUR OS {version} (developer preview, phase 1)",
        "documentNamespace": f"https://nur-os.invalid/spdx/{version}/{doc_hash}",
        "creationInfo": {"creators": ["Tool: tools/nur build"], "created": "1970-01-01T00:00:00Z",
                         "comment": "Timestamp is fixed for reproducibility; see the build record for SOURCE_DATE_EPOCH."},
        "packages": [{
            "SPDXID": root_id, "name": "NUR OS image", "versionInfo": version, "supplier": "Organization: ADM Tech Holding MChJ",
            "downloadLocation": "NOASSERTION", "filesAnalyzed": False, "licenseConcluded": "NOASSERTION",
            "licenseDeclared": "NOASSERTION", "copyrightText": "NOASSERTION",
        }] + packages,
        "relationships": [{"spdxElementId": "SPDXRef-DOCUMENT", "relationshipType": "DESCRIBES", "relatedSpdxElement": root_id}]
        + [{"spdxElementId": root_id, "relationshipType": "CONTAINS", "relatedSpdxElement": p["SPDXID"]} for p in packages],
    }
    seen: set[str] = set()
    for pkg in document["packages"]:
        if pkg["SPDXID"] in seen:
            raise NurError(f"duplicate SPDX identifier {pkg['SPDXID']}")
        seen.add(pkg["SPDXID"])
    path = out / f"nur_{version}.spdx.json"
    write_json(path, document)
    return path
