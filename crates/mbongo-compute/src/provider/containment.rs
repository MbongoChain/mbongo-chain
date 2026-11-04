//! Runner containment (P2): Linux, cgroup v2, a separate OS identity.
//!
//! The boundary protects the service from its runner. Each job runs as an
//! unprivileged account, with no capabilities, no inherited descriptors, and
//! in its own cgroup, which carries the memory and process limits and kills
//! the whole tree at once.
//!
//! [`Containment::establish`] fails closed: it checks the deployment, then
//! proves it at runtime by spawning through the same path as a job. Anything
//! it cannot prove refuses the start. There is no degraded mode.
//!
//! **Not provided:** network isolation, a private filesystem view, CPU or disk
//! limits, and protection of the host from the service itself. See
//! `docs/runbooks/PROVIDER_CONTAINMENT_LINUX.md`. On any other platform,
//! [`Containment::establish`] refuses.

use std::path::PathBuf;

/// What a provider asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContainmentConfig {
    /// The account a runner runs as. Not root, not the service's.
    pub runner_uid: u32,
    /// Its group, and the only one it holds.
    pub runner_gid: u32,
    /// The delegated cgroup. The service itself sits in its `service` child.
    pub cgroup_root: PathBuf,
    /// Ceiling on a job's memory, whatever its spec declares.
    pub max_memory_mib: u64,
    /// Processes and threads in one job.
    pub max_pids: u64,
    /// Key files: owned by the service, nothing for group or other.
    pub secret_files: Vec<PathBuf>,
    /// State and artifact directories: same rule.
    pub private_dirs: Vec<PathBuf>,
    /// Parent of the job directories: the service's, group `runner_gid`, 2750.
    pub work_dir: PathBuf,
    /// Further paths a runner must not read. Only the runtime probe checks
    /// them, so they need not belong to the service.
    pub denied_paths: Vec<PathBuf>,
}

/// Why isolation was refused. Holds no file content.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("ISOLATION_REFUSED={code}: {detail}")]
pub struct ContainmentError {
    /// Stable, for tests and operators.
    pub code: &'static str,
    /// What was found.
    pub detail: String,
}

/// Proof that [`Containment::establish`] passed. Only it can make one, so a
/// service cannot declare isolation it did not establish.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContainmentProof(());

#[cfg(target_os = "linux")]
pub use linux::{Containment, JobCgroup};
#[cfg(not(target_os = "linux"))]
pub use other::{Containment, JobCgroup};

#[cfg(not(target_os = "linux"))]
mod other {
    use std::io;
    use std::process::Command;

    use super::{ContainmentConfig, ContainmentError, ContainmentProof};

    /// Cannot exist here: [`Containment::establish`] always refuses.
    #[derive(Debug)]
    pub enum Containment {}

    /// Cannot exist here.
    #[derive(Debug)]
    pub enum JobCgroup {}

    impl Containment {
        /// Always refuses: no isolation is claimed on this platform.
        pub fn establish(_config: ContainmentConfig) -> Result<Self, ContainmentError> {
            Err(ContainmentError {
                code: "UNSUPPORTED_PLATFORM",
                detail: "runner containment is Linux only".to_owned(),
            })
        }

        /// Unreachable.
        pub fn proof(&self) -> ContainmentProof {
            match *self {}
        }

        /// Unreachable.
        pub fn swept(&self) -> usize {
            match *self {}
        }

        /// Unreachable.
        pub fn probed_paths(&self) -> usize {
            match *self {}
        }

        /// Unreachable.
        pub fn job(&self, _max_memory_mib: u64) -> io::Result<JobCgroup> {
            match *self {}
        }
    }

    impl JobCgroup {
        /// Unreachable.
        pub fn confine(&self, _command: &mut Command) {
            match *self {}
        }

        /// Unreachable.
        pub fn kill(&self) {
            match *self {}
        }
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use std::ffi::OsString;
    use std::fs::{self, File, OpenOptions};
    use std::io;
    use std::os::fd::AsRawFd;
    use std::os::raw::{c_int, c_long, c_ulong};
    use std::os::unix::fs::MetadataExt;
    use std::os::unix::process::{CommandExt, ExitStatusExt};
    use std::path::{Path, PathBuf};
    use std::process::{Command, Output, Stdio};
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    use super::{ContainmentConfig, ContainmentError, ContainmentProof};

