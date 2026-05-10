Round-12 review of `rafaello/plans/milestones/m3-tui-sessions/scope.md` @ `5171b26`.

No new blocking issues found, but I’d fix these before ratification:

1. **High — `ReaperPanicked` is handled but no frontend producer is specified.**  
   §F3/F4 only spawn a reaper task that writes `Exited/WaitFailed` into the watch (`scope.md:779-785`, `858-864`). §C2 then requires handling `ReaperPanicked` (`scope.md:1906-1915`). m2 produces that via a watcher task awaiting the reaper `JoinHandle`; m3 should either specify the same watcher bridge or remove the frontend `ReaperPanicked` branch.

2. **High — `ShutdownReport.exit_status` population is unspecified, but tests depend on it.**  
   `ShutdownReport` has `exit_status` (`scope.md:678-684`), and SenderDropped errors cite the shutdown report / child status (`scope.md:1838-1851`, `2426-2432`). But shutdown steps only say “Return `ShutdownReport`” and never say to copy cached `Exited(status)` / post-SIGTERM outcome into `exit_status` (`scope.md:908-938`). Pin this explicitly.

3. **Medium — new fixture env var is not passed through `rfl chat`.**  
   `signal_ready_then_exit_n` reads `RFL_FIXTURE_EXIT_CODE` (`scope.md:2593-2599`), and the CLI test sets it (`scope.md:2434-2441`), but the `CompiledFrontend.env.pass` allowlist omits it (`scope.md:1751-1761`). The test passes only because default is also `7`; non-default coverage would silently fail.

4. **Medium — `FrontendHandle` peer field is inconsistent.**  
   F1 says `FrontendHandle` carries a peer handle (`scope.md:584-587`) and F3 returns `peer` in the handle literal (`scope.md:801-805`), but the canonical struct sketch omits `peer` (`scope.md:876-890`). Add it or remove the earlier references.

5. **Low — stale `config.*` / flock wording remains.**  
   After adding `FrontendHandle.config`, shutdown pseudocode still uses unqualified `config.shutdown_*` (`scope.md:929-934`), and C2 does the same (`scope.md:1843-1845`). Also S1 still says `flock(LOCK_EX | LOCK_NB)` despite §S5 switching to `Flock::lock` (`scope.md:1146-1148`, `1286-1290`). These are small, but easy to misread during implementation.
