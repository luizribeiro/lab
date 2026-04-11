use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::os::fd::{FromRawFd, OwnedFd, RawFd};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    let mut args = std::env::args().skip(1);
    let Some(action) = args.next() else {
        usage_and_exit();
    };

    let result = match action.as_str() {
        "can-read" => {
            let Some(path) = args.next() else {
                usage_and_exit();
            };
            can_read(Path::new(&path))
        }
        "can-write" => {
            let Some(path) = args.next() else {
                usage_and_exit();
            };
            can_write(Path::new(&path))
        }
        "can-stat" => {
            let Some(path) = args.next() else {
                usage_and_exit();
            };
            can_stat(Path::new(&path))
        }
        "can-exec" => {
            let Some(path) = args.next() else {
                usage_and_exit();
            };
            let rest: Vec<String> = args.collect();
            can_exec(Path::new(&path), &rest)
        }
        "can-connect" => {
            let Some(host) = args.next() else {
                usage_and_exit();
            };
            let Some(port) = args.next() else {
                usage_and_exit();
            };
            can_connect(&host, &port)
        }
        "can-write-temp" => can_write_temp(),
        "fd-read-byte" => {
            let pairs: Vec<String> = args.collect();
            if pairs.is_empty() || !pairs.len().is_multiple_of(2) {
                usage_and_exit();
            }
            fd_read_bytes(&pairs)
        }
        "fd-write-byte" => {
            let pairs: Vec<String> = args.collect();
            if pairs.is_empty() || !pairs.len().is_multiple_of(2) {
                usage_and_exit();
            }
            fd_write_bytes(&pairs)
        }
        "check-no-new-privs" => check_no_new_privs(),
        "check-has-cap" => {
            let Some(cap_str) = args.next() else {
                usage_and_exit();
            };
            check_has_cap(&cap_str)
        }
        "check-has-effective-cap" => {
            let Some(cap_str) = args.next() else {
                usage_and_exit();
            };
            check_has_effective_cap(&cap_str)
        }
        "check-rlimit" => {
            let Some(resource_name) = args.next() else {
                usage_and_exit();
            };
            let Some(expected_str) = args.next() else {
                usage_and_exit();
            };
            check_rlimit(&resource_name, &expected_str)
        }
        "open-many-fds" => {
            let Some(count_str) = args.next() else {
                usage_and_exit();
            };
            let count: usize = count_str.parse().unwrap_or_else(|e| {
                eprintln!("invalid count `{count_str}`: {e}");
                std::process::exit(2);
            });
            open_many_fds(count)
        }
        _ => {
            usage_and_exit();
        }
    };

    if let Err(err) = result {
        eprintln!("sandbox-probe action `{action}` failed: {err}");
        std::process::exit(1);
    }
}

fn can_read(path: &Path) -> Result<(), String> {
    let mut file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut buf = Vec::new();
    file.read_to_end(&mut buf).map_err(|e| e.to_string())?;
    Ok(())
}