    const CGROUP_MOUNT: &str = "/sys/fs/cgroup";
    const JOB_PREFIX: &str = "job-";
    const PROBE_MEMORY_MIB: u64 = 64;
    const REAP_TIMEOUT: Duration = Duration::from_secs(5);
    const SIGKILL: i32 = 9;

    const PR_SET_NO_NEW_PRIVS: c_int = 38;
    const LINUX_CAPABILITY_VERSION_3: u32 = 0x2008_0522;
    const F_SETFD: c_int = 2;
    /// The same on every architecture.
    const SYS_CLOSE_RANGE: c_long = 436;
    const CLOSE_RANGE_CLOEXEC: c_ulong = 4;
    const FIRST_UNSHARED_FD: c_ulong = 3;
    const LAST_FD: c_ulong = 0xFFFF_FFFF;

    #[repr(C)]
    struct CapHeader {
        version: u32,
        pid: c_int,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct CapData {
        effective: u32,
        permitted: u32,
        inheritable: u32,
    }

    extern "C" {
        fn getuid() -> u32;
        fn getgid() -> u32;
        fn write(fd: c_int, buffer: *const u8, count: usize) -> isize;
        fn setgroups(size: usize, list: *const u32) -> c_int;
        fn setsid() -> c_int;
        fn umask(mask: u32) -> u32;
        fn prctl(option: c_int, ...) -> c_int;
        fn capset(header: *mut CapHeader, data: *const CapData) -> c_int;
        fn fcntl(fd: c_int, command: c_int, ...) -> c_int;
        fn syscall(number: c_long, ...) -> c_long;
    }

