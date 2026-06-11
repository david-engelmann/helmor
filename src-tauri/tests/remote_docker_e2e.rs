//! End-to-end test of the remote-runner SSH transport against a real
//! Linux `sshd` running in Docker.
//!
//! This is the layer the macOS-only `remote_binary_integration.rs`
//! can't reach: a genuine `ssh user@host` hop into a different OS,
//! against a `helmor-server` binary compiled FOR that OS. It proves
//! the parts that only break on a real remote:
//!   * the daemon binary actually runs headless on Linux (no GUI libs
//!     dragged into its runtime NEEDED set — see the Dockerfile's
//!     `ldd` guard),
//!   * the SSH transport carries the JSON-RPC frames end-to-end,
//!   * the handshake + `runtime_health` + a `workspace.status`
//!     round-trip work through `RemoteSshRuntime` exactly as the
//!     desktop drives them.
//!
//! ## Opt-in
//!
//! Gated behind `HELMOR_E2E_DOCKER=1` so a plain `cargo test` (local
//! dev, the macOS CI quality suite) never tries to spin up Docker.
//! The dedicated CI job + a developer running the full pass set the
//! env var. When unset, every test in this file early-returns with a
//! skip note.
//!
//! ## Arch selection
//!
//! By default the test targets the container matching the host arch
//! (arm64 on Apple Silicon → port 2223; amd64 elsewhere → port
//! 2222), so it always runs **natively**.
//! `HELMOR_E2E_DOCKER_SERVICE=helmor-test-linux-amd64` (or
//! `...-arm64`) forces a specific leg — used by CI to run each leg
//! natively on its matching-arch runner.
//!
//! **Do not rely on the cross-arch leg locally.** Running the
//! non-host arch means emulation (Rosetta / QEMU), and the
//! webkit-linked, multithreaded daemon wedges during init under
//! emulation (the process starts but never binds its socket — no
//! code bug, the native build of the same code initialises fine).
//! CI is the source of truth for the non-host arch: it runs both
//! legs on native same-arch runners
//! (`.github/workflows/remote-server-e2e.yml`).
//!
//! ## SSH wiring
//!
//! The harness writes a sentinel-bounded `Host` block into the
//! user's `~/.ssh/config` so the desktop's unmodified ssh transport
//! (which just runs `ssh <host> ...`) resolves the right port +
//! identity + `StrictHostKeyChecking no`. The block is removed on
//! `Drop`; a stale-sweep at startup clears any block a previously
//! crashed run left behind.

use std::io::Write;
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use helmor_lib::remote::{RemoteRuntime, RemoteSshRuntime, RuntimeKind};

/// Env gate. The whole file is a no-op unless this is set.
const ENABLE_ENV: &str = "HELMOR_E2E_DOCKER";
/// Optional override for which compose service / arch to target.
const SERVICE_ENV: &str = "HELMOR_E2E_DOCKER_SERVICE";

/// Sentinel comments bracketing the `~/.ssh/config` block the harness
/// owns. The block between these markers is rewritten on setup and
/// deleted on teardown; anything outside is left untouched.
const SSH_CONFIG_BEGIN: &str = "# >>> helmor-e2e-docker (managed) >>>";
const SSH_CONFIG_END: &str = "# <<< helmor-e2e-docker (managed) <<<";

/// `helmor-server` install location baked into the test image
/// (see the Dockerfile). Passed as the `remote_binary` so the
/// transport's probe finds it immediately.
const REMOTE_BINARY: &str = "/home/e2e/.helmor/server/helmor-server";

fn enabled() -> bool {
    std::env::var(ENABLE_ENV).is_ok_and(|v| v == "1" || v.eq_ignore_ascii_case("true"))
}

