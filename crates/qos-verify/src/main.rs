use std::env;
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

const MARKERS: &[&str] = &[
    "QOS-OS boot OK (serial)",
    "heap initialized",
    "QOS_VERIFY BEGIN v3",
    "QOS_VERIFY PASS boot-framebuffer",
    "QOS_VERIFY PASS wx",
    "QOS_VERIFY PASS desktop-frame",
    "QOS_VERIFY PASS quantum-bell",
    "QOS_VERIFY PASS user-copy",
    "hello from ring3 via register syscall ABI (int 0x81)",
    "QOS_VERIFY PASS ring3-write-exit",
    "ring3 negative buffers and entry flags passed",
    "QOS_VERIFY PASS entry-policy",
    "QOS_VERIFY COMPLETE v3",
];

#[derive(Debug)]
struct Options {
    skip_build: bool,
    image: PathBuf,
    log_dir: PathBuf,
    timeout: Duration,
    cpu: String,
}

impl Options {
    fn parse(args: impl IntoIterator<Item = String>) -> Result<Option<Self>> {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap();
        let mut options = Self {
            skip_build: false,
            image: root.join("dist/qos-uefi-verify.img"),
            log_dir: root.join("target/qos-verify"),
            timeout: Duration::from_secs(120),
            cpu: "max".into(),
        };
        let mut args = args.into_iter();
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--skip-build" => options.skip_build = true,
                "--cpu" => {
                    options.cpu = args
                        .next()
                        .filter(|s| !s.is_empty())
                        .ok_or("--cpu needs a model")?
                }
                "--image" => options.image = args.next().ok_or("--image needs a path")?.into(),
                "--log-dir" => {
                    options.log_dir = args.next().ok_or("--log-dir needs a path")?.into()
                }
                "--timeout-secs" => {
                    let seconds: u64 =
                        args.next().ok_or("--timeout-secs needs a value")?.parse()?;
                    if !(1..=3600).contains(&seconds) {
                        return Err("timeout must be 1..=3600 seconds".into());
                    }
                    options.timeout = Duration::from_secs(seconds);
                }
                "--help" | "-h" => return Ok(None),
                _ => return Err(format!("unknown argument: {arg}").into()),
            }
        }
        Ok(Some(options))
    }
}