fn can_write(path: &Path) -> Result<(), String> {
    let mut file = OpenOptions::new()
        .write(true)
        .open(path)
        .map_err(|e| e.to_string())?;
    file.write_all(b"capsa-sandbox-probe\n")
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn can_stat(path: &Path) -> Result<(), String> {
    std::fs::metadata(path).map_err(|e| e.to_string())?;
    Ok(())
}

fn can_exec(path: &Path, args: &[String]) -> Result<(), String> {
    let status = Command::new(path)
        .args(args)
        .status()
        .map_err(|e| e.to_string())?;

    if status.success() {
        Ok(())
    } else {
        Err(format!("command exited with status {status}"))
    }
}

fn can_connect(host: &str, port: &str) -> Result<(), String> {
    let port: u16 = port
        .parse()
        .map_err(|e: std::num::ParseIntError| e.to_string())?;
    TcpStream::connect((host, port)).map_err(|e| e.to_string())?;
    Ok(())
}

fn fd_read_bytes(pairs: &[String]) -> Result<(), String> {
    for pair in pairs.chunks_exact(2) {
        let (mut reader, raw, expected) = open_marked_fd(&pair[0], &pair[1])?;
        let mut buf = [0u8; 1];
        reader
            .read_exact(&mut buf)
            .map_err(|e| format!("read from fd {raw} failed: {e}"))?;

        if buf[0] != expected {
            return Err(format!(
                "fd {raw}: expected byte 0x{expected:02x}, got 0x{:02x}",
                buf[0]
            ));
        }
    }
    Ok(())
}

fn fd_write_bytes(pairs: &[String]) -> Result<(), String> {
    for pair in pairs.chunks_exact(2) {
        let (mut writer, raw, marker) = open_marked_fd(&pair[0], &pair[1])?;
        writer
            .write_all(&[marker])
            .map_err(|e| format!("write to fd {raw} failed: {e}"))?;
    }
    Ok(())
}

/// Parses a `(fd, single-byte-marker)` CLI argument pair, validates
/// that the fd is still open via `F_GETFD`, and returns a `File` that
/// owns the fd plus the marker byte. Shared by `fd-read-byte` and
/// `fd-write-byte`.
fn open_marked_fd(fd_arg: &str, marker_arg: &str) -> Result<(std::fs::File, RawFd, u8), String> {
    let raw: RawFd = fd_arg
        .parse()
        .map_err(|e: std::num::ParseIntError| format!("invalid fd `{fd_arg}`: {e}"))?;
    let marker_bytes = marker_arg.as_bytes();
    if marker_bytes.len() != 1 {
        return Err(format!(
            "fd {raw}: expected a single-byte marker, got {} bytes",
            marker_bytes.len()
        ));
    }

    // Probe with F_GETFD first so a wrapper that closed the fd is
    // reported as EBADF instead of being masked as a read/write error.
    // SAFETY: F_GETFD on an integer fd is a read-only query; no UB
    // for any value.
    if unsafe { libc::fcntl(raw, libc::F_GETFD) } == -1 {
        return Err(format!(
            "fcntl(F_GETFD) on fd {raw} failed: {}",
            std::io::Error::last_os_error()
        ));
    }

    // SAFETY: F_GETFD above proved fd `raw` is open. This probe is
    // single-threaded and each pair's `raw` is distinct (the builder
    // rejects duplicates), so no other owner aliases it.
    let owned = unsafe { OwnedFd::from_raw_fd(raw) };
    Ok((std::fs::File::from(owned), raw, marker_bytes[0]))
}

fn can_write_temp() -> Result<(), String> {
    let mut path = effective_temp_dir();
    path.push(format!(
        "capsa-sandbox-probe-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));

    let mut options = OpenOptions::new();
    options.create_new(true).write(true).mode(0o600);

    let mut file = options.open(&path).map_err(|e| e.to_string())?;

    file.write_all(b"capsa-sandbox-probe-temp\n")
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn effective_temp_dir() -> PathBuf {
    // Some sandbox/runtime combinations may clear TMPDIR while leaving TMP/TEMP.
    ["TMPDIR", "TMP", "TEMP"]
        .iter()
        .filter_map(std::env::var_os)
        .find(|val| !val.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
}

fn check_no_new_privs() -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        // SAFETY: prctl(PR_GET_NO_NEW_PRIVS) is a read-only query.
        let val = unsafe { libc::prctl(libc::PR_GET_NO_NEW_PRIVS, 0, 0, 0, 0) };
        if val == 1 {
            Ok(())
        } else if val == 0 {
            Err("NoNewPrivs is not set".to_string())
        } else {
            Err(format!(
                "prctl(PR_GET_NO_NEW_PRIVS) failed: {}",
                std::io::Error::last_os_error()
            ))
        }
    }
    #[cfg(not(target_os = "linux"))]
    Err("check-no-new-privs is only supported on Linux".to_string())
}

fn check_has_cap(cap_str: &str) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        let cap: u32 = cap_str
            .parse()
            .map_err(|e: std::num::ParseIntError| format!("invalid cap number `{cap_str}`: {e}"))?;
        // SAFETY: prctl(PR_CAPBSET_READ) is a read-only query.
        let val = unsafe { libc::prctl(libc::PR_CAPBSET_READ, cap, 0, 0, 0) };
        if val == 1 {
            Ok(())
        } else if val == 0 {
            Err(format!("capability {cap} is NOT in the bounding set"))
        } else {
            Err(format!(
                "prctl(PR_CAPBSET_READ, {cap}) failed: {}",
                std::io::Error::last_os_error()
            ))
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = cap_str;
        Err("check-has-cap is only supported on Linux".to_string())
    }
}

fn check_has_effective_cap(cap_str: &str) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        let cap: u32 = cap_str
            .parse()
            .map_err(|e: std::num::ParseIntError| format!("invalid cap number `{cap_str}`: {e}"))?;

        #[repr(C)]
        struct CapHeader {
            version: u32,
            pid: i32,
        }
        #[repr(C)]
        struct CapData {
            effective: u32,
            permitted: u32,
            inheritable: u32,
        }
        let header = CapHeader {
            version: 0x20080522_u32,
            pid: 0,
        };
        let mut data = [
            CapData {
                effective: 0,
                permitted: 0,
                inheritable: 0,
            },
            CapData {
                effective: 0,
                permitted: 0,
                inheritable: 0,
            },
        ];
        // SAFETY: SYS_capget is a direct syscall; the structs are stack-local.
        let rc =
            unsafe { libc::syscall(libc::SYS_capget, &header as *const _, &mut data as *mut _) };
        if rc == -1 {
            return Err(format!(
                "capget failed: {}",
                std::io::Error::last_os_error()
            ));
        }

        let (word, bit) = if cap < 32 {
            (data[0].effective, 1u32 << cap)
        } else {
            (data[1].effective, 1u32 << (cap - 32))
        };

        if word & bit != 0 {
            Ok(())
        } else {
            Err(format!("capability {cap} is NOT in the effective set"))
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = cap_str;
        Err("check-has-effective-cap is only supported on Linux".to_string())
    }
}

#[allow(clippy::unnecessary_cast)]
fn check_rlimit(resource_name: &str, expected_str: &str) -> Result<(), String> {
    let resource = match resource_name {
        "nofile" => libc::RLIMIT_NOFILE,
        "as" => libc::RLIMIT_AS,
        "cpu" => libc::RLIMIT_CPU,
        "core" => libc::RLIMIT_CORE,
        "nproc" => libc::RLIMIT_NPROC,
        _ => return Err(format!("unknown rlimit resource `{resource_name}`")),
    };
    let expected: u64 = expected_str.parse().map_err(|e: std::num::ParseIntError| {
        format!("invalid expected value `{expected_str}`: {e}")
    })?;

    let mut rlim = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    // SAFETY: getrlimit is a direct syscall with a stack-local struct.
    let rc = unsafe { libc::getrlimit(resource as _, &mut rlim) };
    if rc == -1 {
        return Err(format!(
            "getrlimit({resource_name}) failed: {}",
            std::io::Error::last_os_error()
        ));
    }

    if rlim.rlim_cur != expected as libc::rlim_t {
        return Err(format!(
            "rlimit {resource_name}: expected rlim_cur={expected}, got {}",
            rlim.rlim_cur
        ));
    }
    Ok(())
}

fn open_many_fds(count: usize) -> Result<(), String> {
    let mut opened = Vec::new();
    for i in 0..count {
        match std::fs::File::open("/dev/null") {
            Ok(f) => opened.push(f),
            Err(e) => {
                return Err(format!("failed to open fd #{i}: {e}"));
            }
        }
    }
    Ok(())
}

fn usage_and_exit() -> ! {
    eprintln!(
        "usage: sandbox_probe <action> [args...]\n\
actions:\n\
  can-read <path>\n\
  can-write <path>\n\
  can-stat <path>\n\
  can-exec <path> [args...]\n\
  can-connect <host> <port>\n\
  can-write-temp\n\
  fd-read-byte <fd> <expected-byte> [<fd> <expected-byte>...]\n\
  fd-write-byte <fd> <byte> [<fd> <byte>...]\n\
  check-rlimit <resource-name> <expected-value>\n\
  open-many-fds <count>\n\
  check-no-new-privs\n\
  check-has-cap <cap-number>\n\
  check-has-effective-cap <cap-number>"
    );
    std::process::exit(2);
}
