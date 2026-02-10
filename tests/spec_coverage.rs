use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read_to_string(p: &Path) -> String {
    fs::read_to_string(p).unwrap_or_else(|e| panic!("failed to read {p:?}: {e}"))
}

fn extract_requirements(spec: &str) -> Vec<(String, bool)> {
    // Returns (id, is_manual)
    let mut out = Vec::new();

    for line in spec.lines() {
        let mut i = 0;
        while let Some(pos) = line[i..].find("GW-") {
            let start = i + pos;
            let mut end = start;
            for (off, ch) in line[start..].char_indices() {
                if ch.is_ascii_uppercase() || ch.is_ascii_digit() || ch == '-' {
                    end = start + off + ch.len_utf8();
                } else {
                    break;
                }
            }
            let candidate = &line[start..end];
            i = end;

            if is_req_id(candidate) {
                let id = candidate.to_string();
                let is_manual = line.contains("[manual]") || line.contains("[no-test]");
                out.push((id, is_manual));
            }
        }
    }
    out
}

fn is_req_id(s: &str) -> bool {
    // Accept: GW-FOO-001, GW-FOO-BAR-012, etc.
    if !s.starts_with("GW-") {
        return false;
    }
    let (_, tail) = match s.rsplit_once('-') {
        Some(x) => x,
        None => return false,
    };
    if tail.len() != 3 || !tail.chars().all(|c| c.is_ascii_digit()) {
        return false;
    }
    s.chars()
        .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '-')
}

fn read_rs_files(dir: &Path) -> Vec<(PathBuf, String)> {
    let mut out = Vec::new();
    for ent in fs::read_dir(dir).unwrap() {
        let ent = ent.unwrap();
        let p = ent.path();
        if ent.file_type().unwrap().is_dir() {
            out.extend(read_rs_files(&p));
            continue;
        }
        if p.extension().and_then(|s| s.to_str()) != Some("rs") {
            continue;
        }
        out.push((p.clone(), read_to_string(&p)));
    }
    out
}

#[test]
fn spec_requirement_ids_are_covered_by_tests() {
    let spec_path = repo_root().join("docs").join("spec.md");
    let spec = read_to_string(&spec_path);

    let reqs = extract_requirements(&spec);
    assert!(
        !reqs.is_empty(),
        "no requirement IDs found in {:?}",
        spec_path
    );

    let mut manual = BTreeSet::new();
    let mut required = BTreeSet::new();
    for (id, is_manual) in reqs {
        if is_manual {
            manual.insert(id);
        } else {
            required.insert(id);
        }
    }

    // Ensure IDs are unique in the spec (easy to accidentally duplicate).
    let mut seen = BTreeSet::new();
    for id in manual.iter().chain(required.iter()) {
        assert!(seen.insert(id.clone()), "duplicate requirement ID in spec: {id}");
    }

    let tests_dir = repo_root().join("tests");
    let mut files = read_rs_files(&tests_dir);
    // Also allow unit tests co-located with code (e.g. src/picker.rs).
    files.extend(read_rs_files(&repo_root().join("src")));
    assert!(!files.is_empty(), "no .rs files found to scan for spec coverage");

    let mut covered_by: HashMap<String, Vec<PathBuf>> = HashMap::new();
    for (path, contents) in &files {
        for id in required.iter() {
            for line in contents.lines() {
                if line.contains("spec:") && line.contains(id) {
                    covered_by.entry(id.clone()).or_default().push(path.clone());
                    break;
                }
            }
        }
    }

    let mut missing = Vec::new();
    for id in required.iter() {
        if !covered_by.contains_key(id) {
            missing.push(id.clone());
        }
    }

    if !missing.is_empty() {
        panic!(
            "spec requirements missing test references:\n{}\n\nAdd `// spec: <ID>` to at least one test that covers each requirement.\nManual requirements can be marked with `[manual]` in docs/spec.md.",
            missing.join("\n")
        );
    }
}
