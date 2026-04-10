# capsa-process

Extension trait for `std::process::Command` that provides fd
inheritance and sealing for child processes.

When you `fork+exec` in Rust, fds are marked `FD_CLOEXEC` by default
and get closed at exec. This crate lets you explicitly pass specific
fds to the child while sealing everything else.

## Usage

```rust,no_run
use std::os::unix::net::UnixDatagram;
use std::process::Command;
use capsa_process::CommandFdExt;

let (parent_sock, child_sock) = UnixDatagram::pair()?;

let mut cmd = Command::new("/usr/bin/my-daemon");
cmd.seal_fds()                      // close all fds >= 3 at exec
   .keep_fd(child_sock.into());     // except this one

cmd.spawn()?;
# Ok::<(), std::io::Error>(())
```

## Methods

- **`map_fd(fd, child_fd)`** — remap an fd to a specific number in the child
- **`keep_fd(fd)`** — keep an fd at its current number in the child
- **`seal_fds()`** — set `FD_CLOEXEC` on all fds `>= 3` not registered
  via `map_fd`/`keep_fd`

Call `seal_fds()` before `map_fd`/`keep_fd` — the hooks run in
registration order, so sealing first then keeping specific fds
composes correctly.
