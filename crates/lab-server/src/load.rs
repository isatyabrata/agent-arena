//! Machine load sampler: load average, CPU %, free memory and the training processes.
//!
//! One background thread samples at most every 5 s (a single `ps` call plus cheap
//! reads) and caches the JSON, so viewers polling `/api/load` cost nothing extra.

use serde_json::{json, Value};
use std::process::Command;

/// Training binaries to track, as (name prefix, track).
const TRACKS: [(&str, &str); 7] =
    [("spark", "A"), ("tracka", "A"), ("kairos", "B"), ("maem", "C"), ("scribe", "D"), ("settle", "E"), ("griddyn", "E")];
const KINDS: [&str; 5] = ["train", "ablate", "gru", "eval", "data"];

/// Map a process name like `spark-train` to its track letter.
pub fn track_of(name: &str) -> Option<&'static str> {
    let n = name.to_ascii_lowercase();
    if !KINDS.iter().any(|k| n.contains(k)) {
        return None;
    }
    TRACKS.iter().find(|(p, _)| n.starts_with(p)).map(|(_, t)| *t)
}

/// Parse `ps -axo pcpu=,comm=` output into (total CPU %, training processes).
pub fn parse_ps(out: &str) -> (f64, Vec<Value>) {
    let mut total = 0.0;
    let mut procs: Vec<(f64, String, &'static str)> = vec![];
    for line in out.lines() {
        let line = line.trim_start();
        let Some((cpu, comm)) = line.split_once(char::is_whitespace) else { continue };
        let Ok(cpu) = cpu.parse::<f64>() else { continue };
        total += cpu;
        let name = comm.trim().rsplit('/').next().unwrap_or("");
        if let Some(track) = track_of(name) {
            procs.push((cpu, name.to_string(), track));
        }
    }
    procs.sort_by(|a, b| b.0.total_cmp(&a.0));
    procs.truncate(12);
    (total, procs.into_iter().map(|(c, n, t)| json!({"name": n, "cpu": (c * 10.0).round() / 10.0, "track": t})).collect())
}

fn run(cmd: &str, args: &[&str]) -> String {
    Command::new(cmd).args(args).output().map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default()
}

fn cores() -> usize {
    std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1)
}

/// 1, 5 and 15 minute load averages.
fn loadavg() -> [f64; 3] {
    let text = std::fs::read_to_string("/proc/loadavg").unwrap_or_else(|_| run("sysctl", &["-n", "vm.loadavg"]));
    let v: Vec<f64> = text.split(|c: char| c.is_whitespace() || c == '{' || c == '}').filter_map(|t| t.parse().ok()).collect();
    [*v.first().unwrap_or(&0.0), *v.get(1).unwrap_or(&0.0), *v.get(2).unwrap_or(&0.0)]
}

/// Free memory in MB (free + inactive + speculative pages on macOS, MemAvailable on Linux).
fn free_mb() -> f64 {
    if let Ok(t) = std::fs::read_to_string("/proc/meminfo") {
        let kb = t.lines().find(|l| l.starts_with("MemAvailable:")).and_then(|l| l.split_whitespace().nth(1)).and_then(|v| v.parse::<f64>().ok());
        return kb.unwrap_or(0.0) / 1024.0;
    }
    let out = run("vm_stat", &[]);
    let page: f64 = out.split("page size of ").nth(1).and_then(|s| s.split_whitespace().next()).and_then(|s| s.parse().ok()).unwrap_or(4096.0);
    let pages: f64 = out
        .lines()
        .filter(|l| l.starts_with("Pages free") || l.starts_with("Pages inactive") || l.starts_with("Pages speculative"))
        .filter_map(|l| l.rsplit(':').next()?.trim().trim_end_matches('.').parse::<f64>().ok())
        .sum();
    pages * page / 1048576.0
}

/// Heavy-job slots held under /tmp/lab-heavy (scripts/heavy.sh): [{slot, pid, cmd}], live owners only.
fn heavy_slots() -> Vec<serde_json::Value> {
    let mut v = vec![];
    let Ok(rd) = std::fs::read_dir("/tmp/lab-heavy") else { return v };
    for e in rd.flatten() {
        let d = e.path();
        let rd = |f: &str| std::fs::read_to_string(d.join(f)).unwrap_or_default().trim().to_string();
        let pid: i64 = rd("pid").parse().unwrap_or(0);
        if pid <= 0 || run("ps", &["-p", &pid.to_string(), "-o", "pid="]).trim().is_empty() {
            continue; // owner died; heavy.sh frees the slot itself
        }
        let cmd: String = rd("cmd").chars().take(80).collect();
        v.push(json!({"slot": e.file_name().to_string_lossy(), "pid": pid, "cmd": cmd}));
    }
    v.sort_by_key(|x| x["slot"].as_str().unwrap_or("").to_string());
    v
}

/// Take one sample as the JSON served on `/api/load`.
pub fn sample() -> String {
    let n = cores();
    let (cpu_sum, procs) = parse_ps(&run("ps", &["-axo", "pcpu=,comm="]));
    let load = loadavg();
    let cpu_pct = (cpu_sum / n as f64).min(100.0);
    json!({
        "load": load, "cores": n, "cpu_pct": (cpu_pct * 10.0).round() / 10.0,
        "free_mb": free_mb().round(), "red": load[0] > 2.0 * n as f64, "procs": procs, "heavy": heavy_slots(),
    })
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_binaries_to_tracks() {
        assert_eq!(track_of("spark-train"), Some("A"));
        assert_eq!(track_of("tracka-gru"), Some("A"));
        assert_eq!(track_of("kairos-train"), Some("B"));
        assert_eq!(track_of("maem-ablate"), Some("C"));
        assert_eq!(track_of("scribe-train"), Some("D"));
        assert_eq!(track_of("settle-train"), Some("E"));
        assert_eq!(track_of("spark-model"), None);
        assert_eq!(track_of("bash"), None);
    }

    #[test]
    fn parses_ps_output() {
        let out = " 12.5 /usr/bin/foo\n 99.0 /x/target/release/spark-train\n  3.0 /x/maem-ablate\n bad line\n";
        let (total, procs) = parse_ps(out);
        assert!((total - 114.5).abs() < 1e-9);
        assert_eq!(procs.len(), 2);
        assert_eq!(procs[0]["name"], "spark-train");
        assert_eq!(procs[0]["track"], "A");
        assert_eq!(procs[1]["track"], "C");
    }

    #[test]
    fn sample_is_valid_json() {
        let v: Value = serde_json::from_str(&sample()).unwrap();
        assert!(v["cores"].as_u64().unwrap() >= 1);
        assert!(v["load"].is_array());
    }
}
