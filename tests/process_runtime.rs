use runact::process::ProcessSpawnOptions;
use std::time::Duration;

#[test]
fn test_spawn_simple_command() {
    let handle = ProcessSpawnOptions::new("echo")
        .arg("hello")
        .spawn()
        .expect("spawn process");

    let output = handle.wait().expect("wait for process");
    assert_eq!(output.exit_code, Some(0));
    assert_eq!(output.stdout, "hello\n");
}

#[test]
fn test_spawn_captures_stderr() {
    let handle = ProcessSpawnOptions::new("sh")
        .args(&["-c", "echo error >&2"])
        .spawn()
        .expect("spawn process");

    let output = handle.wait().expect("wait for process");
    assert_eq!(output.exit_code, Some(0));
    assert_eq!(output.stderr, "error\n");
}

#[test]
fn test_spawn_nonzero_exit() {
    let handle = ProcessSpawnOptions::new("sh")
        .args(&["-c", "exit 42"])
        .spawn()
        .expect("spawn process");

    let output = handle.wait().expect("wait for process");
    assert_eq!(output.exit_code, Some(42));
}

#[test]
fn test_process_timeout() {
    let handle = ProcessSpawnOptions::new("sleep")
        .arg("10")
        .spawn()
        .expect("spawn process");

    let result = handle.wait_timeout(Duration::from_millis(100));
    assert!(result.is_err() || result.unwrap().exit_code.is_none());
}

#[test]
fn test_process_cancel() {
    let mut handle = ProcessSpawnOptions::new("sleep")
        .arg("10")
        .spawn()
        .expect("spawn process");

    handle.kill().expect("cancel process");
    let output = handle.wait().expect("wait after cancel");
    assert_ne!(output.exit_code, Some(0));
}

#[test]
fn test_process_stdin_write() {
    let mut handle = ProcessSpawnOptions::new("cat")
        .stdin_piped()
        .spawn()
        .expect("spawn process");

    handle.write_stdin(b"hello").expect("write stdin");
    handle.close_stdin().expect("close stdin");

    let output = handle.wait().expect("wait for process");
    assert_eq!(output.stdout, "hello");
}

#[test]
fn test_process_stdout_streaming() {
    let mut handle = ProcessSpawnOptions::new("sh")
        .args(&["-c", "echo line1; echo line2; echo line3"])
        .stdout_piped()
        .spawn()
        .expect("spawn process");

    let mut lines = Vec::new();
    while let Some(line) = handle.read_stdout_line().expect("read line") {
        lines.push(line);
    }

    assert_eq!(lines.len(), 3);
    assert_eq!(lines[0], "line1");
    assert_eq!(lines[1], "line2");
    assert_eq!(lines[2], "line3");

    let output = handle.wait().expect("wait for process");
    assert_eq!(output.exit_code, Some(0));
}

#[test]
fn test_process_not_orphaned_on_drop() {
    let handle = ProcessSpawnOptions::new("sleep")
        .arg("100")
        .spawn()
        .expect("spawn process");

    drop(handle);
    std::thread::sleep(Duration::from_millis(50));
}

#[test]
fn test_process_env_vars() {
    let handle = ProcessSpawnOptions::new("sh")
        .args(&["-c", "echo $MY_TEST_VAR"])
        .env("MY_TEST_VAR", "test_value")
        .spawn()
        .expect("spawn process");

    let output = handle.wait().expect("wait for process");
    assert_eq!(output.stdout, "test_value\n");
}

#[test]
fn test_process_working_directory() {
    let handle = ProcessSpawnOptions::new("pwd")
        .current_dir("/tmp")
        .spawn()
        .expect("spawn process");

    let output = handle.wait().expect("wait for process");
    assert_eq!(output.stdout.trim(), "/tmp");
}