fn main() {
    if let Err(error) = run() {
        eprintln!("QOS verification FAILED: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let Some(options) = Options::parse(env::args().skip(1))? else {
        println!(
            "cargo os-verify [--skip-build] [--timeout-secs 120] [--cpu max] [--image PATH] [--log-dir BASE]\n\
                  QEMU_EXE, QEMU_UEFI_CODE and QEMU_UEFI_VARS override discovery.\n\
                  Requires a matching non-Secure-Boot OVMF/edk2 code/vars pair.\n\
                  Only exit 33 plus all smoke markers succeeds; timeout never succeeds."
        );
        return Ok(());
    };
    let qemu = find_qemu()?;
    let (code, vars) = find_firmware(&qemu)?;
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    if !options.skip_build {
        let cargo = env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
        let status = Command::new(cargo)
            .current_dir(root)
            .args([
                "build",
                "-p",
                "qos-image",
                "--release",
                "--features",
                "verify",
            ])
            .status()?;
        if !status.success() {
            return Err(format!("verification image build failed: {status}").into());
        }
    }
    if !options.image.is_file() {
        return Err(format!("image not found: {}", options.image.display()).into());
    }

    let run_id = format!(
        "run-{}-{}",
        std::process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
    );
    fs::create_dir_all(&options.log_dir)?;
    let output = options.log_dir.join(&run_id);
    fs::create_dir(&output)?;
    let scratch_base = env::var_os("QOS_VERIFY_TEMP")
        .map(PathBuf::from)
        .unwrap_or_else(env::temp_dir);
    let scratch = scratch_base.join(format!("qos-verify-{run_id}"));
    if !scratch.as_os_str().to_string_lossy().is_ascii() {
        return Err(
            "QEMU run directory must be ASCII; set QOS_VERIFY_TEMP to an ASCII directory".into(),
        );
    }
    fs::create_dir_all(&scratch_base)?;
    fs::create_dir(&scratch)?;
    fs::copy(&options.image, scratch.join("boot.img"))?;
    fs::copy(&code, scratch.join("code.fd"))?;
    fs::copy(&vars, scratch.join("vars.fd"))?;
    #[cfg(windows)]
    {
        let mut permissions = fs::metadata(scratch.join("vars.fd"))?.permissions();
        permissions.set_readonly(false);
        fs::set_permissions(scratch.join("vars.fd"), permissions)?;
    }
    let mut command = Command::new(&qemu);
    command.current_dir(&scratch).args([
        "-machine",
        "q35",
        "-accel",
        "tcg",
        "-m",
        "512M",
        "-drive",
        "if=pflash,unit=0,format=raw,readonly=on,file=code.fd",
        "-drive",
        "if=pflash,unit=1,format=raw,file=vars.fd",
        "-drive",
        "format=raw,file=boot.img,snapshot=on",
        "-device",
        "qemu-xhci",
        "-device",
        "usb-kbd",
        "-device",
        "usb-mouse",
        "-device",
        "isa-debug-exit,iobase=0xf4,iosize=0x04",
        "-serial",
        "file:serial.log",
        "-debugcon",
        "file:debugcon.log",
        "-global",
        "isa-debugcon.iobase=0xe9",
        "-display",
        "none",
        "-monitor",
        "none",
        "-net",
        "none",
        "-no-reboot",
    ]);
    command.args(["-cpu", &options.cpu]);
    fs::write(output.join("command.txt"), format!(
        "image: {}\nfirmware code: {}\nfirmware vars: {}\ntimeout: {:?}\nscratch: {}\ncommand: {command:?}\n",
        options.image.display(), code.display(), vars.display(), options.timeout, scratch.display(),
    ))?;
    command
        .stdin(std::process::Stdio::null())
        .stdout(File::create(scratch.join("qemu.stdout.log"))?)
        .stderr(File::create(scratch.join("qemu.stderr.log"))?);
    eprintln!(
        "UEFI smoke: timeout {:?}; artifacts {}",
        options.timeout,
        output.display()
    );
    // File redirection avoids a full stdout/stderr pipe wedging the guest before it exits.
    let started = Instant::now();
    let outcome = (|| -> Result<(ExitStatus, bool)> {
        let mut child = command.spawn()?;
        wait_for_guest(&mut child, options.timeout)
    })();
    fs::write(
        output.join("process.txt"),
        format!("outcome: {outcome:?}\nelapsed: {:?}\n", started.elapsed()),
    )?;
    if let Ok(version) = Command::new(&qemu).arg("--version").output() {
        fs::write(output.join("qemu-version.txt"), version.stdout)?;
    }
    for name in [
        "serial.log",
        "debugcon.log",
        "qemu.stdout.log",
        "qemu.stderr.log",
    ] {
        if scratch.join(name).is_file() {
            fs::copy(scratch.join(name), output.join(name))?;
        }
    }
    let debug = fs::read_to_string(output.join("debugcon.log")).unwrap_or_default();
    let serial = fs::read_to_string(output.join("serial.log")).unwrap_or_default();
    let verification = outcome.and_then(|(status, timeout)| {
        validate_logs(&debug, &serial, status.code(), timeout).map_err(Into::into)
    });
    fs::write(
        output.join("result.txt"),
        match &verification {
            Ok(()) => {
                "PASS: UEFI/framebuffer/W^X/desktop/Bell/user-copy/Ring-3 write-exit\n".to_owned()
            }
            Err(error) => format!("FAIL: {error}\n"),
        },
    )?;
    verification?;
    println!("QOS verification PASS: {}", output.display());
    Ok(())
}

fn wait_for_guest(child: &mut Child, timeout: Duration) -> Result<(ExitStatus, bool)> {
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok((status, false)),
            Ok(None) => {}
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error.into());
            }
        }
        if start.elapsed() >= timeout {
            child.kill()?;
            return Ok((child.wait()?, true));
        }
        thread::sleep(Duration::from_millis(50));
    }
}

fn validate_logs(
    debug: &str,
    serial: &str,
    code: Option<i32>,
    timed_out: bool,
) -> std::result::Result<(), String> {
    if timed_out {
        return Err("QEMU timed out (guest killed and reaped)".into());
    }
    if code != Some(33) {
        return Err(format!("expected isa-debug-exit status 33, got {code:?}"));
    }
    for log in [debug, serial] {
        for forbidden in ["PANIC:", "EXCEPTION:", "QOS_VERIFY FAIL"] {
            if log.contains(forbidden) {
                return Err(format!("guest reported {forbidden}"));
            }
        }
    }
    // Require a complete, ordered transcript from one channel, not fragments pooled together.
    for log in [debug, serial] {
        let lines: Vec<_> = log.lines().map(str::trim_end).collect();
        let mut next = 0;
        for line in lines {
            if next < MARKERS.len() && line == MARKERS[next] {
                next += 1;
            }
        }
        if next == MARKERS.len() {
            return Ok(());
        }
    }
    Err("missing or out-of-order verification markers; inspect serial/debugcon artifacts".into())
}

fn find_qemu() -> Result<PathBuf> {
    if let Some(value) = env::var_os("QEMU_EXE") {
        return resolve_executable(Path::new(&value))
            .ok_or_else(|| "QEMU_EXE does not resolve to an executable file".into());
    }
    let name = if cfg!(windows) {
        "qemu-system-x86_64.exe"
    } else {
        "qemu-system-x86_64"
    };
    if let Some(path) = resolve_executable(Path::new(name)) {
        return Ok(path);
    }
    if let Some(program_files) = env::var_os("ProgramFiles") {
        let path = PathBuf::from(program_files).join("qemu").join(name);
        if path.is_file() {
            return Ok(path);
        }
    }
    Err("QEMU not found; install it or set QEMU_EXE".into())
}

