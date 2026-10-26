#[test]
fn help_and_version_do_not_start_the_ui_or_expose_internal_mode() {
    let help = usage_rs::test::command!("aftercast", "--help").assert_success();
    assert!(help.stdout_text().contains("aftercast"));
    assert!(!help.stdout_text().contains("aftercast-child"));
    let version = usage_rs::test::command!("aftercast", "--version").assert_success();
    assert!(version.stdout_text().contains("0.1.0"));
}

#[test]
fn direct_child_invocation_requires_supervisor_authentication() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_aftercast"))
        .arg("--aftercast-child")
        .env_remove("AFTERCAST_WORKER_ENDPOINT")
        .output()
        .expect("run isolated child without supervisor");
    assert!(!output.status.success());
    assert!(!output.stderr.is_empty());
}
