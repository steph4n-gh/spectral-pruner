use std::io::Write;
use std::process::{Command, Stdio};

#[test]
fn weighted_edge_cli_emits_versioned_machine_readable_output() {
    let edge_path = format!(
        "{}/tests/fixtures/attention_edges.tsv",
        env!("CARGO_MANIFEST_DIR")
    );
    let output = Command::new(env!("CARGO_BIN_EXE_spectral-pruner-audit"))
        .args([
            "--nodes",
            "7",
            "--system-start",
            "6",
            "--system-end",
            "6",
            "--threat-threshold",
            "0.9",
            &edge_path,
        ])
        .output()
        .expect("audit CLI must run");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.starts_with('{'));
    assert!(stdout.trim_end().ends_with('}'));
    assert!(stdout.contains("\"schema_version\":1"));
    assert!(stdout.contains("\"action\":\"FATAL_BLOCK\""));
    assert!(stdout.contains("\"density_ratio\":1.00000000000000000"));
    assert!(stdout.contains("\"density_triggered\":true"));
}

#[test]
fn cli_rejects_malformed_edge_rows() {
    let edge_path = format!(
        "{}/tests/fixtures/attention_edges.tsv",
        env!("CARGO_MANIFEST_DIR")
    );
    let output = Command::new(env!("CARGO_BIN_EXE_spectral-pruner-audit"))
        .args([
            "--nodes",
            "3",
            "--system-start",
            "2",
            "--system-end",
            "2",
            &edge_path,
        ])
        .output()
        .expect("audit CLI must run");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("endpoint outside 3-node topology"));
}

#[test]
fn cli_accepts_weighted_edges_from_standard_input() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_spectral-pruner-audit"))
        .args([
            "--nodes",
            "3",
            "--system-start",
            "2",
            "--system-end",
            "2",
            "-",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("audit CLI must run");
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(b"0\t1\t0.8\n0\t2\t0.2\n1\t2\t0.2\n")
        .unwrap();
    let output = child.wait_with_output().unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"schema_version\":1"));
}

#[test]
fn cli_supports_a_calibrated_connectivity_only_policy() {
    let edge_path = format!(
        "{}/tests/fixtures/attention_edges.tsv",
        env!("CARGO_MANIFEST_DIR")
    );
    let output = Command::new(env!("CARGO_BIN_EXE_spectral-pruner-audit"))
        .args([
            "--connectivity-threshold",
            "100.0",
            "--spectral-only",
            "--nodes",
            "7",
            "--system-start",
            "6",
            "--system-end",
            "6",
            &edge_path,
        ])
        .output()
        .expect("audit CLI must run");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"action\":\"FATAL_BLOCK\""));
    assert!(stdout.contains("\"connectivity_triggered\":true"));
    assert!(stdout.contains("\"density_triggered\":false"));
}
