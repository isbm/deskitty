use std::fs;

pub struct SysInfo {
    pub arch: String,
    pub mem_used_bytes: u64,
    pub mem_total_bytes: u64,
    pub load_1: f32,
    pub load_5: f32,
    pub load_15: f32,
}

pub fn gather() -> Option<SysInfo> {
    let mem = read_meminfo()?;
    let load = read_loadavg()?;
    Some(SysInfo {
        arch: std::env::consts::ARCH.to_string(),
        mem_used_bytes: mem.total.saturating_sub(mem.available),
        mem_total_bytes: mem.total,
        load_1: load.0,
        load_5: load.1,
        load_15: load.2,
    })
}

struct MemInfo {
    total: u64,
    available: u64,
}

fn read_meminfo() -> Option<MemInfo> {
    let content = fs::read_to_string("/proc/meminfo").ok()?;
    let mut total: Option<u64> = None;
    let mut available: Option<u64> = None;
    for line in content.lines() {
        if line.starts_with("MemTotal:") {
            total = Some(parse_kb_value(line));
        } else if line.starts_with("MemAvailable:") {
            available = Some(parse_kb_value(line));
        }
        if total.is_some() && available.is_some() {
            break;
        }
    }
    Some(MemInfo { total: total?, available: available? })
}

fn read_loadavg() -> Option<(f32, f32, f32)> {
    let content = fs::read_to_string("/proc/loadavg").ok()?;
    let parts: Vec<&str> = content.split_whitespace().collect();
    let load_1 = parts.first()?.parse::<f32>().ok()?;
    let load_5 = parts.get(1)?.parse::<f32>().ok()?;
    let load_15 = parts.get(2)?.parse::<f32>().ok()?;
    Some((load_1, load_5, load_15))
}

fn parse_kb_value(line: &str) -> u64 {
    line.split_whitespace()
        .nth(1)
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(0)
        * 1024
}

fn format_bytes(bytes: u64) -> String {
    let tib = 1024u64 * 1024 * 1024 * 1024;
    let gib = 1024u64 * 1024 * 1024;
    if bytes >= tib * 10 {
        format!("{:.0}T", bytes as f64 / tib as f64)
    } else if bytes >= tib {
        format!("{:.1}T", bytes as f64 / tib as f64)
    } else if bytes >= gib {
        format!("{:.1}G", bytes as f64 / gib as f64)
    } else if bytes >= 1024 * 1024 {
        format!("{:.1}M", bytes as f64 / (1024.0 * 1024.0))
    } else if bytes >= 1024 {
        format!("{:.1}K", bytes as f64 / 1024.0)
    } else {
        format!("{bytes}B")
    }
}

pub fn load_string(info: &SysInfo) -> String {
    format!("CPU: {:.2} {:.2} {:.2}", info.load_1, info.load_5, info.load_15)
}

pub fn mem_string(info: &SysInfo) -> String {
    let free = info.mem_total_bytes.saturating_sub(info.mem_used_bytes);
    format!("Total: {} / Used: {} / Free: {}", format_bytes(info.mem_total_bytes), format_bytes(info.mem_used_bytes), format_bytes(free))
}
