use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProbeStatus {
    Ok,
    PermissionDenied,
    Gone,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessProbe {
    pub pid: u32,
    pub command: String,
    pub cwd: Option<String>,
    pub cpu_milli_percent: u32,
    pub memory_bytes: u64,
    pub status: ProbeStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessCandidate {
    pub pid: u32,
    pub command: String,
    pub cwd: Option<String>,
    pub cpu_milli_percent: u32,
    pub memory_bytes: u64,
    pub status: ProbeStatus,
    pub pid_reused: bool,
}

pub fn is_training_like_command(command: &str) -> bool {
    let lower = command.to_ascii_lowercase();
    [
        "python",
        "torch",
        "trainer",
        "train",
        "accelerate",
        "deepspeed",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

pub fn collect_training_candidates_from_probes(probes: &[ProcessProbe]) -> Vec<ProcessCandidate> {
    let mut by_pid: HashMap<u32, ProcessCandidate> = HashMap::new();

    for probe in probes {
        if probe.status == ProbeStatus::PermissionDenied {
            by_pid.entry(probe.pid).or_insert(ProcessCandidate {
                pid: probe.pid,
                command: probe.command.clone(),
                cwd: probe.cwd.clone(),
                cpu_milli_percent: probe.cpu_milli_percent,
                memory_bytes: probe.memory_bytes,
                status: ProbeStatus::PermissionDenied,
                pid_reused: false,
            });
            continue;
        }

        if !is_training_like_command(&probe.command) {
            continue;
        }

        let entry = by_pid.entry(probe.pid).or_insert(ProcessCandidate {
            pid: probe.pid,
            command: probe.command.clone(),
            cwd: probe.cwd.clone(),
            cpu_milli_percent: probe.cpu_milli_percent,
            memory_bytes: probe.memory_bytes,
            status: probe.status.clone(),
            pid_reused: false,
        });

        if entry.command != probe.command {
            entry.pid_reused = true;
            entry.command = probe.command.clone();
            entry.cwd = probe.cwd.clone();
        }
        entry.cpu_milli_percent = probe.cpu_milli_percent;
        entry.memory_bytes = probe.memory_bytes;
        entry.status = probe.status.clone();
    }

    let mut values = by_pid.into_values().collect::<Vec<_>>();
    values.sort_by(|left, right| left.pid.cmp(&right.pid));
    values
}

pub fn discover_training_like_processes() -> Vec<ProcessCandidate> {
    let mut sys = sysinfo::System::new_all();
    sys.refresh_all();

    let probes = sys
        .processes()
        .iter()
        .map(|(pid, process)| ProcessProbe {
            pid: pid.as_u32(),
            command: if process.cmd().is_empty() {
                process.name().to_string_lossy().to_string()
            } else {
                process
                    .cmd()
                    .iter()
                    .map(|part| part.to_string_lossy().to_string())
                    .collect::<Vec<_>>()
                    .join(" ")
            },
            cwd: process.cwd().map(|path| path.to_string_lossy().to_string()),
            cpu_milli_percent: (process.cpu_usage() * 10.0) as u32,
            memory_bytes: process.memory(),
            status: ProbeStatus::Ok,
        })
        .collect::<Vec<_>>();

    collect_training_candidates_from_probes(&probes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_training_like_commands() {
        assert!(is_training_like_command("python train.py"));
        assert!(is_training_like_command("accelerate launch script.py"));
        assert!(!is_training_like_command("bash"));
    }

    #[test]
    fn keeps_permission_denied_entry_without_panic() {
        let probes = vec![ProcessProbe {
            pid: 10,
            command: "python secure.py".to_string(),
            cwd: None,
            cpu_milli_percent: 0,
            memory_bytes: 0,
            status: ProbeStatus::PermissionDenied,
        }];

        let out = collect_training_candidates_from_probes(&probes);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].status, ProbeStatus::PermissionDenied);
    }

    #[test]
    fn marks_pid_reuse_when_command_changes() {
        let probes = vec![
            ProcessProbe {
                pid: 42,
                command: "python train_a.py".to_string(),
                cwd: Some("/tmp/a".to_string()),
                cpu_milli_percent: 10,
                memory_bytes: 11,
                status: ProbeStatus::Ok,
            },
            ProcessProbe {
                pid: 42,
                command: "python train_b.py".to_string(),
                cwd: Some("/tmp/b".to_string()),
                cpu_milli_percent: 12,
                memory_bytes: 13,
                status: ProbeStatus::Ok,
            },
        ];

        let out = collect_training_candidates_from_probes(&probes);
        assert_eq!(out.len(), 1);
        assert!(out[0].pid_reused);
        assert_eq!(out[0].command, "python train_b.py");
    }
}
