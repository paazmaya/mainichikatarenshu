use std::env;
use std::path::Path;
#[path = "build_support/ca_config.rs"]
mod ca_config;
#[path = "build_support/kata_config.rs"]
mod kata_config;

/// Embed the validated kata names from `kata.yaml` (or `KATA_CONFIG`) as `kata.rs`.
fn generate_kata_list(out_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-env-changed=KATA_CONFIG");
    let path = env::var("KATA_CONFIG").unwrap_or_else(|_| "kata.yaml".to_owned());
    println!("cargo:rerun-if-changed={path}");
    let names = match std::fs::read_to_string(&path) {
        Ok(contents) => kata_config::parse(&contents)?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            if env::var_os("KATA_CONFIG").is_some() {
                return Err(format!("configured kata file '{path}' does not exist").into());
            }
            println!("cargo:warning=kata.yaml is absent; the firmware requires configuration");
            Vec::new()
        }
        Err(error) => return Err(error.into()),
    };
    let mut output =
        String::from("/// Kata names in configuration order.\npub const KATA: &[&str] = &[\n");
    for name in names {
        output.push_str(&format!("    {name:?},\n"));
    }
    output.push_str("];\n");
    std::fs::write(out_dir.join("kata.rs"), output)?;
    Ok(())
}

/// Embed the optional Supabase connection settings as the expression in `supabase.rs`.
///
/// Synchronization is enabled only when both `SUPABASE_HOST` and
/// `SUPABASE_ANON_KEY` are set; `SUPABASE_TABLE` defaults to `training_records`.
fn generate_supabase_config(out_dir: &Path) -> std::io::Result<()> {
    for name in ["SUPABASE_HOST", "SUPABASE_ANON_KEY", "SUPABASE_TABLE"] {
        println!("cargo:rerun-if-env-changed={name}");
    }
    let setting = |name| env::var(name).ok().filter(|value| !value.trim().is_empty());
    let output = match (setting("SUPABASE_HOST"), setting("SUPABASE_ANON_KEY")) {
        (Some(host), Some(api_key)) => {
            let table = setting("SUPABASE_TABLE").unwrap_or_else(|| "training_records".to_owned());
            format!("Some(SupabaseConfig::new({host:?}, {api_key:?}, {table:?}))")
        }
        _ => "None".to_owned(),
    };
    std::fs::write(out_dir.join("supabase.rs"), output)
}

/// Validate and embed the trusted TLS root CA bundle as `ca_bundle.rs`.
fn generate_ca_bundle(out_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-env-changed=TLS_CA_BUNDLE");
    let path = env::var("TLS_CA_BUNDLE").unwrap_or_else(|_| "certs/ca-roots.pem".to_owned());
    println!("cargo:rerun-if-changed={path}");
    let contents = std::fs::read_to_string(&path)
        .map_err(|error| format!("cannot read TLS CA bundle '{path}': {error}"))?;
    let mut normalized = ca_config::parse_pem_bundle(&contents)?;
    normalized.push('\0');
    let bytes = normalized.as_bytes();
    let output = format!(
        "/// Trusted root CA certificates in nul-terminated PEM format.\npub const CA_BUNDLE_PEM: &core::ffi::CStr = match core::ffi::CStr::from_bytes_with_nul(&{bytes:?}) {{\n    Ok(bundle) => bundle,\n    Err(_) => panic!(\"invalid embedded CA bundle\"),\n}};\n"
    );
    std::fs::write(out_dir.join("ca_bundle.rs"), output)?;
    Ok(())
}

/// Embed the firmware build time as a Unix-seconds expression in `build_time.rs`.
///
/// `SOURCE_DATE_EPOCH` is honored for reproducible builds. The script reruns whenever the
/// sources or manifests change, so the value is the time of the latest real rebuild.
fn generate_build_time(out_dir: &Path) -> std::io::Result<()> {
    println!("cargo:rerun-if-env-changed=SOURCE_DATE_EPOCH");
    for path in ["src", "crates", "Cargo.toml", "Cargo.lock"] {
        println!("cargo:rerun-if-changed={path}");
    }
    let from_env = env::var("SOURCE_DATE_EPOCH")
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok());
    let secs = from_env.unwrap_or_else(|| {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_secs())
    });
    std::fs::write(out_dir.join("build_time.rs"), format!("{secs}u64"))
}

fn main() {
    let out_dir = env::var("OUT_DIR").expect("Cargo sets OUT_DIR");
    let out_dir = Path::new(&out_dir);
    generate_kata_list(out_dir).expect("invalid kata configuration");
    generate_supabase_config(out_dir).expect("cannot write Supabase configuration");
    generate_ca_bundle(out_dir).expect("invalid TLS CA bundle configuration");
    generate_build_time(out_dir).expect("cannot write build time");
    println!("cargo:rerun-if-changed=build_support/ca_config.rs");
    println!("cargo:rerun-if-changed=build_support/kata_config.rs");
}
