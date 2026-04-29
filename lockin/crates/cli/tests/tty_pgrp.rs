//! Regression test: lockin must transfer the controlling TTY's
//! foreground process group to the sandboxed child's pgrp. Without
//! this, an interactive TUI program (claude, htop, vim, etc.) gets
//! SIGTTOU on its first tcsetattr/tcgetwinsz and is suspended in
//! state T immediately after spawn. We verify by polling
//! `ps -o tpgid` on lockin's pid; a successful transfer shows
//! `tpgid != lockin_pid`.

use std::ffi::CString;
use std::os::fd::{FromRawFd, OwnedFd};
use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant};

fn lockin_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_lockin"))
}

fn probe_binary() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("../../target/debug/sandbox_probe");
    assert!(
        path.exists(),
        "sandbox_probe not found — run `cargo build` first"
    );
    path.canonicalize().unwrap()
}

fn write_config() -> tempfile::NamedTempFile {
    let tmp = tempfile::Builder::new().suffix(".toml").tempfile().unwrap();
    let suffix = match std::env::var_os("LOCKIN_TEST_EXEC_DIRS") {
        Some(val) => {
            let dirs: Vec<String> = std::env::split_paths(&val)
                .filter(|p| !p.as_os_str().is_empty() && p.is_absolute())
                .map(|p| {
                    let s = p.to_string_lossy();
                    let escaped = s.replace('\\', "\\\\").replace('"', "\\\"");
                    format!("\"{escaped}\"")
                })
                .collect();
            if dirs.is_empty() {
                String::new()
            } else {
                format!("[filesystem]\nexec_dirs = [{}]\n", dirs.join(", "))
            }
        }
        None => String::new(),
    };
    std::fs::write(tmp.path(), suffix).unwrap();
    tmp
}

#[test]
fn fg_pgrp_is_transferred_to_sandbox_child_under_pty() {
    let config = write_config();
    let lockin = lockin_binary();
    let probe = probe_binary();

    let args: Vec<CString> = [
        lockin.as_os_str(),
        "-c".as_ref(),
        config.path().as_os_str(),
        "--".as_ref(),
        probe.as_os_str(),
        "pause".as_ref(),
        "30".as_ref(),
    ]
    .iter()
    .map(|s| CString::new(s.as_bytes()).unwrap())
    .collect();
    let mut argv: Vec<*const libc::c_char> = args.iter().map(|c| c.as_ptr()).collect();
    argv.push(std::ptr::null());

    let mut master_fd: libc::c_int = 0;
    let pid = unsafe {
        libc::forkpty(
            &mut master_fd,
            std::ptr::null_mut(),
            std::ptr::null_mut() as _,
            std::ptr::null_mut() as _,
        )
    };
    if pid < 0 {
        panic!("forkpty: {}", std::io::Error::last_os_error());
    }
    if pid == 0 {
        unsafe {
            libc::execv(argv[0], argv.as_ptr());
            libc::_exit(127);
        }
    }

    let _master = unsafe { OwnedFd::from_raw_fd(master_fd) };

    // Poll lockin's tpgid (the foreground pgrp of its controlling tty)
    // via `ps`. Forkpty makes lockin a session leader, so initially
    // tpgid == lockin pid. With the fix, lockin tcsetpgrp's the slave
    // to the sandboxed child's pgrp, so tpgid diverges from lockin_pid.
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut observed_tpgid: i32 = -1;
    let mut transferred = false;
    while Instant::now() < deadline {
        if let Some(tpgid) = read_tpgid(pid) {
            observed_tpgid = tpgid;
            if tpgid > 0 && tpgid != pid {
                transferred = true;
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(50));
    }

    let outcome_msg = format!("lockin_pid={pid}, last observed tty fg pgrp={observed_tpgid}");

    unsafe {
        libc::kill(pid, libc::SIGTERM);
    }
    let mut status: libc::c_int = 0;
    let exit_deadline = Instant::now() + Duration::from_secs(10);
    let exited = loop {
        let r = unsafe { libc::waitpid(pid, &mut status, libc::WNOHANG) };
        if r == pid {
            break true;
        }
        if Instant::now() >= exit_deadline {
            break false;
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    if !exited {
        unsafe {
            libc::kill(pid, libc::SIGKILL);
            libc::waitpid(pid, &mut status, 0);
        }
        panic!("lockin under pty did not exit within 10s of SIGTERM; {outcome_msg}");
    }

    assert!(
        transferred,
        "expected lockin to transfer the controlling tty foreground pgrp \
         away from itself to the sandboxed child's pgrp; {outcome_msg}"
    );
}

fn read_tpgid(pid: libc::pid_t) -> Option<i32> {
    let out = Command::new("ps")
        .args(["-o", "tpgid=", "-p", &pid.to_string()])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    std::str::from_utf8(&out.stdout)
        .ok()?
        .trim()
        .parse::<i32>()
        .ok()
}
