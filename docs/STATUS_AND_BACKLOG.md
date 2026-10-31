# QOS implementation baseline and backlog

Audit date: **2026-10-31 (Europe/Istanbul)**. Tracker: [#39](https://github.com/Heptapus-Open-Code-Organization/QOS/issues/39).

This is the dated execution baseline for hardware, software, quantum, security and delivery
work. `MASTERPLAN.md` retains the long-range vision; WPs and ADRs retain their delivered scope
and decisions. GitHub issues are the canonical actionable records. This audit does not mark
unfinished implementation as complete or make a production-security claim.

## Evidence and status rules

- **Host-tested:** exercised by the listed host checks in the audited working tree.
- **QEMU-recorded:** prior WP/ADR records describe QEMU+OVMF success; not rerun during this audit.
- **Partial:** an implementation exists but is incomplete, bypassed by another path, or lacks
  the required end-to-end evidence.
- **Missing:** no complete implementation was identified in the reviewed active paths.
- **Unverified:** a target or behavior needs a recorded run; source presence is not support.

Local checks recorded in this session:

```powershell
cargo test -p qos-abi -p qos-gfx -p qos-ui -p qos-driver -p qos-acpi -p qos-core --features qos-core/std
cargo os-build
cargo check -p qos-core --no-default-features --target x86_64-unknown-none
```

Results: **69 tests and one doctest passed**; kernel build passed with **409 warnings**;
portable `no_std` target check passed. Test counts include the pre-existing, uncommitted
portable routing increment. This is local workspace evidence, not proof of merged/released
behavior. No new QEMU boot, installed-disk boot, cloud-provider or physical-machine test was
performed for this audit. Warnings need classification, not blanket suppression.

## First implementation increment (2026-10-31)

[WP-14](wp/WP-14-verification-gates.md) implements the first #25 slice locally: a strict
UEFI runner, separate verify image, production desktop-frame rendering and real Bell smoke.
The complete `cargo os-verify` path passed on Windows/QEMU 10.1.0 with bundled edk2 in about
10 seconds (guest exit 33). A real one-second timeout failed with exit 1. The interactive
image's SHA-256 stayed unchanged across verification. Current host checks passed **75 tests
and one doctest**; one ignored test is invoked as a sleeping subprocess fixture by the timeout
test. `no_std` and interactive-image builds passed. Publication and Linux GitHub Actions
results are tracked separately on #25. Process, persistence, input and worker
load integration acceptance remains open; the initial audit evidence above is historical.

Publication update: `1108fa5` was pushed to main and all four jobs in
[its Linux CI run](https://github.com/Heptapus-Open-Code-Organization/QOS/actions/runs/41210140971)
passed. [WP-15](wp/WP-15-user-memory-transfers.md) records the subsequent #40 bounded resident
copy increment on `feat/syscall-user-memory`, including actual page-permission/cross-page/VFS
checks and Ring-3 register write/exit. The new v2 smoke runs max and qemu64 CPU profiles; this
does not close the remaining ABI, SMP/VM pinning, resource or legacy-loader acceptance.

## Initial support envelope

The proposed first profile is x86-64 UEFI on a documented QEMU+OVMF configuration: framebuffer,
xHCI keyboard/mouse, AHCI data disk, and one explicitly selected NIC. Exact device IDs and
launch flags must be pinned by [#14](https://github.com/Heptapus-Open-Code-Organization/QOS/issues/14)
and [#5](https://github.com/Heptapus-Open-Code-Organization/QOS/issues/5). A physical UEFI machine
is to be selected and measured under #14; none is promoted to supported by this document.

ARM64, broad vendor-GPU coverage, Wi-Fi/Bluetooth, camera support, advanced media and pulse-level
local QPU control are deferred target decisions. They are not implied by the initial profile.
SMP is a planned correctness milestone; it does not block the first single-core userland slice.

## Hardware inventory

Issue links below use the canonical open records, not closed duplicates #29-#38.

| Area | Audited baseline | Missing / incomplete | Tracking |
|---|---|---|---|
| Boot/platform | UEFI image builder; QEMU-recorded boot | Firmware/VM variants, physical support matrix, recovery | [#14](https://github.com/Heptapus-Open-Code-Organization/QOS/issues/14), [#46](https://github.com/Heptapus-Open-Code-Organization/QOS/issues/46) |
| CPU/SMP | Single BSP; APIC-timer preemption | AP startup, per-CPU state, SMP-safe locking and scheduling | [#7](https://github.com/Heptapus-Open-Code-Organization/QOS/issues/7), [#20](https://github.com/Heptapus-Open-Code-Organization/QOS/issues/20) |
| PCI/driver model | Registry, config I/O, standard capability helpers, xHCI-specific MSI-X | ECAM/MCFG, shared vector/resource lifecycle, BAR lengths, driver migration | [#8](https://github.com/Heptapus-Open-Code-Organization/QOS/issues/8) |
| USB/input | QEMU-recorded boot HID keyboard/mouse | Hubs, runtime attach/detach, general HID; selected laptop input bus | [#43](https://github.com/Heptapus-Open-Code-Organization/QOS/issues/43), [#14](https://github.com/Heptapus-Open-Code-Organization/QOS/issues/14) |
| Storage devices | QEMU-recorded AHCI/SATA data persistence | Common block API/queue/cache, NVMe, virtio-blk, multi-device handling | [#9](https://github.com/Heptapus-Open-Code-Organization/QOS/issues/9) |
| Removable storage | No completed USB storage path | Bulk/BOT/SCSI, block registration, filesystem integration, safe removal | [#22](https://github.com/Heptapus-Open-Code-Organization/QOS/issues/22) |
| Display/GPU | Firmware framebuffer; software compositor and TTF | Runtime modes, display backend, scaling/multi-output; measured acceleration need | [#44](https://github.com/Heptapus-Open-Code-Organization/QOS/issues/44) |
| Networking | E1000/TCP/IP code; documented q35 NIC mismatch | Verified bind/RX/TX, DHCP/DNS/ICMP/TCP, sockets and authentic TLS | [#5](https://github.com/Heptapus-Open-Code-Organization/QOS/issues/5) |
| Power/time | Modern APIC plus separate legacy ACPI power code; tick/RTC helpers | Unified clock/waits, clean poweroff/reboot; later suspend/resume, battery/thermal policy | [#21](https://github.com/Heptapus-Open-Code-Organization/QOS/issues/21), [#13](https://github.com/Heptapus-Open-Code-Organization/QOS/issues/13) |
| Audio | Missing playback stack | Selected controller, bounded PCM/DMA/mixer, volume/settings | [#28](https://github.com/Heptapus-Open-Code-Organization/QOS/issues/28) |

## Software inventory

| Area | Audited baseline | Missing / incomplete | Tracking |
|---|---|---|---|
| Process/application model | Ring-3/page-table/fault demos; desktop apps remain in kernel | Real app processes, lifecycle, IPC/service boundary, process-owned resources | [#41](https://github.com/Heptapus-Open-Code-Organization/QOS/issues/41), [#7](https://github.com/Heptapus-Open-Code-Organization/QOS/issues/7) |
| Memory | Paging, frame recycle list, fixed 64 MiB kernel heap | VMA/demand paging, guards/CoW/mmap, controlled OOM, ownership/cleanup and budgets | [#20](https://github.com/Heptapus-Open-Code-Organization/QOS/issues/20), [#7](https://github.com/Heptapus-Open-Code-Organization/QOS/issues/7) |
| ABI/IPC | Register/shared-buffer ABIs, extended file helpers, one global demo pipe | Safe memory-transfer helpers, stable errors/versioning, owned endpoints/FDs | [#40](https://github.com/Heptapus-Open-Code-Organization/QOS/issues/40), [#41](https://github.com/Heptapus-Open-Code-Organization/QOS/issues/41) |
| Scheduling/waits | Preemptive scheduler; polling/tick-based waits | Blocking queues, wakeup/cancel semantics, calibrated monotonic deadlines | [#21](https://github.com/Heptapus-Open-Code-Organization/QOS/issues/21), [#7](https://github.com/Heptapus-Open-Code-Organization/QOS/issues/7) |
| Filesystem namespace | VFS and GUI adoption delivered; QOSFS flat; optional FAT16 read | Compositor Terminal path parity, persistent dirs/metadata, mount UX | [#6](https://github.com/Heptapus-Open-Code-Organization/QOS/issues/6) |
| Filesystem durability | Two-boot persistence recorded; no crash consistency | Flush/atomic update/recovery/fsck, disk-full/free-space behavior and format versioning | [#27](https://github.com/Heptapus-Open-Code-Organization/QOS/issues/27), [#9](https://github.com/Heptapus-Open-Code-Organization/QOS/issues/9) |
| SDK/app distribution | ELF/flat-image demos; no complete external app workflow | Minimal SDK/library/linker contract, launch, package/update model | [#15](https://github.com/Heptapus-Open-Code-Organization/QOS/issues/15), [#26](https://github.com/Heptapus-Open-Code-Organization/QOS/issues/26) |
| Services/diagnostics | Bespoke kernel workers and serial logging | Init/service lifecycle, bounded logs, restart/health and crash evidence | [#16](https://github.com/Heptapus-Open-Code-Organization/QOS/issues/16) |
| Desktop/interaction | Native-resolution windows, apps, editors and themes | Edge resize, notification/image viewer, shared selection/clipboard, Unicode/input/accessibility | [#11](https://github.com/Heptapus-Open-Code-Organization/QOS/issues/11), [#45](https://github.com/Heptapus-Open-Code-Organization/QOS/issues/45) |
| Setup/install | Config persistence and setup code exist | Verify OOBE evidence, login/i18n, GPT/ESP installation, installed-disk boot and recovery | [#10](https://github.com/Heptapus-Open-Code-Organization/QOS/issues/10) |
| Identity/security | CPU hardening and isolation building blocks | Syscall boundaries, authorization, entropy/crypto/secrets and verified system updates | [#40](https://github.com/Heptapus-Open-Code-Organization/QOS/issues/40), [#24](https://github.com/Heptapus-Open-Code-Organization/QOS/issues/24), [#19](https://github.com/Heptapus-Open-Code-Organization/QOS/issues/19), [#46](https://github.com/Heptapus-Open-Code-Organization/QOS/issues/46) |
| Quality/release | Portable tests and kernel/image CI jobs; best-effort boot smoke | Required end-to-end gates, negative/fuzz tests, persistence/load checks, reproducible release evidence | [#25](https://github.com/Heptapus-Open-Code-Organization/QOS/issues/25), [#39](https://github.com/Heptapus-Open-Code-Organization/QOS/issues/39) |

## Quantum correctness and integration

- [#42](https://github.com/Heptapus-Open-Code-Organization/QOS/issues/42): choose canonical
  IR/engine/QHAL; reconcile portable, kernel typed and parser/simulator paths. Validate classical
  destination/width/order, reset, repeated and mid-circuit measurement, and count semantics.
  Some reviewed typed execution paths ignore the classical destination of `Measure(q,c)`.
- Limits must follow actual allocation budgets, not qubit labels alone. The modern simulator
  uses a 20-qubit ceiling while other paths advertise/account for 32. A complex-f64 statevector
  alone needs 16 MiB at 20 qubits and 64 GiB at 32, before other allocations. Bound jobs,
  shots/gates/time, cancellation and concurrent state memory across every entry point.
- Portable routing and its tests exist as a local increment; Lab/Studio/Terminal still need
  shared execution integration. Native decomposition, directed couplings and improved layout
  are unfinished. Do not equate the local patch with a shipped hardware backend.
- [#23](https://github.com/Heptapus-Open-Code-Organization/QOS/issues/23): noise, mitigation,
  calibration and hybrid depth build on the measurement/resource correctness baseline.
- [#12](https://github.com/Heptapus-Open-Code-Organization/QOS/issues/12): mock then provider
  adapters, backend selection and resilient jobs depend on verified networking (#5), secure
  channels/secrets (#19), and shared execution (#42). No real provider run is claimed.

## Execution stages and dependency gates

These are execution stages **S0-S4**, not replacements for the repository's existing GitHub
milestones M1-M4 or the historical MASTERPLAN milestone bands M1-M6. Issues retain their
existing milestone system; use dependencies to sequence smaller slices.

| Stage | Scope | Exit evidence |
|---|---|---|
| S0: trustworthy baseline | #39, first #25 slices, #14 profile | Consistent inventory, reproducible checks, warning triage and explicit support/evidence labels |
| S1: real userland | #40, first #7/#20/#21/#15 slices, #41; #42 correctness can start independently | External app runs in Ring 3; another app/desktop survives crash/quota; frames/handles/IPC reclaimed |
| S2: durable connected system | #6, #9, #27, #5, #16/#24 foundations | Uniform persistent paths, bounded block I/O, crash-recovery tests, deterministic network round-trips |
| S3: selected hardware expansion | #8, SMP slices of #7, #43/#22, selected NVMe/display/audio slices | Documented VM/device configurations plus selected physical-device evidence; missing devices degrade cleanly |
| S4: usable quantum product | #42 integration, #23/#12, #10/#11/#19/#26/#45/#46 | Installed-disk boot/recovery, usable isolated apps, tested local/remote job flow and signed-update rejection/recovery |

Local quantum correctness does not wait for networking. Cloud-provider traffic waits for
network/security gates. First userland does not wait for SMP, every graphics backend, audio or
all VMM optimizations. Storage durability and boot trust are prerequisites before storing
credentials or advertising a recoverable installed system. GUI improvements may land when
their own layout/input/service contracts are ready.

Recommended first implementation slice after this planning pass: make the existing CI/QEMU
gate truthful (#25), alongside a bounded, reviewable user-memory transfer slice (#40).
Do not begin an unbounded rewrite of the kernel, desktop and quantum engine together.

## Completion record for every implementation slice

Published follow-up: [PR #47](https://github.com/Heptapus-Open-Code-Organization/QOS/pull/47)
merged as `e43c918`; all five main Linux CI jobs passed. [WP-15](wp/WP-15-user-memory-transfers.md)
records the bounded user-copy and entry-policy implementation. [WP-16](wp/WP-16-syscall-abi-contract.md)
continues #40 with the current ABI contract, deterministic shared responses and actual shared
process exit. The execution stages above remain unchanged; resource ownership, SDK and
isolated applications are still unfinished.

Record the issue, scope/ADR, prerequisites, commit/PR, exact configuration/commands, observed
results, resource/performance limits, regression/negative cases, and remaining limitations.
Host checks cannot close a hardware-verification criterion; two-boot survival cannot close a
crash-consistency criterion; login UI cannot close an authorization criterion. Security-sensitive
reproductions follow `SECURITY.md` rather than public issue discussions.