    fn checked(ok: bool) -> io::Result<()> {
        if ok {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }

    /// Between fork and exec, after `setuid`: joins the job's cgroup, then
    /// gives up everything the service holds. Runs in a forked copy of a
    /// threaded process, so it only makes system calls and never allocates.
    fn enter(cgroup_procs: c_int) -> io::Result<()> {
        let mut header = CapHeader {
            version: LINUX_CAPABILITY_VERSION_3,
            pid: 0,
        };
        let nothing = [CapData {
            effective: 0,
            permitted: 0,
            inheritable: 0,
        }; 2];
        // SAFETY: each call gets valid pointers for its duration. `0` written
        // to `cgroup.procs` means the calling process; the permission check
        // uses the credentials the descriptor was opened with. Clearing the
        // permitted and inheritable sets clears the ambient set too.
        unsafe {
            checked(write(cgroup_procs, b"0".as_ptr(), 1) == 1)?;
            // Out of the service's session, and away from its terminal.
            checked(setsid() != -1)?;
            checked(setgroups(0, std::ptr::null()) == 0)?;
            checked(
                prctl(
                    PR_SET_NO_NEW_PRIVS,
                    1 as c_ulong,
                    0 as c_ulong,
                    0 as c_ulong,
                    0 as c_ulong,
                ) == 0,
            )?;
            checked(capset(&mut header, nothing.as_ptr()) == 0)?;
            // Whatever opened a descriptor, and however: none survives exec.
            checked(
                syscall(
                    SYS_CLOSE_RANGE,
                    FIRST_UNSHARED_FD,
                    LAST_FD,
                    CLOSE_RANGE_CLOEXEC,
                ) == 0,
            )
        }
    }

    /// Kills everything in a cgroup and removes it. `false` if it stayed.
    fn reap(path: &Path) -> bool {
        let _ = fs::write(path.join("cgroup.kill"), "1");
        let started = Instant::now();
        loop {
            match fs::remove_dir(path) {
                Ok(()) => return true,
                Err(e) if e.kind() == io::ErrorKind::NotFound => return true,
                // Busy until the last process has exited.
                Err(_) if started.elapsed() < REAP_TIMEOUT => {
                    std::thread::sleep(Duration::from_millis(5));
                }
                Err(e) => {
                    log::error!("containment: a job cgroup stayed: {}", e.kind());
                    return false;
                }
            }
        }
    }

    fn refuse(code: &'static str, detail: impl Into<String>) -> ContainmentError {
        ContainmentError {
            code,
            detail: detail.into(),
        }
    }

    /// One job's cgroup. Dropping it kills what is left inside and removes it.
    #[derive(Debug)]
    pub struct JobCgroup {
        path: PathBuf,
        procs: File,
        uid: u32,
        gid: u32,
        survivors: Arc<AtomicBool>,
    }

    impl JobCgroup {
        /// Makes `command` start inside this cgroup, as the runner, with no
        /// capabilities and no inherited descriptors.
        pub fn confine(&self, command: &mut Command) {
            let procs = self.procs.as_raw_fd();
            command.uid(self.uid).gid(self.gid);
            // SAFETY: `enter` is safe to call between fork and exec. `procs`
            // stays open for as long as `self`, which outlives the spawn.
            unsafe {
                command.pre_exec(move || enter(procs));
            }
        }

        /// Kills every process in the job, however it detached itself.
        pub fn kill(&self) {
            if let Err(e) = fs::write(self.path.join("cgroup.kill"), "1") {
                log::error!("containment: cgroup.kill: {}", e.kind());
            }
        }
    }

    impl Drop for JobCgroup {
        fn drop(&mut self) {
            if !reap(&self.path) {
                self.survivors.store(true, Ordering::SeqCst);
            }
        }
    }

    /// Runner containment, proven at startup.
    #[derive(Debug)]
    pub struct Containment {
        config: ContainmentConfig,
        next_job: AtomicU64,
        swept: usize,
        probed_paths: usize,
        proof: ContainmentProof,
        /// A job's processes could not be killed. Nothing else runs beside them.
        survivors: Arc<AtomicBool>,
    }

    fn check_identity(
        config: &ContainmentConfig,
        uid: u32,
        gid: u32,
    ) -> Result<(), ContainmentError> {
        if uid == 0 {
            return Err(refuse(
                "SERVICE_IDENTITY",
                "the service must not run as root",
            ));
        }
        if config.runner_uid == 0 || config.runner_gid == 0 {
            return Err(refuse("RUNNER_IDENTITY", "the runner must not be root"));
        }
        if config.runner_uid == uid || config.runner_gid == gid {
            return Err(refuse(
                "RUNNER_IDENTITY",
                "the runner must not share the service's identity",
            ));
        }
        Ok(())
    }

    fn check_cgroup(config: &ContainmentConfig, uid: u32) -> Result<(), ContainmentError> {
        let unavailable = |detail: &str| refuse("CGROUP_UNAVAILABLE", detail);
        let root = &config.cgroup_root;
        let read = |name: &str| {
            fs::read_to_string(root.join(name))
                .map_err(|e| unavailable(&format!("{name}: {:?}", e.kind())))
        };
        let (controllers, enabled) = (read("cgroup.controllers")?, read("cgroup.subtree_control")?);
        for name in ["memory", "pids"] {
            if !controllers.split_whitespace().any(|c| c == name)
                || !enabled.split_whitespace().any(|c| c == name)
            {
                return Err(unavailable(&format!("controller {name} is not delegated")));
            }
        }
        for name in ["cgroup.kill", "memory.swap.max"] {
            if !root.join(name).exists() {
                return Err(unavailable(&format!("{name} is missing")));
            }
        }
        if fs::metadata(root).map_err(|e| unavailable(&format!("{:?}", e.kind())))?.uid() != uid {
            return Err(unavailable("the cgroup is not delegated to the service"));
        }
        let own = fs::read_to_string("/proc/self/cgroup").unwrap_or_default();
        let own = own.lines().find_map(|l| l.strip_prefix("0::"));
        let own = own.map(|p| Path::new(CGROUP_MOUNT).join(p.trim_start_matches('/')));
        if own.as_deref() != Some(root.join("service").as_path()) {
            return Err(unavailable(
                "the service is not in the delegated service cgroup",
            ));
        }
        Ok(())
    }

    /// Whoever can write to a parent can replace what sits under it, whatever
    /// its own mode. A sticky directory only lets owners do that.
    fn check_parents(path: &Path, uid: u32) -> Result<(), ContainmentError> {
        let wrong = |detail: String| refuse("PROTECTED_PATH", detail);
        let real = fs::canonicalize(path)
            .map_err(|e| wrong(format!("{}: {:?}", path.display(), e.kind())))?;
        for parent in real.ancestors().skip(1) {
            let meta = fs::metadata(parent)
                .map_err(|e| wrong(format!("{}: {:?}", parent.display(), e.kind())))?;
            let open = meta.mode() & 0o022 != 0 && meta.mode() & 0o1000 == 0;
            if open || (meta.uid() != uid && meta.uid() != 0) {
                return Err(wrong(format!(
                    "{}: owner {} mode {:o}",
                    parent.display(),
                    meta.uid(),
                    meta.mode() & 0o7777
                )));
            }
        }
        Ok(())
    }

    fn check_paths(config: &ContainmentConfig, uid: u32) -> Result<(), ContainmentError> {
        let private = |path: &Path, directory: bool| {
            let shown = path.display();
            let meta = fs::symlink_metadata(path)
                .map_err(|e| refuse("PROTECTED_PATH", format!("{shown}: {:?}", e.kind())))?;
            if meta.is_dir() != directory
                || (!directory && !meta.is_file())
                || meta.uid() != uid
                || meta.mode() & 0o077 != 0
            {
                return Err(refuse(
                    "PROTECTED_PATH",
                    format!(
                        "{shown}: owner {} mode {:o}",
                        meta.uid(),
                        meta.mode() & 0o7777
                    ),
                ));
            }
            check_parents(path, uid)
        };
        for file in &config.secret_files {
            private(file, false)?;
        }
        for dir in &config.private_dirs {
            private(dir, true)?;
        }
        let meta = fs::metadata(&config.work_dir)
            .map_err(|e| refuse("WORK_DIR", format!("{:?}", e.kind())))?;
        if !meta.is_dir()
            || meta.uid() != uid
            || meta.gid() != config.runner_gid
            || meta.mode() & 0o7777 != 0o2750
        {
            return Err(refuse(
                "WORK_DIR",
                format!("group {} mode {:o}", meta.gid(), meta.mode() & 0o7777),
            ));
        }
        check_parents(&config.work_dir, uid)
    }

    fn job_entries(dir: &Path) -> Result<Vec<PathBuf>, ContainmentError> {
        let entries = fs::read_dir(dir)
            .map_err(|e| refuse("CGROUP_UNAVAILABLE", format!("{:?}", e.kind())))?;
        Ok(entries
            .filter_map(Result::ok)
            .filter(|e| e.file_name().to_string_lossy().starts_with(JOB_PREFIX))
            .map(|e| e.path())
            .collect())
    }

    /// Kills what a crash left. Called with the state lock held, so these jobs
    /// are never another service's, and before the path checks, so a start
    /// refused for its paths leaves no runner alive.
    fn sweep_jobs(config: &ContainmentConfig) -> Result<usize, ContainmentError> {
        let groups = job_entries(&config.cgroup_root)?;
        for group in &groups {
            if !reap(group) {
                return Err(refuse(
                    "CGROUP_UNAVAILABLE",
                    "a leftover job cannot be killed",
                ));
            }
        }
        Ok(groups.len())
    }

    fn sweep_job_dirs(config: &ContainmentConfig) -> Result<(), ContainmentError> {
        for dir in job_entries(&config.work_dir)? {
            fs::remove_dir_all(dir).map_err(|e| refuse("WORK_DIR", format!("{:?}", e.kind())))?;
        }
        Ok(())
    }

    fn field<'a>(status: &'a str, name: &str) -> Option<&'a str> {
        status
            .lines()
            .find_map(|l| l.strip_prefix(name)?.strip_prefix(':'))
            .map(str::trim)
    }

