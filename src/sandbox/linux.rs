use anyhow::Result;
use nix::mount::MsFlags;
use nix::sched::CloneFlags;
use nix::unistd::ForkResult;
use std::collections::BTreeMap;
use std::ffi::CString;

use crate::shims;
use crate::Args;

fn die(msg: &str) -> ! {
    eprintln!("{}", msg);
    unsafe { libc::_exit(1) }
}

fn setup_mounts(args: &Args) -> Result<()> {
    // Create dirs before going read-only
    std::fs::create_dir_all(shims::SHIMS_DIR).ok();
    for p in &args.writable {
        std::fs::create_dir_all(p).ok();
    }

    // Private mount tree
    nix::mount::mount(
        None::<&str>,
        "/",
        None::<&str>,
        MsFlags::MS_REC | MsFlags::MS_PRIVATE,
        None::<&str>,
    )?;

    // Read-only root
    nix::mount::mount(
        Some("/"),
        "/",
        None::<&str>,
        MsFlags::MS_BIND
            | MsFlags::MS_REMOUNT
            | MsFlags::MS_RDONLY
            | MsFlags::MS_REC,
        None::<&str>,
    )?;

    // Writable /tmp
    let size = &args.tmpfs_size;
    nix::mount::mount(
        Some("tmpfs"),
        "/tmp",
        Some("tmpfs"),
        MsFlags::MS_NOSUID | MsFlags::MS_NODEV,
        Some(format!("size={}", size).as_str()),
    )?;

    // Additional writable paths
    for p in &args.writable {
        let p = p.to_string_lossy();
        nix::mount::mount(
            Some("tmpfs"),
            p.as_ref(),
            Some("tmpfs"),
            MsFlags::MS_NOSUID | MsFlags::MS_NODEV,
            Some(format!("size={}", size).as_str()),
        )?;
    }

    // Writable shims dir
    nix::mount::mount(
        Some("tmpfs"),
        shims::SHIMS_DIR,
        Some("tmpfs"),
        MsFlags::MS_NOSUID | MsFlags::MS_NODEV,
        Some("size=8m"),
    )?;

    Ok(())
}

fn setup_seccomp() -> Result<()> {
    use seccompiler::SeccompAction;
    use seccompiler::SeccompCmpArgLen;
    use seccompiler::SeccompCmpOp;
    use seccompiler::SeccompCondition;
    use seccompiler::SeccompFilter;
    use seccompiler::SeccompRule;

    #[allow(unused_mut)]
    let mut blocked: Vec<i64> = vec![
        libc::SYS_kill,
        libc::SYS_tkill,
        libc::SYS_tgkill,
        libc::SYS_unlinkat,
        libc::SYS_renameat,
        libc::SYS_renameat2,
        libc::SYS_truncate,
        libc::SYS_ftruncate,
        libc::SYS_mount,
        libc::SYS_umount2,
        libc::SYS_reboot,
    ];
    #[cfg(target_arch = "x86_64")]
    blocked.extend_from_slice(&[
        libc::SYS_unlink,
        libc::SYS_rmdir,
        libc::SYS_rename,
    ]);

    let mut rules: BTreeMap<i64, Vec<SeccompRule>> =
        blocked
            .into_iter()
            .map(|sc| (sc, vec![]))
            .collect();

    // ptrace: block write ops, allow read ops
    #[allow(unused_mut)]
    let mut ptrace_write_ops: Vec<u64> = vec![
        libc::PTRACE_POKETEXT as u64,
        libc::PTRACE_POKEDATA as u64,
        libc::PTRACE_POKEUSER as u64,
        libc::PTRACE_SETREGSET as u64,
    ];
    #[cfg(target_arch = "x86_64")]
    ptrace_write_ops.extend_from_slice(&[
        libc::PTRACE_SETREGS as u64,
        libc::PTRACE_SETFPREGS as u64,
    ]);
    let ptrace_rules: Vec<SeccompRule> = ptrace_write_ops
        .into_iter()
        .map(|op| {
            SeccompRule::new(vec![SeccompCondition::new(
                0,
                SeccompCmpArgLen::Dword,
                SeccompCmpOp::Eq,
                op,
            )
            .unwrap()])
            .unwrap()
        })
        .collect();
    rules.insert(libc::SYS_ptrace, ptrace_rules);

    let arch =
        std::env::consts::ARCH.try_into().map_err(|e| {
            anyhow::anyhow!("unsupported arch: {}", e)
        })?;

    let filter = SeccompFilter::new(
        rules,
        SeccompAction::Allow,
        SeccompAction::Errno(libc::EPERM as u32),
        arch,
    )?;

    let bpf: seccompiler::BpfProgram = filter.try_into()?;
    seccompiler::apply_filter(&bpf)?;
    Ok(())
}

pub fn run(args: Args) -> Result<()> {
    // Read our binary before mounts (it may be under /tmp)
    let self_bin = if !args.no_shims {
        Some(shims::read_self_exe()?)
    } else {
        None
    };

    // Unshare mount + PID namespace
    nix::sched::unshare(
        CloneFlags::CLONE_NEWNS | CloneFlags::CLONE_NEWPID,
    )?;

    // Fork for PID namespace — child is PID 1
    match unsafe { nix::unistd::fork()? } {
        ForkResult::Parent { child } => {
            let status =
                nix::sys::wait::waitpid(child, None)?;
            let code = match status {
                nix::sys::wait::WaitStatus::Exited(
                    _, c,
                ) => c,
                _ => 1,
            };
            std::process::exit(code);
        }
        ForkResult::Child => {
            child_main(args, self_bin);
        }
    }
}

fn child_main(
    args: Args,
    self_bin: Option<Vec<u8>>,
) -> ! {
    if let Err(e) = setup_mounts(&args) {
        die(&format!("ronly: mounts: {}", e));
    }

    if let Some(bin) = &self_bin {
        if let Err(e) = shims::install_shims(bin) {
            die(&format!("ronly: shims: {}", e));
        }

        // PATH: extra shims > built-in shims > system
        let sys_path =
            std::env::var("PATH").unwrap_or_default();
        let mut parts: Vec<String> = args
            .extra_shims
            .iter()
            .map(|d| d.to_string_lossy().into_owned())
            .collect();
        parts.push(shims::SHIMS_DIR.to_string());
        parts.push(sys_path);
        std::env::set_var("PATH", parts.join(":"));
    }

    if let Err(e) = setup_seccomp() {
        die(&format!("ronly: seccomp: {}", e));
    }

    // Determine shell
    let shell = args.shell.unwrap_or_else(|| {
        std::env::var("SHELL")
            .unwrap_or_else(|_| "/bin/bash".to_string())
    });

    // Exec
    if args.command.is_empty() {
        // Interactive shell
        let sh = CString::new(shell).unwrap();
        nix::unistd::execvp(&sh, &[&sh]).unwrap();
    } else {
        // Command mode: exec the command directly
        let argv: Vec<CString> = args
            .command
            .iter()
            .map(|s| CString::new(s.as_str()).unwrap())
            .collect();
        let argv_refs: Vec<&CString> =
            argv.iter().collect();
        nix::unistd::execvp(&argv[0], &argv_refs)
            .unwrap();
    }
    unreachable!()
}
