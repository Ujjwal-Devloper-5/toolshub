//! Hardware detection module.
//!
//! Gathers CPU model, core count, total RAM, GPU description, and the
//! current CPU architecture. All failures are handled gracefully — the
//! returned [`HardwareInfo`] will contain `None` or zero values where
//! detection is unavailable.

use toolshub_core::types::HardwareInfo;
use tokio::process::Command;
use tracing::{debug, warn};

/// Detect hardware information for the current machine.
///
/// Never returns an error; instead uses sensible defaults when individual
/// probes fail.
pub async fn detect_hardware() -> HardwareInfo {
    let arch = std::env::consts::ARCH.to_string();

    let (cpu_model, cpu_cores) = detect_cpu().await;
    let ram_bytes = detect_ram().await;
    let gpu = detect_gpu().await;

    debug!(
        arch = %arch,
        cpu_model = ?cpu_model,
        cpu_cores = cpu_cores,
        ram_bytes = ram_bytes,
        gpu = ?gpu,
        "Hardware detection complete"
    );

    HardwareInfo {
        architecture: arch,
        cpu_brand: cpu_model.unwrap_or_else(|| "Unknown CPU".to_string()),
        cpu_cores: cpu_cores as u32,
        total_ram_mb: ram_bytes / 1024 / 1024,
        gpu_info: gpu,
    }
}

// ── CPU ───────────────────────────────────────────────────────────────────────

/// Returns `(model_name, core_count)`.
async fn detect_cpu() -> (Option<String>, usize) {
    #[cfg(target_os = "linux")]
    return detect_cpu_linux().await;

    #[cfg(target_os = "macos")]
    return detect_cpu_macos().await;

    #[cfg(target_os = "windows")]
    return detect_cpu_windows().await;

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    (None, 0)
}

#[cfg(target_os = "linux")]
async fn detect_cpu_linux() -> (Option<String>, usize) {
    use tokio::fs;

    let contents = match fs::read_to_string("/proc/cpuinfo").await {
        Ok(c) => c,
        Err(e) => {
            warn!(error = %e, "Failed to read /proc/cpuinfo");
            return (None, 0);
        }
    };

    let model = contents
        .lines()
        .find(|l| l.starts_with("model name"))
        .and_then(|l| l.split_once(':').map(|x| x.1))
        .map(|s| s.trim().to_string());

    let cores = contents
        .lines()
        .filter(|l| l.starts_with("processor"))
        .count();

    (model, cores)
}

#[cfg(target_os = "macos")]
async fn detect_cpu_macos() -> (Option<String>, usize) {
    let model = sysctl_string("machdep.cpu.brand_string")
        .await
        .or_else(|| None);

    let cores = sysctl_usize("hw.logicalcpu").await.unwrap_or(0);

    (model, cores)
}

#[cfg(target_os = "windows")]
async fn detect_cpu_windows() -> (Option<String>, usize) {
    // Query WMI via wmic for CPU name and core count.
    let model = wmic_query("cpu", "Name").await;
    let cores_str = wmic_query("cpu", "NumberOfLogicalProcessors").await;
    let cores = cores_str
        .as_deref()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(0);
    (model, cores)
}

// ── RAM ───────────────────────────────────────────────────────────────────────

/// Returns total physical RAM in bytes, or `0` on failure.
async fn detect_ram() -> u64 {
    #[cfg(target_os = "linux")]
    return detect_ram_linux().await;

    #[cfg(target_os = "macos")]
    return detect_ram_macos().await;

    #[cfg(target_os = "windows")]
    return detect_ram_windows().await;

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    0
}

#[cfg(target_os = "linux")]
async fn detect_ram_linux() -> u64 {
    use tokio::fs;

    let contents = match fs::read_to_string("/proc/meminfo").await {
        Ok(c) => c,
        Err(e) => {
            warn!(error = %e, "Failed to read /proc/meminfo");
            return 0;
        }
    };

    // MemTotal:        32768000 kB
    contents
        .lines()
        .find(|l| l.starts_with("MemTotal:"))
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|s| s.parse::<u64>().ok())
        .map(|kb| kb * 1024) // convert kB → bytes
        .unwrap_or(0)
}

#[cfg(target_os = "macos")]
async fn detect_ram_macos() -> u64 {
    sysctl_usize("hw.memsize").await.unwrap_or(0) as u64
}

