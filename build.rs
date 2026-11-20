use std::env;
use std::path::Path;
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

fn main() {
    let out_dir = env::var("OUT_DIR").expect("Cargo sets OUT_DIR");
    let out_dir = Path::new(&out_dir);
    generate_kata_list(out_dir).expect("invalid kata configuration");
    generate_supabase_config(out_dir).expect("cannot write Supabase configuration");
    println!("cargo:rerun-if-changed=build_support/kata_config.rs");
}
