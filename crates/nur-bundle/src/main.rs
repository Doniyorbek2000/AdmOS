//! `nur-bundle` — build-host tool for development signing keys and signed update bundles.
//!
//! ```text
//! nur-bundle keygen --out <secret.json> --pub <public.json> --label <text>
//! nur-bundle create --key <secret.json> --metadata <build.json> --usrhash <hex>
//!                   --usr <file> --usr-verity <file> --uki <file>
//!                   --fault <none|name> --out <bundle-dir>
//! nur-bundle verify --trust <public.json> --bundle <bundle-dir>
//! ```
//!
//! Only **development** keys can be generated. Production signing happens in the HSM-backed
//! release pipeline (docs/RELEASE_STRATEGY.md) and is not implemented in Phase 1.

use std::collections::HashMap;
use std::fs;
use std::os::unix::fs::DirBuilderExt;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use nur_meta::BuildMetadata;
use nur_update::bundle::{signature_file_content, verify_bundle, MANIFEST_FILE, SIGNATURE_FILE};
use nur_update::keys::{KeyEnvironment, KeyFile, TrustStore};
use nur_update::manifest::{
    expected_file_name, Artifact, Manifest, Role, SigningInfo, MANIFEST_SCHEMA,
};
use nur_update::{hex_encode, is_lower_hex, UpdateError};

const USAGE: &str = "usage:
  nur-bundle keygen --out <secret.json> --pub <public.json> --label <text>
  nur-bundle create --key <secret.json> --metadata <build.json> --usrhash <hex> --usr <file>
                    --usr-verity <file> --uki <file> --fault <none|name> --out <bundle-dir>
  nur-bundle verify --trust <public.json> --bundle <bundle-dir>";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.split_first() {
        Some((cmd, rest)) => parse_flags(rest).and_then(|flags| match cmd.as_str() {
            "keygen" => keygen(&flags),
            "create" => create(&flags),
            "verify" => verify(&flags),
            _ => Err(Failure::Usage(format!("unknown command {cmd:?}"))),
        }),
        None => Err(Failure::Usage("missing command".to_owned())),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(Failure::Usage(msg)) => {
            eprintln!("nur-bundle: {msg}\n{USAGE}");
            ExitCode::from(2)
        }
        Err(Failure::Update(e)) => {
            eprintln!("nur-bundle: {e}");
            ExitCode::from(1)
        }
    }
}

enum Failure {
    Usage(String),
    Update(UpdateError),
}

impl From<UpdateError> for Failure {
    fn from(e: UpdateError) -> Self {
        Self::Update(e)
    }
}

fn parse_flags(args: &[String]) -> Result<HashMap<String, String>, Failure> {
    let mut flags = HashMap::new();
    let mut iter = args.iter();
    while let Some(flag) = iter.next() {
        let name = flag
            .strip_prefix("--")
            .ok_or_else(|| Failure::Usage(format!("unexpected argument {flag:?}")))?;
        let value = iter
            .next()
            .ok_or_else(|| Failure::Usage(format!("--{name} needs a value")))?;
        if flags.insert(name.to_owned(), value.clone()).is_some() {
            return Err(Failure::Usage(format!("--{name} given twice")));
        }
    }
    Ok(flags)
}

fn flag<'a>(flags: &'a HashMap<String, String>, name: &str) -> Result<&'a str, Failure> {
    flags
        .get(name)
        .map(String::as_str)
        .ok_or_else(|| Failure::Usage(format!("missing --{name}")))
}

fn io(context: String) -> impl FnOnce(std::io::Error) -> Failure {
    move |source| Failure::Update(UpdateError::Io { context, source })
}

fn keygen(flags: &HashMap<String, String>) -> Result<(), Failure> {
    let label = flag(flags, "label")?;
    let secret_path = Path::new(flag(flags, "out")?);
    let public_path = Path::new(flag(flags, "pub")?);
    let key =
        KeyFile::generate_development(&format!("DEVELOPMENT ONLY - NOT FOR PRODUCTION - {label}"))?;
    key.write_new(secret_path, 0o600)?;
    key.public_only().write_new(public_path, 0o644)?;
    println!(
        "generated development update-signing key {} ({})",
        key.key_id,
        secret_path.display()
    );
    Ok(())
}

fn create(flags: &HashMap<String, String>) -> Result<(), Failure> {
    let key = KeyFile::load(Path::new(flag(flags, "key")?))?;
    if key.environment != KeyEnvironment::Development {
        return Err(Failure::Usage(
            "only development keys are supported by this tool".to_owned(),
        ));
    }
    let metadata_text =
        fs::read_to_string(flag(flags, "metadata")?).map_err(io("reading metadata".to_owned()))?;
    let metadata = BuildMetadata::from_json(&metadata_text)
        .map_err(|e| Failure::Update(UpdateError::Invalid(format!("build metadata: {e}"))))?;
    let usrhash = flag(flags, "usrhash")?.to_owned();
    if !is_lower_hex(&usrhash, 64) {
        return Err(Failure::Usage(
            "--usrhash must be 64 lowercase hex digits".to_owned(),
        ));
    }
    let fault = flag(flags, "fault")?.to_owned();
    let out = PathBuf::from(flag(flags, "out")?);
    fs::DirBuilder::new()
        .mode(0o755)
        .create(&out)
        .map_err(io(format!("creating {} (must not exist)", out.display())))?;

    let mut artifacts = Vec::new();
    for (role, name) in [
        (Role::Usr, "usr"),
        (Role::UsrVerity, "usr-verity"),
        (Role::Uki, "uki"),
    ] {
        let source = PathBuf::from(flag(flags, name)?);
        let file = expected_file_name(role, metadata.version, metadata.architecture, &usrhash);
        let target = out.join(&file);
        fs::copy(&source, &target).map_err(io(format!(
            "copying {} to {}",
            source.display(),
            target.display()
        )))?;
        let (size, sha256) = nur_update::sha256_file(&target)?;
        artifacts.push(Artifact {
            role,
            file,
            size,
            sha256,
        });
    }
    let manifest = Manifest {
        schema: MANIFEST_SCHEMA.to_owned(),
        version: metadata.version,
        security_floor: metadata.security_floor,
        channel: metadata.channel.clone(),
        architecture: metadata.architecture,
        base: metadata.base.clone(),
        usrhash,
        fault_injection: fault,
        signing: SigningInfo {
            key_id: key.key_id.clone(),
            environment: key.environment,
        },
        artifacts,
    };
    manifest.validate()?;
    let bytes = serde_json::to_vec_pretty(&manifest)
        .map_err(|e| Failure::Update(UpdateError::Invalid(e.to_string())))?;
    fs::write(out.join(MANIFEST_FILE), &bytes).map_err(io("writing manifest".to_owned()))?;
    let signature = key.sign(&bytes)?;
    fs::write(out.join(SIGNATURE_FILE), signature_file_content(&signature))
        .map_err(io("writing signature".to_owned()))?;
    println!(
        "bundle {} version {} signed with DEVELOPMENT key {} (signature {}…)",
        out.display(),
        manifest.version,
        key.key_id,
        &hex_encode(&signature)[..16]
    );
    Ok(())
}

fn verify(flags: &HashMap<String, String>) -> Result<(), Failure> {
    let key = KeyFile::load(Path::new(flag(flags, "trust")?))?;
    let verified = verify_bundle(
        Path::new(flag(flags, "bundle")?),
        &TrustStore::from_keys(vec![key]),
    )?;
    println!(
        "bundle OK: version {} ({} artifacts)",
        verified.manifest.version,
        verified.manifest.artifacts.len()
    );
    Ok(())
}