fn resolve_executable(path: &Path) -> Option<PathBuf> {
    if path.is_file() {
        return fs::canonicalize(path).ok();
    }
    env::var_os("PATH")
        .and_then(|paths| {
            env::split_paths(&paths)
                .map(|dir| dir.join(path))
                .find(|p| p.is_file())
        })
        .and_then(|p| fs::canonicalize(p).ok())
}

fn find_firmware(qemu: &Path) -> Result<(PathBuf, PathBuf)> {
    match (env::var_os("QEMU_UEFI_CODE"), env::var_os("QEMU_UEFI_VARS")) {
        (Some(code), Some(vars)) => {
            let pair = (PathBuf::from(code), PathBuf::from(vars));
            if pair.0.is_file() && pair.1.is_file() {
                return Ok(pair);
            }
            return Err("configured firmware code/vars must both be existing files".into());
        }
        (None, None) => {}
        _ => return Err("set both QEMU_UEFI_CODE and QEMU_UEFI_VARS to a matching pair".into()),
    }
    let share = qemu
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("share");
    let pairs = [
        (
            share.join("edk2-x86_64-code.fd"),
            share.join("edk2-i386-vars.fd"),
        ),
        (
            "/usr/share/OVMF/OVMF_CODE_4M.fd".into(),
            "/usr/share/OVMF/OVMF_VARS_4M.fd".into(),
        ),
        (
            "/usr/share/OVMF/OVMF_CODE.fd".into(),
            "/usr/share/OVMF/OVMF_VARS.fd".into(),
        ),
        (
            "/usr/share/ovmf/OVMF_CODE.fd".into(),
            "/usr/share/ovmf/OVMF_VARS.fd".into(),
        ),
        (
            "/usr/share/qemu/edk2-x86_64-code.fd".into(),
            "/usr/share/qemu/edk2-i386-vars.fd".into(),
        ),
    ];
    pairs
        .into_iter()
        .find(|(code, vars)| code.is_file() && vars.is_file())
        .ok_or_else(|| {
            "matching OVMF firmware not found; set QEMU_UEFI_CODE and QEMU_UEFI_VARS".into()
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn good() -> String {
        MARKERS.join("\r\n") + "\r\n"
    }

    #[test]
    fn only_debug_exit_with_complete_transcript_succeeds() {
        assert!(validate_logs(&good(), "", Some(33), false).is_ok());
        assert!(validate_logs("", &good(), Some(33), false).is_ok());
        for code in [None, Some(0), Some(1), Some(35), Some(124)] {
            assert!(validate_logs(&good(), "", code, false).is_err());
        }
    }

    #[test]
    fn timeout_cannot_be_hidden_by_success_markers() {
        assert!(validate_logs(&good(), "", Some(33), true).is_err());
    }

    #[test]
    fn every_marker_is_required_in_order_on_one_channel() {
        for marker in MARKERS {
            assert!(validate_logs(&good().replace(marker, ""), "", Some(33), false).is_err());
        }
        assert!(validate_logs(
            &MARKERS.iter().rev().copied().collect::<Vec<_>>().join("\n"),
            "",
            Some(33),
            false
        )
        .is_err());
        assert!(validate_logs(
            &MARKERS[..4].join("\n"),
            &MARKERS[4..].join("\n"),
            Some(33),
            false
        )
        .is_err());
        assert!(validate_logs(
            &(good().replace("QOS_VERIFY COMPLETE v3", "noise QOS_VERIFY COMPLETE v3")),
            "",
            Some(33),
            false
        )
        .is_err());
    }

    #[test]
    fn errors_on_either_channel_override_a_complete_transcript() {
        for error in [
            "PANIC: allocation",
            "EXCEPTION: PAGE FAULT",
            "QOS_VERIFY FAIL wx",
        ] {
            assert!(validate_logs(&good(), error, Some(33), false).is_err());
            assert!(validate_logs(error, &good(), Some(33), false).is_err());
        }
    }

    #[test]
    fn arguments_reject_invalid_or_unbounded_timeouts() {
        for value in ["0", "3601", "-1", "invalid"] {
            assert!(Options::parse(["--timeout-secs".into(), value.into()]).is_err());
        }
        assert!(Options::parse(["--image".into()]).is_err());
        assert!(Options::parse(["--cpu".into()]).is_err());
        assert!(Options::parse(["--unknown".into()]).is_err());
        assert!(Options::parse(["--help".into()]).unwrap().is_none());
    }

    #[test]
    fn timed_out_process_is_killed_and_reaped() {
        let mut child = Command::new(env::current_exe().unwrap())
            .args(["--ignored", "--exact", "tests::sleeping_child"])
            .env("QOS_VERIFY_TEST_CHILD", "1")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .unwrap();
        let (_, timeout) = wait_for_guest(&mut child, Duration::from_millis(100)).unwrap();
        assert!(timeout);
        assert!(child.try_wait().unwrap().is_some());
    }

    #[test]
    #[ignore = "subprocess fixture for the timeout test"]
    fn sleeping_child() {
        if env::var_os("QOS_VERIFY_TEST_CHILD").is_some() {
            thread::sleep(Duration::from_secs(30));
        }
    }
}
