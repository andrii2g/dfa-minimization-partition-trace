use dfa_minimization_partition_trace::{dfa::Dfa, verify};
use serde_json::Value;
use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "dfa-trace-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn command() -> Command {
    Command::new(env!("CARGO_BIN_EXE_dfa-minimization-partition-trace"))
}
fn example(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("examples")
        .join(name)
}

#[test]
fn produces_verified_reloadable_artifacts_for_every_example() {
    for (name, original_count, reachable_count, minimized_count) in [
        ("redundant.json", 7, 6, 3),
        ("chain.json", 8, 8, 8),
        ("all-accepting.json", 2, 2, 1),
        ("all-rejecting.json", 2, 2, 1),
        ("already-minimal.json", 2, 2, 2),
        ("empty-alphabet.json", 2, 1, 1),
    ] {
        let scratch = Scratch::new();
        let out = scratch.0.join("result");
        let result = command()
            .arg("compare")
            .arg(example(name))
            .arg("--out")
            .arg(&out)
            .args(["--repetitions", "1"])
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let report: Value =
            serde_json::from_str(&fs::read_to_string(out.join("report.json")).unwrap()).unwrap();
        assert_eq!(report["original_states"], original_count);
        assert_eq!(report["reachable_states"], reachable_count);
        assert_eq!(report["moore"]["minimized_states"], minimized_count);
        assert_eq!(report["hopcroft"]["minimized_states"], minimized_count);
        assert_eq!(report["verification"]["canonical_machines_equal"], true);
        assert_eq!(
            report["verification"]["exhaustive"]["words_checked"],
            if name == "empty-alphabet.json" {
                1
            } else {
                511
            }
        );
        let original = Dfa::from_json(&fs::read_to_string(example(name)).unwrap()).unwrap();
        for algorithm in ["moore", "hopcroft"] {
            let minimized = Dfa::from_json(
                &fs::read_to_string(out.join(format!("{algorithm}.min.json"))).unwrap(),
            )
            .unwrap();
            assert!(
                verify::equivalent(&original, &minimized)
                    .unwrap()
                    .equivalent
            );
            assert!(out.join(format!("{algorithm}.trace.json")).exists());
            // Minimality independently: every two distinct remaining states have a witness.
            for p in 0..minimized.names.len() {
                for q in p + 1..minimized.names.len() {
                    let mut left = minimized.clone();
                    left.start = p;
                    let mut right = minimized.clone();
                    right.start = q;
                    assert!(!verify::equivalent(&left, &right).unwrap().equivalent);
                }
            }
        }
        for name in ["original", "reachable", "minimized"] {
            assert!(out.join(format!("{name}.svg")).exists());
            assert!(out.join(format!("{name}.dot")).exists());
        }
    }
}

#[test]
fn no_trace_mode_and_existing_output_protection() {
    let scratch = Scratch::new();
    let out = scratch.0.join("result");
    let result = command()
        .arg("compare")
        .arg(example("redundant.json"))
        .arg("--out")
        .arg(&out)
        .args(["--no-trace", "--max-word-length", "0"])
        .output()
        .unwrap();
    assert!(result.status.success());
    assert!(!out.join("moore.trace.json").exists());
    let before = fs::read(out.join("report.json")).unwrap();
    let repeated = command()
        .arg("compare")
        .arg(example("redundant.json"))
        .arg("--out")
        .arg(&out)
        .output()
        .unwrap();
    assert!(!repeated.status.success());
    assert!(String::from_utf8_lossy(&repeated.stderr).contains("must be empty"));
    assert_eq!(fs::read(out.join("report.json")).unwrap(), before);
}

#[test]
fn rejects_bad_input_and_excessive_enumeration_without_artifacts() {
    let scratch = Scratch::new();
    let out = scratch.0.join("result");
    let invalid = scratch.0.join("invalid.json");
    fs::write(
        &invalid,
        r#"{"alphabet":["a"],"states":["s"],"start":"s","accepting":[],"transitions":{"s":{}}}"#,
    )
    .unwrap();
    let result = command()
        .arg("compare")
        .arg(&invalid)
        .arg("--out")
        .arg(&out)
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("missing transition"));
    assert!(!out.exists());
    let result = command()
        .arg("compare")
        .arg(example("redundant.json"))
        .arg("--out")
        .arg(&out)
        .args(["--max-word-length", "30"])
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("word limit"));
    assert!(!out.exists());
}
