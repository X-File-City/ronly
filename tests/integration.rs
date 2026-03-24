use std::process::Command;

fn ronly() -> Command {
    let bin = env!("CARGO_BIN_EXE_ronly");
    Command::new(bin)
}

fn ronly_run(args: &[&str]) -> std::process::Output {
    ronly()
        .arg("--")
        .args(args)
        .output()
        .expect("failed to run ronly")
}

fn ronly_sh(cmd: &str) -> std::process::Output {
    ronly()
        .arg("--")
        .args(["bash", "-c", cmd])
        .output()
        .expect("failed to run ronly")
}

fn stdout(out: &std::process::Output) -> String {
    String::from_utf8_lossy(&out.stdout).to_string()
}

fn stderr(out: &std::process::Output) -> String {
    String::from_utf8_lossy(&out.stderr).to_string()
}

fn combined(out: &std::process::Output) -> String {
    format!("{}{}", stdout(out), stderr(out))
}

fn skip_if_not_root() {
    if !nix::unistd::geteuid().is_root() {
        eprintln!("skipping: not root");
        return;
    }
}

// --- read operations ---

#[test]
fn echo_hello() {
    skip_if_not_root();
    let out = ronly_run(&["echo", "hello"]);
    assert!(out.status.success());
    assert!(stdout(&out).contains("hello"));
}

#[test]
fn cat_etc_hostname() {
    skip_if_not_root();
    let out = ronly_run(&["cat", "/etc/hostname"]);
    assert!(out.status.success());
    assert!(!stdout(&out).is_empty());
}

#[test]
fn ls_root() {
    skip_if_not_root();
    let out = ronly_run(&["ls", "/"]);
    assert!(out.status.success());
}

#[test]
fn ps_aux() {
    skip_if_not_root();
    let out = ronly_sh("ps aux | head -3");
    assert!(out.status.success());
}

// --- write operations blocked ---

#[test]
fn rm_blocked() {
    skip_if_not_root();
    let out = ronly_sh("rm /etc/hostname 2>&1");
    assert!(!out.status.success());
    let text = combined(&out).to_lowercase();
    assert!(
        text.contains("read-only")
            || text.contains("not permitted")
    );
}

#[test]
fn touch_blocked() {
    skip_if_not_root();
    let out = ronly_sh("touch /etc/ronly_test 2>&1");
    assert!(!out.status.success());
}

#[test]
fn mkdir_blocked() {
    skip_if_not_root();
    let out = ronly_sh("mkdir /etc/ronly_test 2>&1");
    assert!(!out.status.success());
}

// --- /tmp writable ---

#[test]
fn tmp_writable() {
    skip_if_not_root();
    let out = ronly_sh(
        "echo test > /tmp/ronly_test && cat /tmp/ronly_test",
    );
    assert!(out.status.success());
    assert!(stdout(&out).contains("test"));
}

// --- pid namespace ---

#[test]
fn ps_shows_host_init() {
    skip_if_not_root();
    let out = ronly_run(&["ps", "-p", "1", "-o", "comm="]);
    assert!(out.status.success());
    let text = stdout(&out).to_lowercase();
    assert!(
        text.contains("init") || text.contains("systemd")
    );
}

#[test]
fn own_pid_is_1() {
    skip_if_not_root();
    let out = ronly_sh("echo $$");
    assert!(out.status.success());
    assert_eq!(stdout(&out).trim(), "1");
}

// --- seccomp ---

#[test]
fn kill_blocked() {
    skip_if_not_root();
    let out = ronly_sh("kill 1 2>&1");
    assert!(!out.status.success());
    assert!(combined(&out)
        .to_lowercase()
        .contains("not permitted"));
}

// --- shims ---

#[test]
fn docker_exec_blocked() {
    skip_if_not_root();
    let out = ronly_sh("docker exec foo bar 2>&1");
    assert!(!out.status.success());
    assert!(combined(&out).contains("blocked"));
}

#[test]
fn docker_stop_blocked() {
    skip_if_not_root();
    let out = ronly_sh("docker stop foo 2>&1");
    assert!(!out.status.success());
    assert!(combined(&out).contains("blocked"));
}

#[test]
fn kubectl_delete_blocked() {
    skip_if_not_root();
    let out = ronly_sh("kubectl delete pod foo 2>&1");
    assert!(!out.status.success());
    assert!(combined(&out).contains("blocked"));
}

#[test]
fn kubectl_apply_blocked() {
    skip_if_not_root();
    let out = ronly_sh("kubectl apply -f foo 2>&1");
    assert!(!out.status.success());
    assert!(combined(&out).contains("blocked"));
}

// --- exit codes ---

#[test]
fn exit_0() {
    skip_if_not_root();
    let out = ronly_run(&["true"]);
    assert!(out.status.success());
}

#[test]
fn exit_1() {
    skip_if_not_root();
    let out = ronly_run(&["false"]);
    assert_eq!(out.status.code(), Some(1));
}

#[test]
fn exit_42() {
    skip_if_not_root();
    let out = ronly_sh("exit 42");
    assert_eq!(out.status.code(), Some(42));
}