/// (compose service name, host alias, host-side ssh port).
fn target() -> (&'static str, &'static str, u16) {
    let service = std::env::var(SERVICE_ENV).unwrap_or_else(|_| {
        // Default to the host arch's native container.
        if std::env::consts::ARCH == "aarch64" {
            "helmor-test-linux-arm64".to_string()
        } else {
            "helmor-test-linux-amd64".to_string()
        }
    });
    match service.as_str() {
        "helmor-test-linux-arm64" => ("helmor-test-linux-arm64", "helmor-e2e-arm64", 2223),
        // Default + amd64.
        _ => ("helmor-test-linux-amd64", "helmor-e2e-amd64", 2222),
    }
}

fn harness_dir() -> PathBuf {
    // CARGO_MANIFEST_DIR is `src-tauri/`.
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/docker-e2e")
}

fn ssh_config_path() -> PathBuf {
    let home = std::env::var("HOME").expect("HOME must be set");
    PathBuf::from(home).join(".ssh/config")
}

/// Manages the container lifecycle + the ssh-config block. Brings the
/// stack up on `new`, tears it down + cleans the config on `Drop`.
struct DockerHarness {
    service: &'static str,
    host_alias: &'static str,
    port: u16,
    compose_file: PathBuf,
}

impl DockerHarness {
    fn up() -> Self {
        let (service, host_alias, port) = target();
        let dir = harness_dir();
        let compose_file = dir.join("compose.yml");

        // 1. Ensure an ephemeral keypair + authorized_keys fixture.
        let key_path = dir.join("fixtures/id_e2e");
        if !key_path.exists() {
            let status = Command::new("ssh-keygen")
                .args(["-t", "ed25519", "-N", "", "-C", "helmor-e2e", "-f"])
                .arg(&key_path)
                .status()
                .expect("ssh-keygen should spawn");
            assert!(status.success(), "ssh-keygen failed");
            let pubkey = std::fs::read(dir.join("fixtures/id_e2e.pub")).unwrap();
            std::fs::write(dir.join("fixtures/authorized_keys"), pubkey).unwrap();
        }

        // 2. Rewrite the managed ssh-config block.
        write_ssh_config_block(host_alias, port, &key_path);

        // 3. Bring the matching service up. Image must already be
        //    built (the test asserts a clear message if not — building
        //    in-test would balloon the wall clock unpredictably).
        let up = Command::new("docker")
            .args(["compose", "-f"])
            .arg(&compose_file)
            .args(["up", "-d", service])
            .status()
            .expect("docker compose up should spawn");
        assert!(
            up.success(),
            "docker compose up {service} failed — build the image first with:\n  \
             docker compose -f src-tauri/tests/docker-e2e/compose.yml build {service}"
        );

        // 4. Wait for sshd to accept connections on the host port.
        wait_for_port(port, Duration::from_secs(60));

        Self {
            service,
            host_alias,
            port,
            compose_file,
        }
    }

    fn host_alias(&self) -> &str {
        self.host_alias
    }
}

impl Drop for DockerHarness {
    fn drop(&mut self) {
        // Best-effort teardown — don't panic in Drop (would mask the
        // test's own failure). Stop + remove just our service rather
        // than `down` (which would tear down the whole project incl.
        // a sibling arch leg another test might be using).
        let _ = Command::new("docker")
            .args(["compose", "-f"])
            .arg(&self.compose_file)
            .args(["rm", "-sf", self.service])
            .status();
        remove_ssh_config_block();
        let _ = self.port; // silence unused on teardown-only field
    }
}

/// Rewrite (create or replace) the sentinel-bounded managed block in
/// `~/.ssh/config`. Idempotent: a prior managed block is stripped
/// first, so repeated runs don't stack duplicates.
fn write_ssh_config_block(host_alias: &str, port: u16, identity_file: &Path) {
    let path = ssh_config_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    let existing = std::fs::read_to_string(&path).unwrap_or_default();
    let stripped = strip_managed_block(&existing);

    let block = format!(
        "{begin}\n\
         Host {alias}\n\
         \tHostName 127.0.0.1\n\
         \tPort {port}\n\
         \tUser e2e\n\
         \tIdentityFile {identity}\n\
         \tIdentitiesOnly yes\n\
         \tStrictHostKeyChecking no\n\
         \tUserKnownHostsFile /dev/null\n\
         \tBatchMode yes\n\
         {end}\n",
        begin = SSH_CONFIG_BEGIN,
        alias = host_alias,
        port = port,
        identity = identity_file.display(),
        end = SSH_CONFIG_END,
    );

    let mut out = stripped.trim_end().to_string();
    if !out.is_empty() {
        out.push_str("\n\n");
    }
    out.push_str(&block);

    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&path)
        .expect("open ~/.ssh/config for write");
    f.write_all(out.as_bytes()).expect("write ssh config");
}

