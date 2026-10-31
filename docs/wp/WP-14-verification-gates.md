# WP-14: Truthful UEFI verification and CI gates

- Status: partial — first bounded boot smoke slice implemented and locally verified
- Issue: [#25](https://github.com/Heptapus-Open-Code-Organization/QOS/issues/25)
- Audit tracker: [#39](https://github.com/Heptapus-Open-Code-Organization/QOS/issues/39)
- Date: 2026-10-31
- Publication and Linux CI evidence: tracked on #25; local verification alone does not close it

## Problem and first slice

The old `cargo os-verify` alias referenced a disabled, BIOS-era xtask. The CI smoke job built
without forwarding the kernel verification feature, tolerated failure and accepted timeout as
success. The kernel verification branch exited without executing the checks advertised by
the old runner.

This slice adds a dependency-free host package, `qos-verify`, behind `cargo os-verify`. It builds
the real UEFI image with `qos-image/verify -> os/verify`, starts QEMU+OVMF and checks both the
guest exit status and a complete, ordered serial/debugcon transcript. The interactive image
is `dist/qos-uefi.img`; verification is `dist/qos-uefi-verify.img`. Building/running verification
must not overwrite the interactive image.

## Commands and environment

```powershell
# Same entry point locally and in Linux CI; builds the verification image first.
cargo os-verify

# Reuse an already built verification image, or change the bounded guest deadline.
cargo os-verify --skip-build
cargo os-verify --skip-build --timeout-secs 1  # negative test: should fail if boot exceeds 1s

# Portable checks, including the host runner's rejection/timeout tests.
cargo test -p qos-abi -p qos-gfx -p qos-ui -p qos-driver -p qos-acpi -p qos-core -p qos-verify --features qos-core/std
cargo check -p qos-core --no-default-features --target x86_64-unknown-none
```

`QEMU_EXE` overrides executable discovery. Set **both** `QEMU_UEFI_CODE` and `QEMU_UEFI_VARS`
to override a matching firmware pair. Discovery checks QEMU's bundled edk2 pair and common
Linux OVMF pairs, preferring matching 4M files. Missing/half-configured firmware is an error;
there is no silent BIOS fallback or fabricated variable-store image. This profile uses
non-Secure-Boot firmware and does not verify a Secure Boot trust chain.

The runner copies boot image and firmware to a unique ASCII temporary directory and uses
relative filenames in QEMU arguments (Windows non-ASCII-path compatibility). `QOS_VERIFY_TEMP`
can select an ASCII temporary base. Every run gets a private writable variable store and a
snapshot boot disk; existing interactive/persistent data images are not attached or modified.
The profile is q35/TCG, 512 MiB, xHCI keyboard/mouse, display/monitor disabled and networking off.

`--image PATH` selects a different image for diagnostic negative tests. It does not relax the
marker/exit requirements. `--log-dir BASE` selects an artifact base; each run gets a new child
directory so stale logs cannot satisfy a later run.

## Guest contract v1

One complete channel must contain these exact lines in order:

```text
QOS-OS boot OK (serial)
heap initialized
QOS_VERIFY BEGIN v1
QOS_VERIFY PASS boot-framebuffer
QOS_VERIFY PASS wx
QOS_VERIFY PASS desktop-frame
QOS_VERIFY PASS quantum-bell
QOS_VERIFY COMPLETE v1
```

Other diagnostic lines may appear between markers. Any `PANIC:`, `EXCEPTION:` or
`QOS_VERIFY FAIL` in either captured channel rejects the run, even if the other channel
contains success markers. Only QEMU exit **33**, encoding guest `isa-debug-exit` value 0x10,
is successful. Exit 0, 35 (guest failure), 124, a signal, missing/out-of-order markers and
timeouts all fail. A timed-out guest is killed and reaped before diagnostics are collected.
QEMU stdout/stderr go to files rather than undrained pipes.

The guest checks a live framebuffer, a nonempty zero-W+X page-table audit, production desktop
composition/presentation of one frame, and the **real modern simulator** on a 256-shot Bell
circuit. The Bell check requires total counts 256 and both 00/11 with no other outcomes. It
does not use the legacy ABI verify-mode mock.

## Artifacts and CI

Artifacts are written under `target/qos-verify/run-*`: serial/debugcon, QEMU stdout/stderr,
command/firmware/image paths, QEMU version, process outcome/deadline and final result. Source
images/private run copies remain in the reported temporary directory for diagnosis.

The `smoke-uefi` CI job now invokes `cargo os-verify` with a job-level deadline and uploads
diagnostics on success or failure. It no longer uses `continue-on-error` or accepts timeout.
The host job runs all portable crates plus runner tests; the separate image job still builds
and publishes the **interactive** UEFI image. Repository branch protection must select the
desired status checks separately; this working-tree change does not change GitHub settings.

## Local evidence and remaining acceptance

- [x] Runner tests reject wrong exits, timeout, incomplete/unordered transcripts and errors
  on either channel; a subprocess test proves timeout termination/reaping.
- [x] Local Windows QEMU+bundled edk2 run completed all v1 markers and exited successfully.
- [x] A real one-second QEMU run was rejected with exit 1 and timeout diagnostics.
- [x] Full local host suite: 75 tests + one doctest passed; one subprocess fixture is ignored
  by the parent harness and invoked by its timeout test. `no_std` and interactive image builds
  passed; the interactive image SHA-256 was unchanged by the subsequent full verify command.
- [ ] Publish changes and observe the required Linux GitHub Actions job on a PR.
- [ ] Two-boot storage persistence with a disposable AHCI data image and explicit assertions.
- [ ] Ring-3 syscall/isolation/cleanup/negative checks as #40/#41 mature.
- [ ] Background worker/cancel/load responsiveness and wider quantum correctness checks.
- [ ] Scripted input and optional screenshot artifacts; physical-device validation remains #14.

A presented frame is not an interactive input test. A direct Bell smoke is not a background
worker or Ring-3 quantum-service test. Missing storage/NIC in this profile is intentional;
their success paths remain open. Keep #25 open until its broader acceptance criteria land.
