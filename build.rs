use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();

    // Read version from Cargo.toml
    let cargo_toml = fs::read_to_string(Path::new(&manifest_dir).join("Cargo.toml")).unwrap();
    let version = cargo_toml
        .lines()
        .find(|l| l.starts_with("version"))
        .and_then(|l| l.split('"').nth(1))
        .unwrap_or("0.0.0");

    // Generate config.rs
    let config_rs = format!("pub static VERSION: &str = \"{}\";\n", version);
    fs::write(Path::new(&out_dir).join("config.rs"), &config_rs).unwrap();

    // Also write to src/config.rs for IDE support
    fs::write(
        Path::new(&manifest_dir).join("src/config.rs"),
        &config_rs,
    )
    .unwrap();

    // Compile GSettings schema
    let schema_dir = Path::new(&manifest_dir).join("data");
    let schema_file = schema_dir.join("io.github.antraxbr666.Btman.gschema.xml");

    if schema_file.exists() {
        let target_debug = Path::new(&manifest_dir).join("target").join("debug");
        fs::create_dir_all(&target_debug).unwrap();

        let status = Command::new("glib-compile-schemas")
            .arg("--strict")
            .arg("--targetdir")
            .arg(&target_debug)
            .arg(&schema_dir)
            .status()
            .expect("Failed to run glib-compile-schemas. Is libglib installed?");

        if !status.success() {
            panic!("glib-compile-schemas failed");
        }
    }

    println!("cargo:rerun-if-changed=data/io.github.antraxbr666.Btman.gschema.xml");
    println!("cargo:rerun-if-changed=Cargo.toml");
}