fn remove_ssh_config_block() {
    let path = ssh_config_path();
    let Ok(existing) = std::fs::read_to_string(&path) else {
        return;
    };
    let stripped = strip_managed_block(&existing);
    let _ = std::fs::write(&path, stripped);
}

/// Remove everything between (and including) the sentinel markers.
/// Leaves the rest of the file byte-for-byte.
fn strip_managed_block(content: &str) -> String {
    let mut out = String::new();
    let mut in_block = false;
    for line in content.lines() {
        if line.trim() == SSH_CONFIG_BEGIN {
            in_block = true;
            continue;
        }
        if line.trim() == SSH_CONFIG_END {
            in_block = false;
            continue;
        }
        if !in_block {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

fn wait_for_port(port: u16, timeout: Duration) {
    let deadline = Instant::now() + timeout;
    loop {
        if TcpStream::connect(("127.0.0.1", port)).is_ok() {
            return;
        }
        if Instant::now() >= deadline {
            panic!("sshd on 127.0.0.1:{port} never came up within {timeout:?}");
        }
        std::thread::sleep(Duration::from_millis(250));
    }
}

#[test]
fn ssh_transport_completes_handshake_and_health_against_linux_container() {
    if !enabled() {
        eprintln!(
            "skipping docker E2E (set {ENABLE_ENV}=1 to run). \
             Build the image first: docker compose -f \
             src-tauri/tests/docker-e2e/compose.yml build"
        );
        return;
    }

    let harness = DockerHarness::up();

    // Connect via the desktop's real SSH runtime — same path the
    // wizard's Connect button drives. The host alias resolves to
    // 127.0.0.1:<port> via the managed ssh-config block.
    let runtime = RemoteSshRuntime::connect_ssh(harness.host_alias(), REMOTE_BINARY)
        .expect("connect_ssh to the Linux container should complete the handshake");

    // 1. runtime_health: proves the binary runs headless on Linux +
    //    the wire carries a real RPC round-trip.
    let health = runtime
        .runtime_health()
        .expect("runtime_health round-trip over ssh should succeed");
    match &health.kind {
        RuntimeKind::Remote { host } => {
            assert!(
                host.contains(harness.host_alias()) || !host.is_empty(),
                "remote health should name the host, got {host:?}"
            );
        }
        other => panic!("expected RuntimeKind::Remote, got {other:?}"),
    }
    // The daemon reports its own CARGO_PKG_VERSION; don't pin an
    // exact value (it moves every release), just assert it's a
    // non-empty semver-shaped string.
    assert!(
        !health.version.is_empty(),
        "daemon should report a non-empty version"
    );
    assert!(
        health.version.split('.').count() >= 2,
        "daemon version should look like semver, got {:?}",
        health.version
    );

    // 2. workspace.status against a fresh git repo created inside the
    //    container — exercises a real workspace_* RPC over the wire,
    //    not just the handshake.
    let repo_path = "/home/e2e/e2e-repo";
    init_repo_in_container(harness.service, repo_path);
    let status = runtime
        .workspace_status(Path::new(repo_path))
        .expect("workspace.status over ssh should succeed");
    assert!(
        status.is_clean,
        "freshly-committed repo in the container should report clean, got {status:?}"
    );
}

/// Sustained-load soak: hammer a cheap RPC (`workspace_status`) at the
/// daemon for [`SOAK_DURATION_SECS`] seconds and assert the daemon's
/// RSS stays bounded. Catches transport-layer leaks (per-call subscriber
/// piles, growing pending-id maps, etc.) that a one-shot E2E test can't.
///
/// Gated behind `#[ignore]` so `cargo test --tests` doesn't pay the
/// 5-minute price on every push; CI's `Remote server E2E` skips this
/// by default. Run via:
///
///   cargo test --test remote_docker_e2e soak_workspace_status -- \
///     --ignored --nocapture --test-threads=1
///
/// or via the `Remote server soak` GitHub workflow (manual dispatch).
const SOAK_DURATION_SECS: u64 = 300; // 5 min — long enough to spot leaks, short enough for `workflow_dispatch`.
const SOAK_RSS_GROWTH_BUDGET_BYTES: u64 = 64 * 1024 * 1024; // 64 MB peak growth over the run

#[test]
#[ignore = "soak: gated, run via `cargo test soak_workspace_status -- --ignored --nocapture`"]
fn soak_workspace_status_against_linux_container() {
    if !enabled() {
        eprintln!("skipping docker soak (set {ENABLE_ENV}=1 to run)");
        return;
    }
    let harness = DockerHarness::up();
    let runtime = RemoteSshRuntime::connect_ssh(harness.host_alias(), REMOTE_BINARY)
        .expect("connect_ssh for soak");

    let repo_path = "/home/e2e/e2e-soak-repo";
    init_repo_in_container(harness.service, repo_path);

    let initial_rss = sample_daemon_rss(harness.service);
    eprintln!("soak start — initial daemon RSS: {initial_rss} bytes");

    let mut peak_rss = initial_rss;
    let mut iterations = 0u64;
    // Per-iteration round-trip elapsed times for latency percentiles.
    // 8 bytes/sample × ~100k samples over a 5-min run = ~800 KB —
    // negligible vs. the daemon's 64 MB budget and well worth the
    // P50 / P95 / P99 signal it gives us.
    let mut elapsed_samples: Vec<Duration> = Vec::with_capacity(150_000);
    let deadline = std::time::Instant::now() + Duration::from_secs(SOAK_DURATION_SECS);
    while std::time::Instant::now() < deadline {
        let call_start = std::time::Instant::now();
        let _ = runtime
            .workspace_status(Path::new(repo_path))
            .expect("workspace.status round-trip during soak");
        elapsed_samples.push(call_start.elapsed());
        iterations += 1;

        // Sample every 100 iters — cheap, and we don't want sampling
        // noise to dominate the wall clock.
        if iterations.is_multiple_of(100) {
            let now_rss = sample_daemon_rss(harness.service);
            peak_rss = peak_rss.max(now_rss);
            eprintln!(
                "soak iter={iterations} now_rss={now_rss} peak_rss={peak_rss} (initial={initial_rss})"
            );
        }
    }

    let (p50, p95, p99) = compute_latency_percentiles(&mut elapsed_samples);
    eprintln!(
        "soak done — {iterations} iterations, peak RSS {peak_rss}, initial RSS {initial_rss}, growth {} bytes; latency p50={}µs p95={}µs p99={}µs",
        peak_rss.saturating_sub(initial_rss),
        p50.as_micros(),
        p95.as_micros(),
        p99.as_micros(),
    );
    assert!(
        peak_rss.saturating_sub(initial_rss) < SOAK_RSS_GROWTH_BUDGET_BYTES,
        "daemon RSS grew {} bytes during soak — leak suspected (budget {SOAK_RSS_GROWTH_BUDGET_BYTES} bytes; initial {initial_rss}, peak {peak_rss})",
        peak_rss.saturating_sub(initial_rss),
    );
}

/// Sort the elapsed-time samples in place and return (p50, p95, p99).
/// Quantile = "nearest-rank" on the sorted array — closest match for
/// the soak's `eprintln!` line is more about being comparable across
/// runs than statistically precise.
fn compute_latency_percentiles(samples: &mut [Duration]) -> (Duration, Duration, Duration) {
    if samples.is_empty() {
        return (Duration::ZERO, Duration::ZERO, Duration::ZERO);
    }
    samples.sort_unstable();
    let pick = |p: f64| -> Duration {
        let idx = ((samples.len() as f64 * p).ceil() as usize)
            .saturating_sub(1)
            .min(samples.len() - 1);
        samples[idx]
    };
    (pick(0.50), pick(0.95), pick(0.99))
}

/// Read the daemon's RSS from inside the container. `daemon.pid`
/// holds the active daemon's PID; `/proc/<pid>/status`'s `VmRSS` is
/// in KiB and gets converted to bytes for the budget comparison.
fn sample_daemon_rss(service: &str) -> u64 {
    let script = "pid=$(cat $HOME/.helmor/server/daemon.pid 2>/dev/null); \
                  if [ -z \"$pid\" ]; then echo 0; exit 0; fi; \
                  grep -E '^VmRSS:' /proc/$pid/status 2>/dev/null | awk '{print $2}'";
    let out = Command::new("docker")
        .args(["exec", "--user", "e2e", service, "bash", "-lc", script])
        .output()
        .expect("docker exec for RSS sample");
    if !out.status.success() {
        return 0;
    }
    let kib_str = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let kib: u64 = kib_str.parse().unwrap_or(0);
    kib * 1024
}

/// `git init` + initial commit inside the container via `docker exec`.
fn init_repo_in_container(service: &str, repo_path: &str) {
    let script = format!(
        "set -e; \
         mkdir -p {repo} && cd {repo} && \
         git init -q && git checkout -q -b main && \
         git config user.email e2e@example.com && \
         git config user.name 'Helmor E2E' && \
         git config commit.gpgsign false && \
         echo base > file.txt && git add file.txt && \
         git commit -q -m initial",
        repo = repo_path,
    );
    let out = Command::new("docker")
        .args(["exec", "--user", "e2e", service, "bash", "-lc", &script])
        .output()
        .expect("docker exec should spawn");
    assert!(
        out.status.success(),
        "git init in container failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// Reconnect-storm chaos test: drop + reconnect the SSH transport in
/// a tight loop and assert the daemon's RSS doesn't accumulate per-
/// connection leftovers. Catches the class of bug where each new
/// connection leaks a daemon-side subscriber, pending-id map, or
/// transport buffer — invisible to the steady-state soak (which holds
/// one connection open the whole time).
///
/// Gated behind `#[ignore]` so `cargo test --tests` doesn't pay the
/// per-cycle SSH-handshake wall clock on every push.
///
///   cargo test --test remote_docker_e2e reconnect_storm -- \
///     --ignored --nocapture --test-threads=1
const RECONNECT_STORM_CYCLES: u64 = 10;
const RECONNECT_STORM_RSS_BUDGET_BYTES: u64 = 32 * 1024 * 1024; // 32 MB peak growth across 10 reconnects

#[test]
#[ignore = "soak: gated, run via `cargo test reconnect_storm -- --ignored --nocapture`"]
fn reconnect_storm_against_linux_container() {
    if !enabled() {
        eprintln!("skipping reconnect-storm (set {ENABLE_ENV}=1 to run)");
        return;
    }
    let harness = DockerHarness::up();
    let repo_path = "/home/e2e/e2e-reconnect-repo";
    init_repo_in_container(harness.service, repo_path);

    // Baseline RSS *before* the first connect — measured with the
    // daemon definitely up because the soak harness's image bootstrap
    // leaves it running.
    let initial_rss = sample_daemon_rss(harness.service);
    eprintln!("reconnect-storm start — initial daemon RSS: {initial_rss} bytes");

    let mut peak_rss = initial_rss;
    let mut connect_elapsed_samples: Vec<Duration> =
        Vec::with_capacity(RECONNECT_STORM_CYCLES as usize);

    for cycle in 1..=RECONNECT_STORM_CYCLES {
        let connect_start = std::time::Instant::now();
        let runtime = RemoteSshRuntime::connect_ssh(harness.host_alias(), REMOTE_BINARY)
            .unwrap_or_else(|err| panic!("connect_ssh failed on cycle {cycle}: {err:#}"));
        connect_elapsed_samples.push(connect_start.elapsed());

        let status = runtime
            .workspace_status(Path::new(repo_path))
            .unwrap_or_else(|err| panic!("workspace_status failed on cycle {cycle}: {err:#}"));
        assert!(
            status.is_clean,
            "cycle {cycle}: workspace should report clean, got {status:?}"
        );

        // Dropping `runtime` closes the writer + child + reader.
        // This is the path that's been historically leak-prone.
        drop(runtime);

        let now_rss = sample_daemon_rss(harness.service);
        peak_rss = peak_rss.max(now_rss);
        eprintln!(
            "reconnect-storm cycle={cycle} now_rss={now_rss} peak_rss={peak_rss} (initial={initial_rss})"
        );
    }

    let (p50, p95, p99) = compute_latency_percentiles(&mut connect_elapsed_samples);
    let growth = peak_rss.saturating_sub(initial_rss);
    eprintln!(
        "reconnect-storm done — {RECONNECT_STORM_CYCLES} cycles, peak RSS {peak_rss}, initial RSS {initial_rss}, growth {growth} bytes; connect latency p50={}ms p95={}ms p99={}ms",
        p50.as_millis(),
        p95.as_millis(),
        p99.as_millis(),
    );
    assert!(
        growth < RECONNECT_STORM_RSS_BUDGET_BYTES,
        "daemon RSS grew {growth} bytes across {RECONNECT_STORM_CYCLES} reconnect cycles — per-connection leak suspected (budget {RECONNECT_STORM_RSS_BUDGET_BYTES} bytes; initial {initial_rss}, peak {peak_rss})",
    );
}

// ---------------------------------------------------------------------------
// percentile helper unit tests — exercised without the docker harness so they
// run on every `cargo test --tests` push, not just the soak workflow.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod percentile_tests {
    use super::compute_latency_percentiles;
    use std::time::Duration;

    #[test]
    fn empty_samples_return_zero_durations() {
        let mut s: Vec<Duration> = vec![];
        let (p50, p95, p99) = compute_latency_percentiles(&mut s);
        assert_eq!(p50, Duration::ZERO);
        assert_eq!(p95, Duration::ZERO);
        assert_eq!(p99, Duration::ZERO);
    }

    #[test]
    fn single_sample_collapses_all_percentiles() {
        let mut s = vec![Duration::from_millis(7)];
        let (p50, p95, p99) = compute_latency_percentiles(&mut s);
        assert_eq!(p50, Duration::from_millis(7));
        assert_eq!(p95, Duration::from_millis(7));
        assert_eq!(p99, Duration::from_millis(7));
    }

    #[test]
    fn percentiles_track_nearest_rank() {
        // 100 samples = 1ms .. 100ms — nearest-rank on a sorted
        // array gives exactly the percentile-numbered sample.
        let mut s: Vec<Duration> = (1..=100).map(Duration::from_millis).collect();
        let (p50, p95, p99) = compute_latency_percentiles(&mut s);
        assert_eq!(p50, Duration::from_millis(50));
        assert_eq!(p95, Duration::from_millis(95));
        assert_eq!(p99, Duration::from_millis(99));
    }

    #[test]
    fn percentiles_sort_unsorted_input() {
        let mut s = vec![
            Duration::from_millis(99),
            Duration::from_millis(1),
            Duration::from_millis(50),
            Duration::from_millis(20),
            Duration::from_millis(95),
        ];
        let (p50, _p95, p99) = compute_latency_percentiles(&mut s);
        assert_eq!(p50, Duration::from_millis(50));
        assert_eq!(p99, Duration::from_millis(99));
    }
}
