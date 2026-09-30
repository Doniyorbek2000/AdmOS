"""Command-line interface of `tools/nur`, the canonical NUR developer entry point.

Exit codes: 0 success/PASS; 1 failure/FAIL; 2 usage error; 4 environment limitation (the
environment cannot perform the operation — this is never reported as success).
"""

from __future__ import annotations

import argparse
import json
import logging
import sys

from . import build as build_mod
from . import bundle as bundle_mod
from . import checks, keys as keys_mod, scenarios
from .common import BASES, EnvironmentLimitation, NurError, setup_logging

log = logging.getLogger("nur")


def _parser() -> argparse.ArgumentParser:
    p = argparse.ArgumentParser(prog="nur", description="NUR OS developer tooling (Phase 1).")
    p.add_argument("-v", "--verbose", action="store_true")
    sub = p.add_subparsers(dest="command", required=True)

    k = sub.add_parser("keys", help="development signing keys")
    ksub = k.add_subparsers(dest="keys_command", required=True)
    ksub.add_parser("init-dev", help="create missing DEVELOPMENT keys (never overwrites)")
    ksub.add_parser("verify", help="check development keys")
    ksub.add_parser("production", help="(refuses) production keys are not handled by this tool")

    b = sub.add_parser("build", help="build a signed NUR OS image")
    b.add_argument("--version", required=True, help="build version YYYY.MM.DD.N")
    b.add_argument("--base", default="debian", choices=sorted(BASES))
    b.add_argument("--fault", default="none", choices=build_mod.FAULTS,
                   help="deliberate fault for negative tests (recorded in metadata and bundle)")
    b.add_argument("--security-floor", default=None)
    b.add_argument("--force", action="store_true", help="rebuild an existing version")
    b.add_argument("--debug-console", action="store_true", help="forward the journal to the serial console")

    bd = sub.add_parser("bundle", help="create a signed update bundle for a built version")
    bd.add_argument("--version", required=True)
    bd.add_argument("--force", action="store_true")

    v = sub.add_parser("verify", help="fast static checks (repository, Rust, image configuration)")
    v.add_argument("--skip-rust", action="store_true")

    t = sub.add_parser("test", help="run a QEMU demonstration scenario against built images")
    tsub = t.add_subparsers(dest="scenario", required=True)
    for name in ("boot", "secureboot", "integrity", "tpm"):
        s = tsub.add_parser(name)
        s.add_argument("--version", required=True)
    u = tsub.add_parser("update")
    u.add_argument("--from", dest="from_version", required=True)
    u.add_argument("--to", dest="to_version", required=True)
    rb = tsub.add_parser("rollback")
    rb.add_argument("--from", dest="from_version", required=True)
    rb.add_argument("--broken", dest="broken_version", required=True)
    return p


def main(argv: list[str] | None = None) -> int:
    args = _parser().parse_args(argv)
    setup_logging(args.verbose)
    try:
        return _dispatch(args)
    except EnvironmentLimitation as exc:
        log.error("ENVIRONMENT LIMITATION: %s", exc)
        return exc.exit_code
    except NurError as exc:
        log.error("%s", exc)
        return exc.exit_code
    except KeyboardInterrupt:
        log.error("interrupted")
        return 130


def _dispatch(args: argparse.Namespace) -> int:
    if args.command == "keys":
        if args.keys_command == "production":
            keys_mod.refuse_production()
        if args.keys_command == "init-dev":
            build_mod.cargo_build(build_mod.BUILD / "logs" / "cargo-keys.log")
            keys_mod.init_dev_keys(build_mod.nur_bundle_binary())
        keys_mod.verify_dev_keys(keys_mod.dev_keys())
        print(f"development keys OK in {keys_mod.DEV_KEY_DIR} (NOT FOR PRODUCTION)")
        return 0
    if args.command == "build":
        record = build_mod.build(args.version, args.base, args.fault, args.security_floor, args.force, args.debug_console)
        print(json.dumps({k: record[k] for k in ("version", "usrhash", "fault_injection", "base", "timings")}, indent=2))
        return 0
    if args.command == "bundle":
        print(bundle_mod.create_bundle(args.version, force=args.force))
        return 0
    if args.command == "verify":
        return checks.run_all(skip_rust=args.skip_rust)
    if args.command == "test":
        if args.scenario == "update":
            result = scenarios.scenario_update(args.from_version, args.to_version)
        elif args.scenario == "rollback":
            result = scenarios.scenario_rollback(args.from_version, args.broken_version)
        else:
            result = getattr(scenarios, f"scenario_{args.scenario}")(args.version)
        print(json.dumps({"scenario": result["scenario"], "verdict": result["verdict"],
                          "error": result.get("error"), "report": result["workdir"] + "/report.json"}, indent=2))
        return 0 if result["verdict"] == "PASS" else 1
    raise NurError(f"unhandled command {args.command}", exit_code=2)


if __name__ == "__main__":
    sys.exit(main())