    impl Containment {
        /// Checks the deployment and proves it, or refuses. Sweeps what a
        /// crash left, so call it with the state lock held.
        pub fn establish(config: ContainmentConfig) -> Result<Self, ContainmentError> {
            // SAFETY: neither call touches memory, and neither can fail.
            let (uid, gid) = unsafe { (getuid(), getgid()) };
            check_identity(&config, uid, gid)?;
            check_cgroup(&config, uid)?;
            let swept = sweep_jobs(&config)?;
            check_paths(&config, uid)?;
            sweep_job_dirs(&config)?;
            // Job directories: read-only for the runner's group, whatever
            // umask the service was started with.
            // SAFETY: touches no memory and cannot fail.
            unsafe {
                umask(0o027);
            }
            let mut containment = Self {
                config,
                next_job: AtomicU64::new(0),
                swept,
                probed_paths: 0,
                proof: ContainmentProof(()),
                survivors: Arc::new(AtomicBool::new(false)),
            };
            containment.probe_identity()?;
            containment.probe_descriptors()?;
            containment.probed_paths = containment.probe_denied()?;
            containment.probe_kill()?;
            Ok(containment)
        }

        /// That this containment was established.
        pub fn proof(&self) -> ContainmentProof {
            self.proof
        }

        /// Leftover jobs killed at startup.
        pub fn swept(&self) -> usize {
            self.swept
        }

