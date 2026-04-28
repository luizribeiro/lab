# lockin-process

`Command` extensions for safe fd passing across `exec` on Unix.

## Why

Rust sets `FD_CLOEXEC` on new fds by default. This crate gives you
explicit control: keep or remap selected fds, then seal everything
else.

## Usage

```rust
use std::os::unix::net::UnixDatagram;
use std::process::Command;
use lockin_process::CommandFdExt;

let (_parent, child) = UnixDatagram::pair()?;

let mut cmd = Command::new("/bin/sh");
cmd.seal_fds()               // mark fds >= 3 FD_CLOEXEC; kernel closes them at exec
   .keep_fd(child.into());   // except this one

cmd.arg("-c").arg("true");
cmd.status()?;
# Ok::<(), std::io::Error>(())
```

## API

- **`keep_fd(fd)`** — keep fd at the same number in the child.
- **`map_fd(fd, child_fd)`** — remap fd to `child_fd` in the child.
- **`seal_fds()`** — mark fds `>= 3` `FD_CLOEXEC` so the kernel
  closes them on `execve`, except those re-allowed by a later
  `keep_fd`/`map_fd`. On Linux ≥ 5.11 the full range is covered in
  one child-side `close_range` syscall; on older Linux it falls
  back to a `fcntl` loop over `[3, min(RLIMIT_NOFILE, 65536)]`. On
  macOS, the parent enumerates currently-open fds at registration
  time via `proc_pidinfo(PROC_PIDLISTFDS)` and marks each one
  precisely (no numeric cap), and a defensive child-side `fcntl`
  sweep covers the residual race window up to fd 65,536.

Call `seal_fds()` first, then `keep_fd()`/`map_fd()` — hooks run
in registration order.
