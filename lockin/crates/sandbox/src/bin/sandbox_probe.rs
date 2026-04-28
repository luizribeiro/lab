use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream, UdpSocket};
use std::os::fd::{FromRawFd, OwnedFd, RawFd};
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::net::{UnixDatagram, UnixStream};
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
        "can-readdir" => {
            let Some(path) = args.next() else {
                usage_and_exit();
            };
            can_readdir(Path::new(&path))
        }
        "can-mkdir" => {
            let Some(path) = args.next() else {
                usage_and_exit();
            };
            can_mkdir(Path::new(&path))
        }
        "can-truncate" => {
            let Some(path) = args.next() else {
                usage_and_exit();
            };
            can_truncate(Path::new(&path))
        }
        "can-utime" => {
            let Some(path) = args.next() else {
                usage_and_exit();
            };
            can_utime(Path::new(&path))
        }
        "can-rmdir" => {
            let Some(path) = args.next() else {
                usage_and_exit();
            };
            can_rmdir(Path::new(&path))
        }
        "can-rename" => {
            let Some(from) = args.next() else {
                usage_and_exit();
            };
            let Some(to) = args.next() else {
                usage_and_exit();
            };
            can_rename(Path::new(&from), Path::new(&to))
        }
        "can-unlink" => {
            let Some(path) = args.next() else {
                usage_and_exit();
            };
            can_unlink(Path::new(&path))
        }
        "can-chmod" => {
            let Some(path) = args.next() else {
                usage_and_exit();
            };
            let Some(mode_str) = args.next() else {
                usage_and_exit();
            };
            let Ok(mode) = u32::from_str_radix(mode_str.trim_start_matches("0o"), 8) else {
                usage_and_exit();
            };
            can_chmod(Path::new(&path), mode)
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
        "can-tcp-listen" => {
            let Some(host) = args.next() else {
                usage_and_exit();
            };
            let Some(port) = args.next() else {
                usage_and_exit();
            };
            can_tcp_listen(&host, &port)
        }
        "can-udp-send" => {
            let Some(host) = args.next() else {
                usage_and_exit();
            };
            let Some(port) = args.next() else {
                usage_and_exit();
            };
            can_udp_send(&host, &port)
        }
        "can-unix-stream-connect" => {
            let Some(path) = args.next() else {
                usage_and_exit();
            };
            can_unix_stream_connect(Path::new(&path))
        }
        "can-unix-dgram-connect" => {
            let Some(path) = args.next() else {
                usage_and_exit();
            };
            can_unix_dgram_connect(Path::new(&path))
        }
        #[cfg(target_os = "macos")]
        "can-mach-lookup" => {
            let Some(name) = args.next() else {
                usage_and_exit();
            };
            mach::can_mach_lookup(&name)
        }
        #[cfg(target_os = "macos")]
        "can-mach-register" => {
            let Some(name) = args.next() else {
                usage_and_exit();
            };
            mach::can_mach_register(&name)
        }
        #[cfg(target_os = "macos")]
        "can-xpc-lookup" => {
            let Some(name) = args.next() else {
                usage_and_exit();
            };
            mach::can_xpc_lookup(&name)
        }
        "print-env" => {
            let Some(name) = args.next() else {
                usage_and_exit();
            };
            print_env(&name)
        }
        "env-var-unset" => {
            let Some(name) = args.next() else {
                usage_and_exit();
            };
            env_var_unset(&name)
        }
        "can-proxy-connect" => {
            let Some(target) = args.next() else {
                usage_and_exit();
            };
            can_proxy_connect(&target)
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
        "pause" => {
            let secs: u64 = args.next().and_then(|s| s.parse().ok()).unwrap_or(60);
            std::thread::sleep(std::time::Duration::from_secs(secs));
            Ok(())
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
    file.write_all(b"lockin-probe\n")
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn can_stat(path: &Path) -> Result<(), String> {
    std::fs::metadata(path).map_err(|e| e.to_string())?;
    Ok(())
}

fn can_readdir(path: &Path) -> Result<(), String> {
    let entries = std::fs::read_dir(path).map_err(|e| e.to_string())?;
    for entry in entries {
        entry.map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn can_mkdir(path: &Path) -> Result<(), String> {
    std::fs::create_dir(path).map_err(|e| e.to_string())
}

fn can_truncate(path: &Path) -> Result<(), String> {
    OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(path)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

fn can_utime(path: &Path) -> Result<(), String> {
    use std::time::{Duration, SystemTime};
    let file = std::fs::File::options()
        .write(true)
        .open(path)
        .map_err(|e| e.to_string())?;
    let when = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000);
    file.set_times(
        std::fs::FileTimes::new()
            .set_accessed(when)
            .set_modified(when),
    )
    .map_err(|e| e.to_string())
}

fn can_rmdir(path: &Path) -> Result<(), String> {
    std::fs::remove_dir(path).map_err(|e| e.to_string())
}

fn can_rename(from: &Path, to: &Path) -> Result<(), String> {
    std::fs::rename(from, to).map_err(|e| e.to_string())
}

fn can_unlink(path: &Path) -> Result<(), String> {
    std::fs::remove_file(path).map_err(|e| e.to_string())
}

fn can_chmod(path: &Path, mode: u32) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).map_err(|e| e.to_string())
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

fn can_tcp_listen(host: &str, port: &str) -> Result<(), String> {
    let port: u16 = port
        .parse()
        .map_err(|e: std::num::ParseIntError| e.to_string())?;
    TcpListener::bind((host, port)).map_err(|e| format!("bind: {e}"))?;
    Ok(())
}

fn can_udp_send(host: &str, port: &str) -> Result<(), String> {
    let port: u16 = port
        .parse()
        .map_err(|e: std::num::ParseIntError| e.to_string())?;
    let sock = UdpSocket::bind("127.0.0.1:0").map_err(|e| format!("bind: {e}"))?;
    sock.send_to(b"x", (host, port))
        .map_err(|e| format!("sendto: {e}"))?;
    Ok(())
}

fn can_unix_stream_connect(path: &Path) -> Result<(), String> {
    UnixStream::connect(path).map_err(|e| format!("connect {}: {e}", path.display()))?;
    Ok(())
}

fn can_unix_dgram_connect(path: &Path) -> Result<(), String> {
    let sock = UnixDatagram::unbound().map_err(|e| format!("unbound: {e}"))?;
    sock.connect(path)
        .map_err(|e| format!("connect {}: {e}", path.display()))?;
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

fn print_env(name: &str) -> Result<(), String> {
    let value = std::env::var(name).map_err(|e| format!("env var `{name}`: {e}"))?;
    println!("{value}");
    Ok(())
}

fn env_var_unset(name: &str) -> Result<(), String> {
    match std::env::var_os(name) {
        Some(value) => Err(format!("env var `{name}` is set (len={})", value.len())),
        None => Ok(()),
    }
}

/// Reads `HTTP_PROXY` from env, opens a TCP connection to that
/// proxy, sends a CONNECT request for `target`, expects HTTP/1.1 200,
/// then writes and reads back a handshake byte over the tunnel to
/// verify bidirectional forwarding. Used by lockin's end-to-end
/// proxy-mode integration test to prove the full chain (env
/// injection + sandbox rules + proxy tunneling) works for an
/// allowlisted host.
fn can_proxy_connect(target: &str) -> Result<(), String> {
    let proxy_url =
        std::env::var("HTTP_PROXY").map_err(|e| format!("env var `HTTP_PROXY`: {e}"))?;
    let proxy_addr = proxy_url
        .strip_prefix("http://")
        .ok_or_else(|| format!("proxy url `{proxy_url}` must start with http://"))?;
    let mut stream =
        TcpStream::connect(proxy_addr).map_err(|e| format!("connect proxy `{proxy_addr}`: {e}"))?;

    let request = format!("CONNECT {target} HTTP/1.1\r\nHost: {target}\r\n\r\n");
    stream
        .write_all(request.as_bytes())
        .map_err(|e| format!("write CONNECT: {e}"))?;
    stream.flush().map_err(|e| format!("flush: {e}"))?;

    let mut head = Vec::new();
    let mut byte = [0u8; 1];
    loop {
        let n = stream.read(&mut byte).map_err(|e| format!("read: {e}"))?;
        if n == 0 {
            return Err("proxy closed before response completed".into());
        }
        head.push(byte[0]);
        if head.ends_with(b"\r\n\r\n") {
            break;
        }
        if head.len() > 4096 {
            return Err("response head too long".into());
        }
    }
    let text = String::from_utf8(head).map_err(|e| format!("response not utf-8: {e}"))?;
    if !text.starts_with("HTTP/1.1 200 ") {
        return Err(format!(
            "CONNECT rejected: {}",
            text.lines().next().unwrap_or("")
        ));
    }

    stream
        .write_all(b"P")
        .map_err(|e| format!("write handshake: {e}"))?;
    let mut echoed = [0u8; 1];
    stream
        .read_exact(&mut echoed)
        .map_err(|e| format!("read echoed handshake: {e}"))?;
    if echoed != *b"P" {
        return Err(format!("echo mismatch: expected 'P', got {echoed:?}"));
    }
    Ok(())
}

fn can_write_temp() -> Result<(), String> {
    let mut path = effective_temp_dir();
    path.push(format!(
        "lockin-probe-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));

    let mut options = OpenOptions::new();
    options.create_new(true).write(true).mode(0o600);

    let mut file = options.open(&path).map_err(|e| e.to_string())?;

    file.write_all(b"lockin-probe-temp\n")
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
  can-readdir <path>\n\
  can-mkdir <path>\n\
  can-rmdir <path>\n\
  can-utime <path>\n\
  can-truncate <path>\n\
  can-rename <from> <to>\n\
  can-unlink <path>\n\
  can-chmod <path> <octal-mode>\n\
  can-exec <path> [args...]\n\
  can-connect <host> <port>\n\
  can-tcp-listen <host> <port>\n\
  can-udp-send <host> <port>\n\
  can-unix-stream-connect <path>\n\
  can-unix-dgram-connect <path>\n\
  print-env <var-name>\n\
  env-var-unset <var-name>\n\
  can-proxy-connect <host:port>  (reads HTTP_PROXY from env)\n\
  can-write-temp\n\
  fd-read-byte <fd> <expected-byte> [<fd> <expected-byte>...]\n\
  fd-write-byte <fd> <byte> [<fd> <byte>...]\n\
  check-rlimit <resource-name> <expected-value>\n\
  open-many-fds <count>\n\
  pause [seconds]\n\
  check-no-new-privs\n\
  check-has-cap <cap-number>\n\
  check-has-effective-cap <cap-number>\n\
  can-mach-lookup <service-name>           (macOS only)\n\
  can-mach-register <service-name>         (macOS only)\n\
  can-xpc-lookup <service-name>            (macOS only)"
    );
    std::process::exit(2);
}

#[cfg(target_os = "macos")]
mod mach {
    use std::ffi::{c_char, c_void, CString};
    use std::ptr;
    use std::sync::OnceLock;

    type KernReturnT = i32;
    type MachPortT = u32;
    type NameT = *const c_char;

    const KERN_SUCCESS: KernReturnT = 0;
    const MACH_PORT_NULL: MachPortT = 0;
    const MACH_PORT_RIGHT_RECEIVE: u32 = 1;
    const MACH_MSG_TYPE_MAKE_SEND: u32 = 20;
    const BLOCK_IS_GLOBAL: i32 = 1 << 28;

    type XpcObject = *mut c_void;
    type XpcConnection = *mut c_void;

    extern "C" {
        static bootstrap_port: MachPortT;
        fn bootstrap_look_up(bp: MachPortT, name: NameT, sp: *mut MachPortT) -> KernReturnT;
        fn bootstrap_register(bp: MachPortT, name: NameT, sp: MachPortT) -> KernReturnT;
        static mach_task_self_: MachPortT;
        fn mach_port_allocate(task: MachPortT, right: u32, name: *mut MachPortT) -> KernReturnT;
        fn mach_port_insert_right(
            task: MachPortT,
            name: MachPortT,
            poly: MachPortT,
            type_: u32,
        ) -> KernReturnT;

        fn xpc_connection_create_mach_service(
            name: *const c_char,
            queue: *mut c_void,
            flags: u64,
        ) -> XpcConnection;
        fn xpc_connection_set_event_handler(conn: XpcConnection, handler: *mut c_void);
        fn xpc_connection_resume(conn: XpcConnection);
        fn xpc_connection_cancel(conn: XpcConnection);
        fn xpc_release(obj: XpcObject);
        fn xpc_dictionary_create(
            keys: *const *const c_char,
            values: *const XpcObject,
            count: usize,
        ) -> XpcObject;
        fn xpc_connection_send_message_with_reply_sync(
            conn: XpcConnection,
            msg: XpcObject,
        ) -> XpcObject;
        fn xpc_get_type(obj: XpcObject) -> *const c_void;
        static _xpc_type_error: c_void;
        static _NSConcreteGlobalBlock: c_void;
    }

    #[repr(C)]
    struct Block {
        isa: *const c_void,
        flags: i32,
        reserved: i32,
        invoke: extern "C" fn(*mut Block, XpcObject),
        descriptor: *const BlockDesc,
    }

    #[repr(C)]
    struct BlockDesc {
        reserved: u64,
        size: u64,
    }

    static DESC: BlockDesc = BlockDesc {
        reserved: 0,
        size: std::mem::size_of::<Block>() as u64,
    };

    extern "C" fn noop_block(_blk: *mut Block, _obj: XpcObject) {}

    fn noop_handler() -> *mut c_void {
        static BLK: OnceLock<usize> = OnceLock::new();
        let addr = *BLK.get_or_init(|| {
            let blk = Box::leak(Box::new(Block {
                isa: unsafe { &_NSConcreteGlobalBlock as *const _ },
                flags: BLOCK_IS_GLOBAL,
                reserved: 0,
                invoke: noop_block,
                descriptor: &DESC,
            }));
            blk as *mut Block as usize
        });
        addr as *mut c_void
    }

    pub fn can_mach_lookup(name: &str) -> Result<(), String> {
        let cname = CString::new(name).map_err(|e| e.to_string())?;
        let mut port: MachPortT = MACH_PORT_NULL;
        let kr = unsafe { bootstrap_look_up(bootstrap_port, cname.as_ptr(), &mut port) };
        if kr == KERN_SUCCESS && port != MACH_PORT_NULL {
            Ok(())
        } else {
            Err(format!("bootstrap_look_up `{name}` kr={kr} port={port}"))
        }
    }

    pub fn can_mach_register(name: &str) -> Result<(), String> {
        let cname = CString::new(name).map_err(|e| e.to_string())?;
        let mut port: MachPortT = MACH_PORT_NULL;
        let task = unsafe { mach_task_self_ };
        let kr = unsafe { mach_port_allocate(task, MACH_PORT_RIGHT_RECEIVE, &mut port) };
        if kr != KERN_SUCCESS {
            return Err(format!("mach_port_allocate kr={kr}"));
        }
        let kr = unsafe { mach_port_insert_right(task, port, port, MACH_MSG_TYPE_MAKE_SEND) };
        if kr != KERN_SUCCESS {
            return Err(format!("mach_port_insert_right kr={kr}"));
        }
        let kr = unsafe { bootstrap_register(bootstrap_port, cname.as_ptr(), port) };
        if kr == KERN_SUCCESS {
            Ok(())
        } else {
            Err(format!("bootstrap_register `{name}` kr={kr}"))
        }
    }

    pub fn can_xpc_lookup(name: &str) -> Result<(), String> {
        let cname = CString::new(name).map_err(|e| e.to_string())?;
        let conn =
            unsafe { xpc_connection_create_mach_service(cname.as_ptr(), ptr::null_mut(), 0) };
        if conn.is_null() {
            return Err(format!(
                "xpc_connection_create_mach_service `{name}` -> null"
            ));
        }
        unsafe {
            xpc_connection_set_event_handler(conn, noop_handler());
            xpc_connection_resume(conn);
        }

        let msg = unsafe { xpc_dictionary_create(ptr::null(), ptr::null(), 0) };
        let reply = unsafe { xpc_connection_send_message_with_reply_sync(conn, msg) };
        unsafe { xpc_release(msg) };
        let result = if reply.is_null() {
            Err(format!("xpc reply null for `{name}`"))
        } else {
            let ty = unsafe { xpc_get_type(reply) };
            let err_ty = unsafe { &_xpc_type_error as *const _ };
            if ty == err_ty {
                Err(format!("xpc reply was XPC_TYPE_ERROR for `{name}`"))
            } else {
                Ok(())
            }
        };
        if !reply.is_null() {
            unsafe { xpc_release(reply) };
        }
        unsafe {
            xpc_connection_cancel(conn);
            xpc_release(conn);
        }
        result
    }
}
