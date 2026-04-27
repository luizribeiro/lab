# macOS Seatbelt Baseline Audit (`system.sb`)

Reference inventory of Apple's `system.sb` profile, which `lockin` imports unconditionally at `crates/sandbox/src/darwin/policy.rs:16` (`policy.import_system()`). Every `lockin`-generated profile is therefore `(deny default)` plus this baseline plus `lockin`'s structured allows. This document captures what the baseline grants on its own so `lockin`'s contract can be precise about what its rules add.

## 1. Audit metadata

| Field | Value |
|-------|-------|
| Audit date | 2026-04-27 |
| Host | `Darwin ferrum 25.3.0 Darwin Kernel Version 25.3.0: Wed Jan 28 20:47:03 PST 2026; root:xnu-12377.81.4~5/RELEASE_ARM64_T6031 arm64` |
| `sw_vers` | macOS 26.3, build 25D125 |
| Architecture | arm64 (Apple silicon) |

| File | Bytes | Lines | SHA-256 |
|------|-------|-------|---------|
| `/System/Library/Sandbox/Profiles/system.sb` | 11847 | 305 | `8e6c396a0a4a6db758b49104e045d39a4af0ca28c61300a683f4de88c393e7f6` |
| `/System/Library/Sandbox/Profiles/dyld-support.sb` | 2655 | 69 | `06215a5d32689aefe395c29710e182eb54ba22162f50df8b4842290f8a19bf1c` |

> Apple caveat (verbatim from both files): "The sandbox rules in this file currently constitute Apple System Private Interface and are subject to change at any time and without notice. The contents of this file are also auto-generated and not user editable; it may be overwritten at any time."
> Implication: every grant here is subject to silent drift between OS releases (and even point releases). Re-run this audit on each macOS update we support; CI should hash-check the two files and fail loudly if they change.

## 2. Import graph

```
system.sb
  └── (import "dyld-support.sb")        ; system.sb:12
```

`dyld-support.sb` contains no further `(import ...)` directives. The full transitive set of Apple-shipped profile content imported by `lockin` is exactly these two files.

`lockin`'s renderer (`crates/sandbox/src/darwin/seatbelt.rs:87-92`) only emits `(import "system.sb")`; it never imports `dyld-support.sb` directly — it transitively gets it via `system.sb`.

## 3. Baseline grants — categorized inventory

Citations are `system.sb:LINE` unless prefixed `dyld-support.sb:LINE`.

### 3.1 Filesystem reads (full `file-read*`)

| Target | Form | Citation |
|--------|------|----------|
| `/Library/Apple` | subpath | system.sb:33 |
| `/Library/Filesystems/NetFSPlugins` | subpath | system.sb:34 |
| `/Library/Preferences/Logging` | subpath | system.sb:35 |
| `/System` | subpath | system.sb:36 |
| `/private/var/db/DarwinDirectory/local/recordStore.data` | literal | system.sb:37 |
| `/private/var/db/timezone` | subpath | system.sb:38 |
| `/usr/lib` | subpath | system.sb:39 |
| `/usr/share` | subpath | system.sb:40 |
| `/` | literal (root dir, for cwd inheritance) | system.sb:96 |
| `/dev/autofs_nowait` | literal | system.sb:103 |
| `/dev/random` | literal | system.sb:104 |
| `/dev/urandom` | literal | system.sb:105 |
| `/private/etc/master.passwd` | literal | system.sb:106 |
| `/private/etc/passwd` | literal | system.sb:107 |
| `/private/etc/protocols` | literal | system.sb:108 |
| `/private/etc/services` | literal | system.sb:109 |
| `/dev/null`, `/dev/zero` | literal (also `file-write-data`) | system.sb:111-113 |
| `/dev/dtracehelper` | literal (also `file-write-data`, `file-ioctl`) | system.sb:120-121 |
| `/private/var/db/eligibilityd/eligibility.plist` | literal | system.sb:124-125 |
| `/Library/Apple/usr/libexec/oah/libRosettaRuntime` | literal (metadata only) | system.sb:145-146 |
| Cryptex graft points (`/System/Cryptexes/{OS,App}`, `/System/Volumes/Preboot/Cryptexes/...`) | subpath + ancestors | dyld-support.sb:26-38 |
| `/` (libignition openat root) | literal | dyld-support.sb:55-56 |

