//! `nurctl` — NUR OS system control (Phase 1).
//!
//! ```text
//! nurctl status [--json]              slot, boot assessment and entry state
//! nurctl health-check                 run boot health checks (used by nur-boot-health.service)
//! nurctl boot-failed                  handle a failed health check (OnFailure= handler)
//! nurctl update verify <bundle-dir>   verify signature, manifest and policy only
//! nurctl update install <bundle-dir>  verify, stage and install into the inactive slot
//! nurctl metadata                     print validated build metadata
//! ```
//!
//! Exit codes: 0 success; 1 check failed / update refused / operation failed; 2 usage error;
//! 3 environment error (I/O, missing system facilities).
//!
//! Machine-readable results are printed as single lines prefixed with `NUR-HEALTH `,
//! `NUR-UPDATE ` or `NUR-BOOT ` so that serial-console test harnesses can parse them.

use std::fs;
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::Path;
use std::process::{Command, ExitCode};
use std::time::Instant;

use nur_boot::status::HEALTH_REPORT_PATH;
use nur_boot::{collect_status, run_health_checks, HostCommandRunner, System};
use nur_meta::{BuildMetadata, BUILD_METADATA_PATH};
use nur_update::keys::TrustStore;
use nur_update::{bundle, install, policy, stage, UpdateError};
use serde::Serialize;

const TRUST_STORE: &str = "/usr/lib/nur/keys/update";
const UPDATE_RESULT_PATH: &str = "/var/lib/nur/update/last.json";
const SYSTEMCTL: &str = "/usr/bin/systemctl";

const EXIT_FAILED: u8 = 1;
const EXIT_USAGE: u8 = 2;
const EXIT_ENVIRONMENT: u8 = 3;

const USAGE: &str = "usage:
  nurctl status [--json]
  nurctl health-check
  nurctl boot-failed
  nurctl update verify <bundle-dir>
  nurctl update install <bundle-dir>
  nurctl metadata
  nurctl --version";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let code = match args.as_slice() {
        ["status"] => status(false),
        ["status", "--json"] => status(true),
        ["health-check"] => health_check(),
        ["boot-failed"] => boot_failed(),
        ["update", "verify", dir] => update(Path::new(dir), false),
        ["update", "install", dir] => update(Path::new(dir), true),
        ["metadata"] => metadata(),
        ["--version"] => {
            println!("nurctl {}", env!("CARGO_PKG_VERSION"));
            0
        }
        ["--help" | "-h" | "help"] => {
            println!("{USAGE}");
            0
        }
        _ => {
            eprintln!("{USAGE}");
            EXIT_USAGE
        }
    };
    ExitCode::from(code)
}

fn to_json_line<T: Serialize>(value: &T) -> String {
    serde_json::to_string(value)
        .unwrap_or_else(|e| format!("{{\"serialization_error\":{:?}}}", e.to_string()))
}

/// Writes `content` atomically (temporary file + rename) with mode 0644.
fn write_atomic(path: &Path, content: &str) -> std::io::Result<()> {
    let parent = path.parent().unwrap_or(Path::new("/"));
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o755)
        .create(parent)?;
    let tmp = parent.join(format!(
        ".{}.tmp",
        path.file_name().and_then(|n| n.to_str()).unwrap_or("out")
    ));
    let _ = fs::remove_file(&tmp);
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o644)
        .open(&tmp)?;
    file.write_all(content.as_bytes())?;
    file.sync_all()?;
    fs::rename(&tmp, path)
}

fn status(json: bool) -> u8 {
    let status = collect_status(&System::default(), &HostCommandRunner);
    if json {
        match serde_json::to_string_pretty(&status) {
            Ok(text) => println!("{text}"),
            Err(e) => {
                eprintln!("nurctl: cannot serialize status: {e}");
                return EXIT_ENVIRONMENT;
            }
        }
        return 0;
    }
    println!(
        "NUR OS version : {}",
        status
            .version
            .as_deref()
            .unwrap_or("UNKNOWN (metadata unreadable)")
    );
    if let Some(err) = &status.metadata_error {
        println!("metadata error : {err}");
    }
    println!(
        "booted slot    : {}",
        status.booted_slot.as_deref().unwrap_or("unknown")
    );
    println!(
        "usrhash        : {}",
        status.usrhash.as_deref().unwrap_or("none")
    );
    println!(
        "secure boot    : {}",
        match status.secure_boot {
            Some(true) => "enabled",
            Some(false) => "disabled",
            None => "unknown",
        }
    );
    println!(
        "assessment     : {}",
        if status.assessment.under_assessment {
            "UNDER ASSESSMENT (not yet blessed)"
        } else {
            "blessed / not counted"
        }
    );
    for slot in &status.slots {
        println!(
            "slot {:<2} {:<12} {:<28} {}",
            slot.name,
            slot.device,
            slot.label,
            if slot.booted { "(booted)" } else { "" }
        );
    }
    for entry in &status.entries {
        println!("entry {:<34} {}", entry.file, entry.state);
    }
    for warning in &status.warnings {
        println!("warning: {warning}");
    }
    0
}

fn health_check() -> u8 {
    let report = run_health_checks(&System::default(), &HostCommandRunner);
    let line = to_json_line(&report);
    println!("NUR-HEALTH {line}");
    for check in &report.checks {
        println!(
            "  [{}] {}: {}",
            if check.passed { "PASS" } else { "FAIL" },
            check.name,
            check.detail
        );
    }
    if let Err(e) = write_atomic(Path::new(HEALTH_REPORT_PATH), &format!("{line}\n")) {
        eprintln!("nurctl: warning: cannot store health report: {e}");
    }
    if report.passed {
        0
    } else {
        EXIT_FAILED
    }
}