#[cfg(target_os = "windows")]
async fn detect_ram_windows() -> u64 {
    let raw = wmic_query("computersystem", "TotalPhysicalMemory").await;
    raw.as_deref()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0)
}

// ── GPU ───────────────────────────────────────────────────────────────────────

/// Returns a human-readable GPU description, or `None` if unavailable.
async fn detect_gpu() -> Option<String> {
    #[cfg(target_os = "linux")]
    return detect_gpu_linux().await;

    #[cfg(target_os = "macos")]
    return detect_gpu_macos().await;

    #[cfg(target_os = "windows")]
    return detect_gpu_windows().await;

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    None
}

#[cfg(target_os = "linux")]
async fn detect_gpu_linux() -> Option<String> {
    // Prefer lspci for detailed GPU information.
    let lspci = Command::new("lspci")
        .output()
        .await;

    if let Ok(out) = lspci {
        if out.status.success() {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let gpu_line = stdout
                .lines()
                .find(|l| {
                    let lower = l.to_lowercase();
                    lower.contains("vga") || lower.contains("3d controller") || lower.contains("display controller")
                })
                .map(|l| {
                    // Strip the PCI address prefix and extract description.
                    l.splitn(2, ':')
                        .last()
                        .unwrap_or(l)
                        .trim()
                        .to_string()
                });

            if gpu_line.is_some() {
                return gpu_line;
            }
        }
    }

    // Fallback: glxinfo for Mesa/OpenGL description.
    let glxinfo = Command::new("glxinfo")
        .arg("-B")
        .output()
        .await;

    if let Ok(out) = glxinfo {
        if out.status.success() {
            let stdout = String::from_utf8_lossy(&out.stdout);
            return stdout
                .lines()
                .find(|l| l.contains("OpenGL renderer"))
                .and_then(|l| l.split_once(':').map(|x| x.1))
                .map(|s| s.trim().to_string());
        }
    }

    warn!("GPU detection unavailable — lspci and glxinfo not found or failed");
    None
}

#[cfg(target_os = "macos")]
async fn detect_gpu_macos() -> Option<String> {
    let out = Command::new("system_profiler")
        .arg("SPDisplaysDataType")
        .output()
        .await;

    match out {
        Ok(o) if o.status.success() => {
            let stdout = String::from_utf8_lossy(&o.stdout);
            // Find the first "Chipset Model:" line.
            stdout
                .lines()
                .find(|l| l.contains("Chipset Model:"))
                .and_then(|l| l.splitn(2, ':').nth(1))
                .map(|s| s.trim().to_string())
        }
        Ok(o) => {
            warn!(stderr = %String::from_utf8_lossy(&o.stderr), "system_profiler failed");
            None
        }
        Err(e) => {
            warn!(error = %e, "Failed to run system_profiler");
            None
        }
    }
}

#[cfg(target_os = "windows")]
async fn detect_gpu_windows() -> Option<String> {
    wmic_query("path win32_VideoController", "Name").await
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Run `sysctl -n <key>` and return the trimmed stdout string.
#[cfg(target_os = "macos")]
async fn sysctl_string(key: &str) -> Option<String> {
    let out = Command::new("sysctl")
        .args(["-n", key])
        .output()
        .await
        .ok()?;

    if out.status.success() {
        Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
    } else {
        None
    }
}

/// Run `sysctl -n <key>` and parse the result as a `usize`.
#[cfg(target_os = "macos")]
async fn sysctl_usize(key: &str) -> Option<usize> {
    sysctl_string(key).await?.parse().ok()
}

/// Run `wmic <class> get <field> /value` and return the trimmed value.
#[cfg(target_os = "windows")]
async fn wmic_query(class: &str, field: &str) -> Option<String> {
    let out = Command::new("wmic")
        .args([class, "get", field, "/value"])
        .output()
        .await
        .ok()?;

    if out.status.success() {
        let stdout = String::from_utf8_lossy(&out.stdout);
        // Output looks like: "Name=Intel(R) Core(TM) i7\r\r\n"
        for line in stdout.lines() {
            if let Some(rest) = line.strip_prefix(&format!("{field}=")) {
                let value = rest.trim().to_string();
                if !value.is_empty() {
                    return Some(value);
                }
            }
        }
    }
    None
}
