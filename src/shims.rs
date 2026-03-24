#![allow(dead_code)]
use anyhow::Result;
use std::fs;
use std::path::Path;

pub const SHIMS_DIR: &str = "/usr/lib/ronly/shims";

/// Tools we provide shims for.
const SHIMMED_TOOLS: &[&str] =
    &["docker", "kubectl"];

/// Bind-mount our own binary into SHIMS_DIR under each
/// tool name. `exe` must be the resolved path to our
/// binary, obtained before any mounts changed.
#[cfg(target_os = "linux")]
pub fn install_shims(
    exe: &std::path::Path,
) -> Result<()> {
    use nix::mount::MsFlags;
    let dir = Path::new(SHIMS_DIR);
    fs::create_dir_all(dir)?;
    for name in SHIMMED_TOOLS {
        let dest = dir.join(name);
        fs::write(&dest, b"")?;
        nix::mount::mount(
            Some(exe),
            &dest,
            None::<&str>,
            MsFlags::MS_BIND,
            None::<&str>,
        )?;
    }
    Ok(())
}


/// Check if we were invoked as a shim (argv[0] is a tool
/// name, not "ronly"). If so, handle it and exit.
/// Returns None if we're running as ronly itself.
pub fn maybe_run_as_shim() -> Option<i32> {
    let argv0 = std::env::args().next()?;
    let name = Path::new(&argv0)
        .file_name()?
        .to_str()?;

    match name {
        "docker" => Some(shim_docker()),
        "kubectl" => Some(shim_kubectl()),
        _ => None,
    }
}

fn shim_docker() -> i32 {
    let args: Vec<String> =
        std::env::args().skip(1).collect();
    let sub = args.first().map(|s| s.as_str());

    match sub {
        Some(
            "ps" | "logs" | "inspect" | "stats" | "top"
            | "images" | "info" | "version" | "events"
            | "diff",
        ) => exec_real("/usr/bin/docker"),
        Some("network" | "volume") => {
            let sub2 =
                args.get(1).map(|s| s.as_str());
            match sub2 {
                Some("ls" | "inspect") => {
                    exec_real("/usr/bin/docker")
                }
                _ => {
                    let s = sub.unwrap();
                    let s2 = sub2.unwrap_or("(none)");
                    blocked("docker", &format!("{} {}", s, s2))
                }
            }
        }
        Some(s) => blocked("docker", s),
        None => blocked("docker", "(no subcommand)"),
    }
}

fn shim_kubectl() -> i32 {
    let args: Vec<String> =
        std::env::args().skip(1).collect();
    let sub = args.first().map(|s| s.as_str());

    match sub {
        Some(
            "get" | "describe" | "logs" | "top"
            | "explain" | "version" | "cluster-info"
            | "api-resources" | "api-versions",
        ) => exec_real("/usr/bin/kubectl"),
        Some("config") => {
            let sub2 =
                args.get(1).map(|s| s.as_str());
            match sub2 {
                Some(
                    "view" | "current-context"
                    | "get-contexts",
                ) => exec_real("/usr/bin/kubectl"),
                _ => blocked(
                    "kubectl",
                    &format!(
                        "config {}",
                        sub2.unwrap_or("(none)")
                    ),
                ),
            }
        }
        Some("auth") => {
            let sub2 =
                args.get(1).map(|s| s.as_str());
            match sub2 {
                Some("can-i" | "whoami") => {
                    exec_real("/usr/bin/kubectl")
                }
                _ => blocked(
                    "kubectl",
                    &format!(
                        "auth {}",
                        sub2.unwrap_or("(none)")
                    ),
                ),
            }
        }
        Some(s) => blocked("kubectl", s),
        None => blocked("kubectl", "(no subcommand)"),
    }
}

fn blocked(tool: &str, sub: &str) -> i32 {
    eprintln!(
        "ronly: {} {} is blocked (read-only session)",
        tool, sub
    );
    1
}

fn exec_real(bin: &str) -> i32 {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    let args: Vec<CString> = std::env::args_os()
        .map(|a| {
            CString::new(a.as_bytes()).unwrap()
        })
        .collect();
    let bin_c = CString::new(bin).unwrap();
    let mut argv: Vec<*const libc::c_char> =
        args.iter().map(|a| a.as_ptr()).collect();
    // Replace argv[0] with real binary path
    argv[0] = bin_c.as_ptr();
    argv.push(std::ptr::null());
    unsafe { libc::execv(bin_c.as_ptr(), argv.as_ptr()) };
    // If we get here, exec failed
    eprintln!(
        "{}: command not found",
        std::env::args().next().unwrap_or_default()
    );
    127
}