        /// Paths the runtime probe found unreadable.
        pub fn probed_paths(&self) -> usize {
            self.probed_paths
        }

        /// A cgroup for one job, with its limits in place. `max_memory_mib`
        /// comes from the job spec, so the provider's ceiling caps it.
        pub fn job(&self, max_memory_mib: u64) -> io::Result<JobCgroup> {
            if self.survivors.load(Ordering::SeqCst) {
                return Err(io::Error::from(io::ErrorKind::ResourceBusy));
            }
            let n = self.next_job.fetch_add(1, Ordering::SeqCst);
            let path = self.config.cgroup_root.join(format!("{JOB_PREFIX}{n}"));
            fs::create_dir(&path)?;
            let opened = OpenOptions::new().write(true).open(path.join("cgroup.procs"));
            let group = match opened {
                Ok(procs) => JobCgroup {
                    path,
                    procs,
                    uid: self.config.runner_uid,
                    gid: self.config.runner_gid,
                    survivors: Arc::clone(&self.survivors),
                },
                Err(e) => {
                    reap(&path);
                    return Err(e);
                }
            };
            let bytes = max_memory_mib
                .min(self.config.max_memory_mib)
                .checked_mul(1 << 20)
                .ok_or_else(|| io::Error::from(io::ErrorKind::InvalidInput))?;
            fs::write(group.path.join("memory.max"), bytes.to_string())?;
            fs::write(group.path.join("memory.swap.max"), "0")?;
            // One process over the limit takes the whole job with it.
            fs::write(group.path.join("memory.oom.group"), "1")?;
            fs::write(
                group.path.join("pids.max"),
                self.config.max_pids.to_string(),
            )?;
            Ok(group)
        }

        fn probe(
            &self,
            program: &str,
            args: &[OsString],
        ) -> Result<(JobCgroup, Output), ContainmentError> {
            let group = self
                .job(PROBE_MEMORY_MIB)
                .map_err(|e| refuse("CGROUP_UNAVAILABLE", format!("job cgroup: {:?}", e.kind())))?;
            let mut command = Command::new(program);
            command.args(args).env_clear().current_dir("/").stdin(Stdio::null());
            group.confine(&mut command);
            let output = command
                .output()
                .map_err(|e| refuse("SPAWN_DENIED", format!("{program}: {:?}", e.kind())))?;
            Ok((group, output))
        }

        /// A contained process reports what it actually is.
        fn probe_identity(&self) -> Result<(), ContainmentError> {
            let args = ["/proc/self/status", "/proc/self/cgroup"].map(OsString::from);
            let (group, output) = self.probe("/bin/cat", &args)?;
            let text = String::from_utf8_lossy(&output.stdout);
            let wrong = |what: &str| refuse("PROBE_IDENTITY", what);
            let ids = |name: &str, id: u32| {
                let id = id.to_string();
                let values: Vec<&str> =
                    field(&text, name).unwrap_or_default().split_whitespace().collect();
                values.len() == 4 && values.iter().all(|v| *v == id)
            };
            if !output.status.success() {
                return Err(wrong("the probe did not run"));
            }
            if !ids("Uid", self.config.runner_uid) || !ids("Gid", self.config.runner_gid) {
                return Err(wrong("wrong uid or gid"));
            }
            if field(&text, "Groups") != Some("") {
                return Err(wrong("supplementary groups remain"));
            }
            for name in ["CapInh", "CapPrm", "CapEff", "CapAmb"] {
                if field(&text, name) != Some("0000000000000000") {
                    return Err(wrong("capabilities remain"));
                }
            }
            if field(&text, "NoNewPrivs") != Some("1") {
                return Err(wrong("no_new_privs is not set"));
            }
            let own = group.path.file_name().unwrap_or_default().to_string_lossy();
            if !text.lines().any(|l| l.starts_with("0::") && l.ends_with(&format!("/{own}"))) {
                return Err(wrong("not in the job cgroup"));
            }
            let expected = (PROBE_MEMORY_MIB.min(self.config.max_memory_mib) << 20).to_string();
            let applied = fs::read_to_string(group.path.join("memory.max")).unwrap_or_default();
            if applied.trim() != expected {
                return Err(wrong("the memory limit was not applied"));
            }
            Ok(())
        }

