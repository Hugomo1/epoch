use epoch::collectors::process::{
    ProbeStatus, ProcessProbe, collect_training_candidates_from_probes,
};

#[test]
fn process_discovery_finds_training_like_commands() {
    let probes = vec![
        ProcessProbe {
            pid: 101,
            command: "python train.py --epochs 3".to_string(),
            cwd: Some("/tmp/project-a".to_string()),
            cpu_milli_percent: 250,
            memory_bytes: 1_000,
            status: ProbeStatus::Ok,
        },
        ProcessProbe {
            pid: 102,
            command: "bash".to_string(),
            cwd: Some("/tmp/project-b".to_string()),
            cpu_milli_percent: 10,
            memory_bytes: 20,
            status: ProbeStatus::Ok,
        },
    ];

    let out = collect_training_candidates_from_probes(&probes);
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].pid, 101);
    assert!(out[0].command.contains("train.py"));
}

#[test]
fn process_discovery_handles_permission_denied_gracefully() {
    let probes = vec![ProcessProbe {
        pid: 501,
        command: "python /restricted/train.py".to_string(),
        cwd: None,
        cpu_milli_percent: 0,
        memory_bytes: 0,
        status: ProbeStatus::PermissionDenied,
    }];

    let out = collect_training_candidates_from_probes(&probes);
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].pid, 501);
    assert_eq!(out[0].status, ProbeStatus::PermissionDenied);
}

#[test]
fn process_discovery_pid_reuse_is_safe() {
    let probes = vec![
        ProcessProbe {
            pid: 9001,
            command: "python train_old.py".to_string(),
            cwd: Some("/tmp/old".to_string()),
            cpu_milli_percent: 120,
            memory_bytes: 300,
            status: ProbeStatus::Ok,
        },
        ProcessProbe {
            pid: 9001,
            command: "python train_new.py".to_string(),
            cwd: Some("/tmp/new".to_string()),
            cpu_milli_percent: 180,
            memory_bytes: 500,
            status: ProbeStatus::Ok,
        },
    ];

    let out = collect_training_candidates_from_probes(&probes);
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].pid, 9001);
    assert!(out[0].pid_reused);
    assert_eq!(out[0].command, "python train_new.py");
}

#[test]
fn process_discovery_unsupported_platform_degrades_cleanly() {
    #[cfg(not(target_os = "linux"))]
    {
        let probes = vec![ProcessProbe {
            pid: 777,
            command: "python train.py".to_string(),
            cwd: None,
            cpu_milli_percent: 0,
            memory_bytes: 0,
            status: ProbeStatus::PermissionDenied,
        }];
        let out = collect_training_candidates_from_probes(&probes);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].status, ProbeStatus::PermissionDenied);
    }

    #[cfg(target_os = "linux")]
    {
        let probes = vec![ProcessProbe {
            pid: 778,
            command: "python train.py".to_string(),
            cwd: Some("/tmp".to_string()),
            cpu_milli_percent: 100,
            memory_bytes: 1000,
            status: ProbeStatus::Ok,
        }];
        let out = collect_training_candidates_from_probes(&probes);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].status, ProbeStatus::Ok);
    }
}
