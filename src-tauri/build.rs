use std::{
    env,
    fmt::Write as _,
    fs,
    path::{Path, PathBuf},
};

fn main() {
    generate_bundled_rule_manifest();
    tauri_build::build()
}

fn generate_bundled_rule_manifest() {
    let manifest_dir = PathBuf::from(
        env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is unavailable"),
    );
    let rules_dir = manifest_dir.join("resources").join("app-rules");
    println!("cargo:rerun-if-changed={}", rules_dir.display());

    let mut rule_files = read_rule_files(&rules_dir);
    rule_files.sort();

    let mut generated = String::from("pub const BUNDLED_RULE_SOURCES: &[(&str, &str)] = &[\n");
    for file_name in rule_files {
        let relative_path = format!("/resources/app-rules/{file_name}");
        writeln!(
            generated,
            "    ({file_name:?}, include_str!(concat!(env!(\"CARGO_MANIFEST_DIR\"), {relative_path:?}))),"
        )
        .expect("writing to a String cannot fail");
    }
    generated.push_str("];\n");

    let output_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR is unavailable"));
    fs::write(output_dir.join("bundled_application_rules.rs"), generated)
        .expect("failed to generate bundled application rule manifest");
}

fn read_rule_files(rules_dir: &Path) -> Vec<String> {
    let entries = fs::read_dir(rules_dir).expect("failed to read resources/app-rules");
    entries
        .map(|entry| entry.expect("failed to read an application rule directory entry"))
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
        .filter_map(|entry| {
            let path = entry.path();
            let is_json = path
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("json"));
            let file_name = entry.file_name();
            let file_name = file_name
                .to_str()
                .expect("application rule filenames must be valid UTF-8");
            (is_json && !file_name.eq_ignore_ascii_case("schema.json"))
                .then(|| file_name.to_owned())
        })
        .collect()
}