        /// A descriptor left inheritable on purpose must not arrive.
        fn probe_descriptors(&self) -> Result<(), ContainmentError> {
            let canary = File::open("/dev/null")
                .map_err(|e| refuse("PROBE_DESCRIPTORS", format!("{:?}", e.kind())))?;
            // SAFETY: clears the flags of a descriptor this function owns.
            checked(unsafe { fcntl(canary.as_raw_fd(), F_SETFD, 0 as c_int) } == 0)
                .map_err(|e| refuse("PROBE_DESCRIPTORS", format!("{:?}", e.kind())))?;
            let (_group, output) = self.probe("/bin/ls", &[OsString::from("/proc/self/fd")])?;
            let text = String::from_utf8_lossy(&output.stdout);
            let found: Vec<&str> = text.split_whitespace().collect();
            // 0 to 2, and the directory `ls` is reading.
            if !output.status.success() || found != ["0", "1", "2", "3"] {
                return Err(refuse("PROBE_DESCRIPTORS", format!("{found:?}")));
            }
            Ok(())
        }

        /// The runner tries every protected path. Each must fail with a
        /// permission error: a missing file proves nothing, and refuses too.
        /// What it read is never kept or shown.
        fn probe_denied(&self) -> Result<usize, ContainmentError> {
            let pid = std::process::id();
            let config = &self.config;
            let targets: Vec<OsString> = config
                .secret_files
                .iter()
                .chain(&config.private_dirs)
                .chain(&config.denied_paths)
                .map(OsString::from)
                .chain([
                    OsString::from(format!("/proc/{pid}/environ")),
                    OsString::from(format!("/proc/{pid}/mem")),
                ])
                .collect();
            let (_group, output) = self.probe("/bin/cat", &targets)?;
            if output.status.success() || !output.stdout.is_empty() {
                return Err(refuse(
                    "PROBE_READABLE",
                    "a runner can read a protected path",
                ));
            }
            let text = String::from_utf8_lossy(&output.stderr);
            let lines = text.lines().count();
            let denied = text.lines().filter(|l| l.ends_with("Permission denied")).count();
            if lines != targets.len() || denied != targets.len() {
                return Err(refuse(
                    "PROBE_INCONCLUSIVE",
                    format!(
                        "{denied} of {} paths denied, {lines} reported",
                        targets.len()
                    ),
                ));
            }
            Ok(targets.len())
        }

        /// `cgroup.kill` reaches a process the service could not signal.
        fn probe_kill(&self) -> Result<(), ContainmentError> {
            let group = self
                .job(PROBE_MEMORY_MIB)
                .map_err(|e| refuse("CGROUP_UNAVAILABLE", format!("job cgroup: {:?}", e.kind())))?;
            let mut command = Command::new("/bin/sleep");
            command.arg("30").env_clear().current_dir("/").stdin(Stdio::null());
            group.confine(&mut command);
            let mut child = command
                .spawn()
                .map_err(|e| refuse("SPAWN_DENIED", format!("sleep: {:?}", e.kind())))?;
            group.kill();
            let status =
                child.wait().map_err(|e| refuse("PROBE_KILL", format!("{:?}", e.kind())))?;
            if status.signal() != Some(SIGKILL) {
                return Err(refuse("PROBE_KILL", "the process was not killed"));
            }
            Ok(())
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn reads_a_status_field() {
            let status = "Name:\tcat\nGroups:\t \nNoNewPrivs:\t1\nCapEff:\t0000000000000000\n";
            assert_eq!(field(status, "Groups"), Some(""));
            assert_eq!(field(status, "NoNewPrivs"), Some("1"));
            assert_eq!(field(status, "Missing"), None);
        }
    }
}
