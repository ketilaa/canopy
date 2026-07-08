use anyhow::{Context, Result};
use canopy_llm::LlmClient;

pub(crate) fn uuid_v4() -> String {
    let mut buf = [0u8; 16];
    if let Ok(mut f) = std::fs::File::open("/dev/urandom") {
        use std::io::Read;
        let _ = f.read_exact(&mut buf);
    }
    buf[6] = (buf[6] & 0x0f) | 0x40;
    buf[8] = (buf[8] & 0x3f) | 0x80;
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        buf[0], buf[1], buf[2], buf[3], buf[4], buf[5], buf[6], buf[7],
        buf[8], buf[9], buf[10], buf[11], buf[12], buf[13], buf[14], buf[15]
    )
}

pub(crate) fn iso_now() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let (y, mo, d, h, mi, s) = epoch_to_parts(secs);
    format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z", y, mo, d, h, mi, s)
}

fn epoch_to_parts(secs: u64) -> (u64, u64, u64, u64, u64, u64) {
    let s = secs % 60; let secs = secs / 60;
    let mi = secs % 60; let secs = secs / 60;
    let h = secs % 24; let days = secs / 24;
    let mut year = 1970u64;
    let mut rem = days;
    loop {
        let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
        let dy = if leap { 366 } else { 365 };
        if rem < dy { break; }
        rem -= dy; year += 1;
    }
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let months = [31u64,if leap{29}else{28},31,30,31,30,31,31,30,31,30,31];
    let mut mo = 1u64;
    for &days_in_month in &months {
        if rem < days_in_month { break; }
        rem -= days_in_month; mo += 1;
    }
    (year, mo, rem + 1, h, mi, s)
}

pub(crate) fn build_client(agent: &str, debug: bool) -> Result<LlmClient> {
    let client = match canopy_storage::load_config()
        .context("failed to read .canopy/config.yaml")?
    {
        Some(cfg) => {
            let agent_cfg = cfg.for_agent(agent).ok_or_else(|| {
                anyhow::anyhow!(
                    "no LLM config for agent '{}' and no default in .canopy/config.yaml",
                    agent
                )
            })?;
            LlmClient::from_agent_config(&agent_cfg, debug)
        }
        None => LlmClient::default_local(debug),
    };
    // Always log to file; console debug only when --llm-debug is passed.
    Ok(client.with_log_path(".canopy/logs/llm-debug.log"))
}

pub(crate) fn unix_timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .to_string()
}
pub(crate) fn project_name() -> String {
    // Try git remote name first
    if let Ok(output) = std::process::Command::new("git")
        .args(["remote", "get-url", "origin"])
        .output()
    {
        let url = String::from_utf8_lossy(&output.stdout);
        let name = url.trim().trim_end_matches(".git");
        if let Some(part) = name.rsplit('/').next() {
            if !part.is_empty() {
                return part.to_string();
            }
        }
    }
    // Fall back to current directory name
    std::env::current_dir()
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        .unwrap_or_else(|| "project".to_string())
}
