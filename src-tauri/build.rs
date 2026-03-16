fn main() {
    // Scan migrations directory and generate MIGRATIONS constant
    let migrations_dir = std::path::Path::new("../migrations");
    println!("cargo:rerun-if-changed=../migrations");

    let mut sql_files: Vec<String> = std::fs::read_dir(migrations_dir)
        .expect("failed to read migrations directory")
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let name = entry.file_name().into_string().ok()?;
            if name.ends_with(".sql") {
                Some(name)
            } else {
                None
            }
        })
        .collect();

    sql_files.sort();

    for filename in &sql_files {
        println!("cargo:rerun-if-changed=../migrations/{filename}");
    }

    let entries: String = sql_files
        .iter()
        .map(|f| {
            format!(
                "    (\"{f}\", include_str!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/../migrations/{f}\"))),\n"
            )
        })
        .collect();

    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR not set");
    let dest = std::path::Path::new(&out_dir).join("migrations_list.rs");
    std::fs::write(
        dest,
        format!("const MIGRATIONS: &[(&str, &str)] = &[\n{entries}];\n"),
    )
    .expect("failed to write migrations_list.rs");

    tauri_build::build()
}