#[derive(Serialize)]
struct BootFailedReport<'a> {
    event: &'a str,
    under_assessment: bool,
    entry: Option<String>,
    action: &'a str,
}

/// Invoked when nur-boot-health.service fails. While the booted entry is under assessment, the
/// system reboots so that systemd-boot counts the failed attempt; once all attempts are used,
/// the entry is marked bad and the previous version boots. A blessed entry is never rebooted in
/// a loop: the failure is reported and the system stays up for diagnosis.
fn boot_failed() -> u8 {
    let status = collect_status(&System::default(), &HostCommandRunner);
    let under = status.assessment.under_assessment;
    let report = BootFailedReport {
        event: "health-check-failed",
        under_assessment: under,
        entry: status
            .assessment
            .boot_count_path
            .clone()
            .or(status.assessment.entry_selected.clone()),
        action: if under {
            "reboot-to-consume-boot-attempt"
        } else {
            "none-entry-already-blessed"
        },
    };
    println!("NUR-BOOT {}", to_json_line(&report));
    if !under {
        // Handled as designed: a blessed entry is not rebooted; the failed health unit and the
        // stored report remain visible for diagnosis.
        return 0;
    }
    match Command::new(SYSTEMCTL).arg("reboot").env_clear().status() {
        Ok(s) if s.success() => 0,
        Ok(s) => {
            eprintln!("nurctl: systemctl reboot failed: {s}");
            EXIT_ENVIRONMENT
        }
        Err(e) => {
            eprintln!("nurctl: cannot execute systemctl: {e}");
            EXIT_ENVIRONMENT
        }
    }
}

fn metadata() -> u8 {
    match fs::read_to_string(BUILD_METADATA_PATH) {
        Ok(text) => match BuildMetadata::from_json(&text) {
            Ok(m) => match serde_json::to_string_pretty(&m) {
                Ok(t) => {
                    println!("{t}");
                    0
                }
                Err(e) => {
                    eprintln!("nurctl: {e}");
                    EXIT_ENVIRONMENT
                }
            },
            Err(e) => {
                eprintln!("nurctl: {e}");
                EXIT_FAILED
            }
        },
        Err(e) => {
            eprintln!("nurctl: reading {BUILD_METADATA_PATH}: {e}");
            EXIT_ENVIRONMENT
        }
    }
}

#[derive(Serialize)]
struct UpdateReport {
    action: &'static str,
    result: &'static str,
    from_version: Option<String>,
    to_version: Option<String>,
    fault_injection: Option<String>,
    duration_ms: u128,
    error: Option<String>,
}

fn exit_code_for(error: &UpdateError) -> u8 {
    match error {
        UpdateError::Io { .. } => EXIT_ENVIRONMENT,
        _ => EXIT_FAILED,
    }
}

fn update(bundle_dir: &Path, install_it: bool) -> u8 {
    let started = Instant::now();
    let mut report = UpdateReport {
        action: if install_it { "install" } else { "verify" },
        result: "failed",
        from_version: None,
        to_version: None,
        fault_injection: None,
        duration_ms: 0,
        error: None,
    };
    let outcome = run_update(bundle_dir, install_it, &mut report);
    report.duration_ms = started.elapsed().as_millis();
    let code = match &outcome {
        Ok(()) => {
            report.result = if install_it { "installed" } else { "verified" };
            0
        }
        Err(e) => {
            report.error = Some(e.to_string());
            exit_code_for(e)
        }
    };
    let line = to_json_line(&report);
    println!("NUR-UPDATE {line}");
    if install_it {
        if let Err(e) = write_atomic(Path::new(UPDATE_RESULT_PATH), &format!("{line}\n")) {
            eprintln!("nurctl: warning: cannot store update result: {e}");
        }
    }
    code
}

fn run_update(
    bundle_dir: &Path,
    install_it: bool,
    report: &mut UpdateReport,
) -> Result<(), UpdateError> {
    let current_text = fs::read_to_string(BUILD_METADATA_PATH).map_err(|e| UpdateError::Io {
        context: format!("reading {BUILD_METADATA_PATH}"),
        source: e,
    })?;
    let current = BuildMetadata::from_json(&current_text)
        .map_err(|e| UpdateError::Invalid(format!("running system metadata: {e}")))?;
    report.from_version = Some(current.version.to_string());

    let trust = TrustStore::load_dir(Path::new(TRUST_STORE))?;
    let verified = bundle::verify_bundle(bundle_dir, &trust)?;
    report.to_version = Some(verified.manifest.version.to_string());
    report.fault_injection = Some(verified.manifest.fault_injection.clone());
    println!(
        "nurctl: manifest signature OK (key {}, {:?}); version {}",
        verified.manifest.signing.key_id,
        verified.manifest.signing.environment,
        verified.manifest.version
    );
    policy::check_policy(&verified.manifest, &current)?;
    println!(
        "nurctl: policy OK ({} -> {}, floor {})",
        current.version, verified.manifest.version, current.security_floor
    );
    if !install_it {
        return Ok(());
    }

    let staged = stage::stage_artifacts(
        bundle_dir,
        &verified.manifest,
        Path::new(stage::STAGING_DIR),
    )?;
    println!(
        "nurctl: {} artifacts staged and verified in {}",
        staged.files.len(),
        staged.dir.display()
    );
    let result = install::run_sysupdate(verified.manifest.version);
    // Staged copies are no longer needed whether or not installation succeeded.
    if let Err(e) = fs::remove_dir_all(&staged.dir) {
        eprintln!("nurctl: warning: cannot remove staging directory: {e}");
    }
    let output = result?;
    for line in output.log.lines() {
        println!("sysupdate: {line}");
    }
    Ok(())
}