### 3.2 Filesystem reads — metadata / existence only

| Target | Operations | Citation |
|--------|------------|----------|
| `/etc`, `/tmp`, `/var` | `file-read-metadata file-test-existence` (symlink resolution only — does **not** allow reading directory entries or content) | system.sb:83-86 |
| `/private/etc/localtime` | `file-read-metadata file-test-existence` | system.sb:87 |
| `path-ancestors "/System/Volumes/Data/private"` | `file-read-metadata file-test-existence` | system.sb:90-91 |

### 3.3 Filesystem writes

| Target | Operations | Citation |
|--------|------------|----------|
| `/dev/null`, `/dev/zero` | `file-write-data` | system.sb:111-113 |
| `/dev/fd` (subpath) | `file-write-data file-read-data` | system.sb:117-118 |
| `/dev/dtracehelper` | `file-write-data file-ioctl` | system.sb:120-121 |
| `/cores/*` | `file-write-create` (regular files only) | system.sb:133-135 |

No general-purpose write surface. No write to `/tmp`, `/var`, `$HOME`, or any user-data location.

### 3.4 Executable mapping (`file-map-executable`)

The set is broader than 3.1 in some places (e.g. `/System/Library/Extensions` is mappable but not present in the read list because it's covered by `/System` already; `/System/iOSSupport/...` framework dirs are explicitly mappable):

| Subpath | Citation |
|---------|----------|
| `/Library/Apple/System/Library/Frameworks` | system.sb:44 |
| `/Library/Apple/System/Library/PrivateFrameworks` | system.sb:45 |
| `/Library/Apple/usr/lib` | system.sb:46 |
| `/System/Library/Extensions` | system.sb:47 |
| `/System/Library/Frameworks` | system.sb:48 |
| `/System/Library/PrivateFrameworks` | system.sb:49 |
| `/System/Library/SubFrameworks` | system.sb:50 |
| `/System/iOSSupport/System/Library/{Frameworks,PrivateFrameworks,SubFrameworks}` | system.sb:51-53 |
| `/usr/lib` | system.sb:54 |
| Cryptex graft points (see 3.1) | dyld-support.sb:34 |

### 3.5 Mach services (global-name lookup)

Always allowed via `(allow mach-lookup ...)`:

| Service | Purpose (inferred) |
|---------|--------------------|
| `com.apple.analyticsd` | CoreAnalytics submission |
| `com.apple.analyticsd.messagetracer` | CoreAnalytics message-tracer |
| `com.apple.appsleep` | App Nap |
| `com.apple.bsd.dirhelper` | per-user `/var/folders` dir lookup |
| `com.apple.cfprefsd.agent` | user CFPreferences |
| `com.apple.cfprefsd.daemon` | system CFPreferences |
| `com.apple.diagnosticd` | diagnostics framework |
| `com.apple.dt.automationmode.reader` | automation-mode reader |
| `com.apple.espd` | endpoint-security daemon |
| `com.apple.logd` | unified logging (writes) |
| `com.apple.logd.events` | unified logging events |
| `com.apple.runningboard` | process lifecycle |
| `com.apple.secinitd` | container init / sandbox extensions |
| `com.apple.system.DirectoryService.libinfo_v1` | DirectoryService libinfo |
| `com.apple.system.logger` | ASL (legacy syslog) |
| `com.apple.system.notification_center` | Darwin notification center |
| `com.apple.system.opendirectoryd.libinfo` | OpenDirectory libinfo (`getpwuid` etc.) |
| `com.apple.system.opendirectoryd.membership` | OD membership |
| `com.apple.trustd` | certificate trust evaluation |
| `com.apple.trustd.agent` | certificate trust agent |
| `com.apple.xpc.activity.unmanaged` | XPC activity |
| local-name `com.apple.cfprefsd.agent` | per-process cfprefsd |

Citations: system.sb:156-178.

Conditional (gated):
- `com.apple.internal.objc_trace` — only if `system-attribute apple-internal` (system.sb:180-181). Off on production hardware.
- `com.apple.osanalytics.osanalyticshelper` — only for `is-platform-binary` processes (system.sb:183-184). User binaries do **not** get this.

Plus, for SBPL v1 compatibility: `(allow mach-lookup (xpc-service-name-prefix ""))` (system.sb:22-23) — every XPC service name is allowed for *XPC-service-style* lookup. (This is a separate matcher from `global-name`; it covers names registered as XPC services rather than the bootstrap namespace.)

Plus, mach registration: `(allow mach-register (local-name-prefix ""))` (system.sb:19) — any process can register any local-name service.

Plus, `(allow mach-bootstrap)` and `(allow syscall*)` when `*import-path*` is unset (system.sb:14-16) — i.e., when `system.sb` is loaded as the *primary* profile. **When imported as a sub-profile (which is how `lockin` uses it), `*import-path*` is set, so neither of these is granted by the baseline.** `lockin` users do not inherit blanket `syscall*`.

### 3.6 Network

| Form | Status | Citation |
|------|--------|----------|
| IP networking (TCP/UDP, `network*`) | **Not granted** by baseline. | — |
| Unix-domain outbound to `/private/var/run/syslog` | **Granted** | system.sb:149-150 |
| `system-network` bundle (route/syscontrol sockets, mDNS, networkd, AppSSO, etc.) | **Defined but not invoked** — see §4 | system.sb:267-299 |

### 3.7 sysctl

- `(allow sysctl-read)` — **unrestricted read** of all sysctl names (system.sb:190).
- `(allow sysctl-write (sysctl-name "kern.grade_cputype" "kern.wq_limit_cooperative_threads"))` — narrowly scoped writes only (system.sb:191-193).
- AppleInternal-only: `vm.task_no_footprint_for_debug` write (system.sb:67-68).

### 3.8 IOKit

No `iokit-open*` grants in the unconditional baseline. All IOKit grants live inside the `(define (system-graphics) ...)` bundle (see §4) which is **not auto-invoked**.

### 3.9 POSIX shared memory / semaphores

- `ipc-posix-shm-read*` on `apple.shm.notification_center` (literal) and any name with prefix `apple.cfprefs.` (system.sb:152-154). Read-only.
- No baseline grants for POSIX semaphores.

### 3.10 Signals

No signal-sending rules in the baseline. (Standard POSIX permission checks still apply at the kernel level.)

### 3.11 Other syscalls / capabilities

| Grant | Scope | Citation |
|-------|-------|----------|
| `syscall-unix SYS_csrctl` | one syscall | system.sb:71 |
| `syscall-unix SYS_debug_syscall_reject` | one syscall | system.sb:186-187 |
| `system-mac-syscall vnguard` | guarded vnodes | system.sb:74 |
| `system-mac-syscall Sandbox/67` | container-expected check | system.sb:77-80 |
| `system-fsctl FSIOC_CAS_BSDFLAGS` | `copyfile(3)` chflags | system.sb:99 |
| `system-automount` | platform-binary + restricted only | system.sb:26-29 |
| `dyld bootstrap`: `SYS___mac_syscall`, `SYS_getfsstat[64]`, `SYS_map_with_linking_np`, `SYS_open`, `SYS_openat`, `SYS_fstatat[64]`, `SYS_dup` | per-syscall | dyld-support.sb:41-68 |
| `system-fcntl` `F_ADDFILESIGS_RETURN`, `F_CHECK_LV`, `F_GETPATH` | dyld | dyld-support.sb:45-48 |
| `system-mac-syscall` `Sandbox/2` (SYSCALL_CHECK_SANDBOX) | dyld | dyld-support.sb:49-51 |

## 4. Defined-but-not-invoked bundles

These are `(define ...)` forms in the baseline. They are *not* invoked by `system.sb`. They become live only when a caller invokes them by name from within a profile that inherits these definitions — i.e., a `lockin` consumer using `raw_seatbelt_rules` *could* invoke them.

| Bundle | What invoking it grants | Citation |
|--------|-------------------------|----------|
| `(system-network)` | mach lookups for `networkd`, `nehelper`, `nesessionmanager`, `dnssd.service`, `cfnetworkagent`, `AppSSO.service-xpc`, `symptomsd`, `usymptomsd`, `networkscored`, `SystemConfiguration.{PPPController,SCNetworkReachability}`; `network-outbound` to `com.apple.netsrc` and `com.apple.network.statistics` system-control sockets; `system-socket` AF_SYSTEM/SYSPROTO_CONTROL and AF_ROUTE; reads of `com.apple.networkd.plist`, `com.apple.networkextension.tracker-info`, `nsurlstoraged/dafsaData.bin`; `ipc-posix-shm-read-data` on `/com.apple.AppSSO.version`; `user-preference-read` for `com.apple.CFNetwork` and `com.apple.SystemConfiguration` prefs. **Note:** does not by itself grant IP `network*` — but supplies the surrounding plumbing. | system.sb:267-299 |
| `(system-graphics)` | broad GPU access: `iokit-open-service` for `IOAccelerator`, `IOSurfaceRoot`, `IOFramebuffer`, `AppleGraphicsDeviceControl`, `AGPM`, `AppleGraphicsControl`, `AppleGraphicsPolicy`; `iokit-open-user-client` for the matching client classes including `IOSurfaceRootUserClient`, `IOAccelerationUserClient`, `AppleIntelMEUserClient`, `AppleSNBFBUserClient`, `AGPMClient`, `AppleGraphicsControlClient`, `AppleGraphicsPolicyClient`, `AppleMGPUPowerControlClient`; mach lookups `gpumemd.source`, `lsd.mapdb`, `CARenderServer`, `CoreDisplay.master`, `CoreDisplay.Notification`, `cvmsServ`; `user-preference-read` GPU prefs; reads of `/private/var/db/CVMS` and `/Library/GPUBundles`; `iokit-set-properties` for IODisplay brightness. | system.sb:196-264 |
| `(oopjit-runner)` | `file-read* file-map-executable file-write-unlink` for any path bearing the `com.apple.sandbox.oopjit` extension token. | system.sb:303-305 |

## 5. Implications for `lockin`'s contract

**Current renderer behavior** (`crates/sandbox/src/darwin/policy.rs:8-82`): emits `(version 1) (deny default) (import "system.sb")` then layers structured allows. The `(deny default)` is in front of the import. In SBPL evaluation, allow rules from imports are still effective — `(deny default)` only applies where no allow matches. So the baseline's allows survive `(deny default)`.

`lockin` therefore **cannot honestly claim** "deny-by-default with no implicit grants" on macOS. It enforces deny-by-default *on top of Apple's `system.sb` baseline*. The baseline grants the following **regardless of `lockin` policy**:

1. **Filesystem reads under `/System`, `/usr/lib`, `/usr/share`, `/Library/Apple`, `/Library/Filesystems/NetFSPlugins`, `/Library/Preferences/Logging`, `/private/var/db/timezone`, and the cryptex graft points are unconditional.** The sandboxed program can always `open(2)` and `mmap(2)` system frameworks and dylibs. There is no way to remove this without forking the profile and not importing `system.sb`. (Removing the import would break dyld bootstrap.)
2. **`/private/etc/{passwd,master.passwd,protocols,services}` are readable.** `getpwuid`/`getpwnam`-style lookups against the local file (not OD) work. `/private/etc/localtime` is stat-able; the timezone data under `/private/var/db/timezone` is fully readable.
3. **`/dev/{null,zero,random,urandom,fd,dtracehelper,autofs_nowait}` are accessible.** `/dev/null` and `/dev/zero` are writable; `/dev/fd/*` is read+write; `/dev/dtracehelper` is ioctl-able. `lockin`'s `ioctl_paths` semantics already permit ioctl on baseline paths.
4. **Unix-socket connect to `/private/var/run/syslog` is open.** This is `network-outbound` flavor, not file-write. Implication: `lockin`'s `NetworkMode::Deny` is **IP-deny, not all-sockets-deny**. A program inside `Deny` can still connect to the syslog socket and emit log lines. Likewise `NetworkMode::Proxy` adds its IP allow on top of an already-open syslog socket.
5. **A specific list of Apple Mach services is reachable** (see §3.5). The `lockin` README's claim that no Apple Mach services are reachable is incorrect: at minimum `cfprefsd`, `trustd`, `logd`, `analyticsd`, `runningboard`, `secinitd`, `notification_center`, OpenDirectory libinfo, DirectoryService libinfo, and `diagnosticd` are reachable. Trust evaluation, CoreAnalytics submission, unified-logging writes, and OD-backed identity lookups all still work.
6. **`mach-register` with any local-name and `mach-lookup` for any XPC-service name (SBPL v1 compat) are open.** A sandboxed process can register arbitrary per-pid mach names and do XPC-service-style lookups by name prefix. The bootstrap-namespace `global-name` lookups remain restricted to the list in §3.5.
7. **`sysctl-read` is unrestricted.** Any sysctl name can be read. `kern.grade_cputype` and `kern.wq_limit_cooperative_threads` are writable. This is *much* broader than most callers will assume.
8. **Core dumps to `/cores/` may be created** (regular files only).
9. **Several specific syscalls and `system-mac-syscall`s are pre-allowed** (see §3.11). When `system.sb` is imported (not loaded as primary), the blanket `(allow syscall*)` does **not** apply — so `lockin` does not inherit "all syscalls allowed" the way an unsandboxed process would, but it does inherit the per-syscall bootstrap set required by dyld and a few extras.
10. **`lockin` does not inherit IP networking, IOKit, or graphics access from the baseline.** Those live in `(define ...)` bundles that are not auto-invoked. However, a caller passing `raw_seatbelt_rules` containing `(system-graphics)` or `(system-network)` would unlock the entire bundle in one token — `raw_seatbelt_rules` callers must be aware of this.

## 6. Recommended follow-ups

1. **Update `crates/sandbox/README.md` and the v0.1 security claims doc** to replace any "deny-by-default with no implicit grants" wording with the precise contract: "default-deny on top of Apple's `system.sb` baseline; the baseline is documented in `docs/macos-seatbelt-baseline.md`." *Why:* current claims are not accurate as written.
2. **Add an explicit `(deny network-outbound (literal "/private/var/run/syslog"))` to `NetworkMode::Deny` rendering in `policy.rs`** (and document it in `Proxy` if we want the same posture there). *Why:* without it, `Deny` is IP-only; a program retains a Unix-socket egress channel to syslog.
3. **Document the inherited Mach service surface as part of `NetworkMode::Deny`'s contract** (link to §3.5 of this doc). *Why:* `cfprefsd`, `trustd`, `logd`, `analyticsd`, OpenDirectory libinfo, etc. are reachable regardless of network mode. Anyone reading "network denied" without context will assume otherwise.
4. **Document that filesystem reads under `/System`, `/usr/lib`, `/usr/share`, `/Library/Apple`, `/private/var/db/timezone`, and the cryptex graft points are unconditional** in the public API rustdoc for `SandboxSpec`. *Why:* users may pass these paths in `read_only_dirs` thinking they're adding access; they're not, and removing them from the spec doesn't remove access either.
5. **Pin the SHA-256 hashes from §1 in a build-time check** (e.g., a doctest or an integration test that hashes both files at runtime and asserts against known values, gated behind a feature flag for CI). *Why:* Apple ships silent updates; a baseline drift detector is the only practical defense, and §1 already provides the trusted hashes.
6. **M2 behavioral tests must use temp dirs outside the baseline read allowlists.** Specifically, do **not** rely on `/etc/*`, `/tmp`, `/var/*`, `~/Documents`, `/usr/share/...`, or `/System/...` as "should-be-denied" probes — `/etc`, `/tmp`, `/var` are stat-allowed (false positive on `file-test-existence`), `/usr/share` is fully read-allowed, and `~/Documents` collides with TCC enforcement which may mask Seatbelt behavior. Use a fresh `tempfile::tempdir()` outside the spec's allowlists. *Why:* baseline grants will silently make "should-be-denied" reads succeed and produce green tests for the wrong reason.
7. **M2 mach-lookup tests must probe both an in-baseline-allowed and an out-of-baseline service.** Allowed probe: `com.apple.cfprefsd.daemon` (must succeed). Denied probe: `com.apple.tccd` (must fail — not in §3.5). *Why:* asserting only one side leaves the test unable to distinguish "everything allowed" from "policy correct".
8. **Document `raw_seatbelt_rules` invocation hazards.** `(system-graphics)` and `(system-network)` are one-token bundle invocations that unlock large surfaces (see §4). Add a callout in the rustdoc for `SandboxSpec::raw_seatbelt_rules`. *Why:* the field's footgun risk is much larger than "you can write arbitrary SBPL" suggests; named bundles defined by the baseline can be pulled in with a single S-expression.
9. **Decide and document whether to allowlist `/cores/` writes.** A sandboxed misbehaving binary can produce core dumps in `/cores/` if writeable on this host. *Why:* this is a baseline-granted write surface that escapes the spec's read/write path model entirely.
10. **Re-run this audit on each macOS minor/major bump we support and bump the `Audited on` field in §1.** *Why:* see Apple's caveat — these files can change without notice.
