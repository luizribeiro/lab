# capsa-process

Extension trait for `std::process::Command` that provides fd inheritance
and sealing for child processes.

## Usage

```rust
use std::os::fd::OwnedFd;
use std::process::Command;
use capsa_process::CommandFdExt;

let (read_end, write_end) = std::io::pipe().unwrap();
let read_fd: OwnedFd = read_end.into();
let raw = std::os::fd::AsRawFd::as_raw_fd(&read_fd);

let mut cmd = Command::new("/usr/bin/my-daemon");
cmd.arg(format!("--control-fd={raw}"))
    .map_fd(read_fd, 10)   // remap to fd 10 in child
    .seal_fds();            // close all other fds >= 3

let child = cmd.spawn().unwrap();
```

### Methods

- **`map_fd(fd, child_fd)`** — map an owned fd to a specific number in the child
- **`keep_fd(fd)`** — keep an fd at its current number in the child
- **`seal_fds()`** — close all fds >= 3 not registered via `map_fd`/`keep_fd`
