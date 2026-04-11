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
cmd.seal_fds()               // close all fds >= 3 at exec
   .keep_fd(child.into());   // except this one

cmd.arg("-c").arg("true");
cmd.status()?;
# Ok::<(), std::io::Error>(())
```

## API

- **`keep_fd(fd)`** — keep fd at the same number in the child.
- **`map_fd(fd, child_fd)`** — remap fd to `child_fd` in the child.
- **`seal_fds()`** — close all fds `>= 3` not registered via
  `keep_fd`/`map_fd`.

Call `seal_fds()` first, then `keep_fd()`/`map_fd()` — hooks run
in registration order.
