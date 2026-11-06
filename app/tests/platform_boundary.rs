//! Constitution IV: only `crates/platform` talks to the operating system (research R2, R13).
//!
//! `cargo deny` cannot ban `windows-sys` or `zbus` by name, since winit, wgpu and Slint pull them
//! in, so this test checks the direct dependencies of `app` and `settings` and the platform
//! conditions in their sources.

use std::fs;
use std::path::{Path, PathBuf};

/// Crates that call the operating system; only `crates/platform` may depend on them.
const OS_CRATES: &[&str] = &[
    "windows-sys",
    "windows",
    "tray-icon",
    "muda",
    "ksni",
    "zbus",
    "libc",
    "nix",
];

/// Words that make a `cfg` condition platform-specific.
const PLATFORM_PREDICATES: &[&str] = &["windows", "unix", "target_os", "target_family"];

/// The crates that must stay free of OS code, relative to the workspace root.
const CHECKED_CRATES: &[&str] = &["app", "crates/settings"];

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("app is inside the workspace")
        .to_path_buf()
}

/// Names of the direct dependencies in every dependency table of a manifest, target-specific
/// tables included. A renamed dependency (`x = { package = "y" }`) counts as `y`.
fn direct_dependencies(manifest: &str) -> Vec<String> {
    let doc: toml::Table = manifest.parse().expect("Cargo.toml parses");
    let mut owners: Vec<&toml::Table> = vec![&doc];
    if let Some(targets) = doc.get("target").and_then(toml::Value::as_table) {
        owners.extend(targets.values().filter_map(toml::Value::as_table));
    }
    let tables = owners.into_iter().flat_map(|owner| {
        ["dependencies", "dev-dependencies", "build-dependencies"]
            .into_iter()
            .filter_map(|key| owner.get(key).and_then(toml::Value::as_table))
    });
    let mut names = Vec::new();
    for deps in tables {
        for (key, value) in deps {
            let package = value
                .as_table()
                .and_then(|t| t.get("package"))
                .and_then(toml::Value::as_str)
                .unwrap_or(key);
            names.push(package.to_string());
        }
    }
    names
}

/// Every platform-specific `cfg` in a source file, as `(line, condition)`. Looks at `cfg(...)`,
/// `cfg!(...)` and `cfg_attr(...)`; `#[cfg(test)]` and other conditions without a platform word
/// pass.
fn platform_conditions(source: &str) -> Vec<(usize, String)> {
    let mut found = Vec::new();
    for (index, line) in source.lines().enumerate() {
        let code = line.split("//").next().unwrap_or("");
        let mut rest = code;
        while let Some(start) = rest.find("cfg") {
            let after = &rest[start + 3..];
            let after = after.strip_prefix("_attr").unwrap_or(after);
            let after = after.strip_prefix('!').unwrap_or(after);
            if let Some(inner) = after.strip_prefix('(') {
                let condition = balanced(inner);
                if is_platform_specific(condition) {
                    found.push((index + 1, format!("cfg({condition})")));
                }
            }
            rest = &rest[start + 3..];
        }
    }
    found
}

/// The text up to the parenthesis that closes the one just opened (or the rest of the line).
fn balanced(text: &str) -> &str {
    let mut depth = 1;
    for (i, c) in text.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return &text[..i];
                }
            }
            _ => {}
        }
    }
    text
}

fn is_platform_specific(condition: &str) -> bool {
    condition
        .split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .any(|word| PLATFORM_PREDICATES.contains(&word))
}

fn rust_files(folder: &Path, files: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(folder).expect("source folder is readable") {
        let path = entry.expect("folder entry").path();
        if path.is_dir() {
            rust_files(&path, files);
        } else if path.extension().is_some_and(|e| e == "rs") {
            files.push(path);
        }
    }
}

#[test]
fn no_os_crate_is_a_direct_dependency() {
    let root = workspace_root();
    let mut failures = Vec::new();
    for krate in CHECKED_CRATES {
        let manifest_path = root.join(krate).join("Cargo.toml");
        let manifest = fs::read_to_string(&manifest_path).expect("Cargo.toml is readable");
        for name in direct_dependencies(&manifest) {
            if OS_CRATES.contains(&name.as_str()) {
                failures.push(format!(
                    "{krate} depends on {name}; OS calls go through crates/platform"
                ));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn no_platform_condition_in_sources() {
    let root = workspace_root();
    let mut failures = Vec::new();
    for krate in CHECKED_CRATES {
        let mut files = Vec::new();
        rust_files(&root.join(krate).join("src"), &mut files);
        for file in files {
            let source = fs::read_to_string(&file).expect("source is readable");
            for (line, condition) in platform_conditions(&source) {
                let shown = file.strip_prefix(&root).unwrap_or(&file).display();
                failures.push(format!(
                    "{shown}:{line}: {condition}; platform code goes in crates/platform"
                ));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn finds_os_crates_in_every_table() {
    let manifest = r#"
        [dependencies]
        log = "0.4"
        sys = { package = "windows-sys", version = "0.61" }

        [dev-dependencies]
        libc = "0.2"

        [target.'cfg(target_os = "linux")'.dependencies]
        zbus = "5"
    "#;
    let names = direct_dependencies(manifest);
    for expected in ["log", "windows-sys", "libc", "zbus"] {
        assert!(names.iter().any(|n| n == expected), "{expected} in {names:?}");
    }
}

#[test]
fn finds_platform_conditions_and_skips_others() {
    let source = "\
#[cfg(test)]
mod tests {}
#[cfg(windows)]
fn a() {}
#[cfg(not(windows))]
fn b() {}
#[cfg_attr(target_os = \"linux\", allow(dead_code))]
fn c() {}
fn d() -> bool { cfg!(unix) }
#[cfg(all(test, feature = \"windows_like\"))]
fn e() {}
// #[cfg(windows)] in a comment
";
    let lines: Vec<usize> = platform_conditions(source).iter().map(|f| f.0).collect();
    assert_eq!(lines, vec![3, 5, 7, 9]);
}
