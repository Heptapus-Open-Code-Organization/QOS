# QOS hardware and VM support

Status reconciled **2026-10-31**. Execution inventory:
[STATUS_AND_BACKLOG.md](STATUS_AND_BACKLOG.md). Canonical hardware tracker:
[#14](https://github.com/Heptapus-Open-Code-Organization/QOS/issues/14).

## Boot and build

The current image builder uses **bootloader 0.11** and produces a **UEFI x86-64** image at
`dist/qos-uefi.img`. The active builder creates the UEFI image only; legacy BIOS/container
instructions from the older bootloader 0.9 path are not current release instructions.

```powershell
cargo image
./run-qos-uefi.ps1 -Build -Serial
```

Prerequisites: the toolchain pinned in `rust-toolchain.toml`, QEMU, and compatible OVMF/edk2
firmware. The Windows launcher discovers QEMU/firmware, uses a private writable firmware
variable store, and handles non-ASCII workspace paths through an ASCII-only run directory.
Inspect the launcher for the exact device profile before reproducing a result.

The modern desktop uses a firmware framebuffer and software compositor with TrueType fonts
at native resolution. This does not establish a vendor GPU driver, hardware acceleration,
runtime mode setting, or multi-monitor support.

## Evidence levels and targets

"QEMU-recorded" means prior WP/ADR records contain successful runs. The October audit rebuilt
the kernel and ran host tests, but did not independently rerun these boots.

| Target | Evidence | Boundary |
|---|---|---|
| QEMU + OVMF, q35 | UEFI/framebuffer, APIC, xHCI HID and AHCI data persistence recorded in WP-01/03/04/05/09 | Selected emulated devices/configurations; networking remains an open work package |
| QEMU + OVMF, pc | UEFI desktop boot reported in README | No claim that every q35 device/interrupt path is covered |
| VirtualBox, VMware, Hyper-V | Unverified for the current image/profile | Use UEFI configuration when testing; collect results under #14 |
| Physical x86-64 UEFI machines | No completed validation matrix identified | Select a machine, firmware version and device inventory before claiming support |
| Legacy BIOS-only machines | No current BIOS image from qos-image | Retired BIOS launch instructions are not proof of current support |
| Other CPU architectures | Outside current initial support profile | No ARM64 or other architecture support claim |

## Device support boundaries

| Subsystem | Current implementation / records | Remaining work |
|---|---|---|
| CPU | Single bootstrap processor, preemptive kernel/Ring-3 scheduling infrastructure | SMP/AP bring-up and per-CPU ownership/scheduling: #7/#20 |
| ACPI/interrupts | Bootloader RSDP/MADT discovery, local APIC and IO-APIC, QEMU-recorded timer/input | Legacy power-control code integration, wider firmware validation: #13/#14 |
| PCI | Config I/O enumeration, standard capability helpers, xHCI-specific MSI-X | ECAM/MCFG, BAR lengths, general vector/resource management: #8 |
| PS/2 | Legacy keyboard/mouse fallback | Validate fallback per target; not a promise of laptop internal input support |
| USB input | xHCI boot-protocol keyboard/mouse, multiple devices, MSI-X plus poll fallback recorded | Hubs, runtime hotplug/removal, general HID and laptop bus inventory: #43 |
| Storage | AHCI/SATA data disk, QOSFS persistence recorded across two boots | Common block layer, NVMe/virtio-blk, multi-device and partition handling: #9 |
| USB storage | No completed mass-storage path | BOT/SCSI and block/VFS integration: #22 |
| Graphics | Firmware framebuffer, native-resolution software desktop | Display backend/mode setting/multi-output: #44 |
| Network | E1000/TCP/IP code; E1000/e1000e mismatch documented in WP-10 | Pin an actual NIC/device ID and verify link, DHCP/DNS/TCP/TLS: #5 |
| Audio | No completed playback stack | Selected controller, PCM/mixer and settings: #28 |
| Power/laptop | Legacy ACPI shutdown/reboot code exists | Verified shutdown first; suspend/resume, battery/thermal policy later: #13 |
| Wi-Fi/Bluetooth/camera | No completed implementation identified in reviewed paths | Deferred target decisions; do not infer support from PCI/USB enumeration |

A USB keyboard result does not prove I2C-HID touchpad support. Two-boot file survival does not
prove power-loss/crash consistency (#27). Missing-device fallbacks need tests alongside success
paths; support is a per-configuration claim.

## Safe VM and physical validation workflow

1. Build the current UEFI image and record the commit plus any local modifications.
2. Record QEMU/hypervisor version, firmware, CPU/memory, disk/input/display/NIC device models
   and exact launch arguments.
3. Capture serial boot markers, faults and W^X results; exercise input, desktop, storage and
   networking separately. Record "unverified" rather than extrapolating from a device name.
4. Use a disposable data image to test write/reboot/removal behavior. Preserve the current
   launcher boot-volume protections; QOSFS has no crash-consistency guarantee yet.
5. For physical USB boot, use a dedicated disposable medium and identify the destination
   explicitly before writing `dist/qos-uefi.img`; imaging overwrites the chosen medium.
   Secure Boot enrollment/verified boot is unfinished (#46), so firmware policy must be
   recorded rather than claiming an established trust chain.
6. Booting the image from removable media is distinct from installing QOS to a target disk.
   GPT/ESP installation and boot without installation media remain #10.

## Result record required by #14

For every VM or physical run, store the date, commit/worktree status, model/firmware, device
IDs, launch or boot settings, pass/fail/unverified for boot/display/input/storage/network/power,
serial/screenshots, reproduction steps and remaining limitations. Physical machine names are
added to the support matrix only after this evidence exists.

Relevant records: [WP-04 USB](wp/WP-04-usb-input.md),
[WP-09 VFS](wp/WP-09-vfs-unification.md),
[WP-10 networking](wp/WP-10-networking.md),
[ADR-0014 boot](adr/0014-migrate-to-bootloader-0.11-uefi-framebuffer.md).
