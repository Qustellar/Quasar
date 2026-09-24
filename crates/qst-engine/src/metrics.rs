use super::FrameDiagnostics;

pub(crate) struct BenchmarkCapture {
    pub(crate) warmup_remaining: u32,
    pub(crate) measured_remaining: u32,
    pub(crate) resolution_stable: bool,
    pub(crate) frame_ms: Vec<f32>,
    pub(crate) fixed_ms: Vec<f32>,
    pub(crate) render_ms: Vec<f32>,
    pub(crate) working_sets: Vec<u64>,
    pub(crate) private_working_sets: Vec<u64>,
    pub(crate) private_commits: Vec<u64>,
}

impl BenchmarkCapture {
    pub(crate) fn new(warmup_frames: u32, measured_frames: u32) -> Self {
        Self {
            warmup_remaining: warmup_frames,
            measured_remaining: measured_frames.max(1),
            resolution_stable: true,
            frame_ms: Vec::new(),
            fixed_ms: Vec::new(),
            render_ms: Vec::new(),
            working_sets: Vec::new(),
            private_working_sets: Vec::new(),
            private_commits: Vec::new(),
        }
    }

    pub(crate) fn record(
        &mut self,
        diagnostics: &FrameDiagnostics,
        actual_size: (u32, u32),
        requested_size: (u32, u32),
    ) -> bool {
        self.resolution_stable &= actual_size == requested_size;
        if self.warmup_remaining > 0 {
            self.warmup_remaining -= 1;
            return false;
        }
        if self.measured_remaining == 0 {
            return true;
        }
        self.frame_ms.push(diagnostics.frame_seconds * 1000.0);
        self.fixed_ms
            .push(diagnostics.fixed_update_seconds * 1000.0);
        self.render_ms.push(diagnostics.render_seconds * 1000.0);
        if let Some(bytes) = diagnostics.resident_working_set_bytes {
            self.working_sets.push(bytes);
        }
        if let Some(bytes) = diagnostics.private_working_set_bytes {
            self.private_working_sets.push(bytes);
        }
        if let Some(bytes) = diagnostics.private_commit_bytes {
            self.private_commits.push(bytes);
        }
        self.measured_remaining -= 1;
        self.measured_remaining == 0
    }

    pub(crate) fn print_summary(&self, width: u32, height: u32) {
        let working_set_min = self.working_sets.iter().min().copied();
        let working_set_peak = self.working_sets.iter().max().copied();
        let private_working_set_min = self.private_working_sets.iter().min().copied();
        let private_working_set_peak = self.private_working_sets.iter().max().copied();
        let private_commit_min = self.private_commits.iter().min().copied();
        let private_commit_peak = self.private_commits.iter().max().copied();
        println!(
            "benchmark width={width} height={height} resolution_stable={} frames={} frame_p50_ms={:.3} frame_p95_ms={:.3} fixed_p95_ms={:.3} render_p95_ms={:.3}",
            self.resolution_stable,
            self.frame_ms.len(),
            percentile(&self.frame_ms, 0.50),
            percentile(&self.frame_ms, 0.95),
            percentile(&self.fixed_ms, 0.95),
            percentile(&self.render_ms, 0.95),
        );
        println!(
            "benchmark_memory working_set_min_bytes={working_set_min:?} working_set_peak_bytes={working_set_peak:?} private_working_set_min_bytes={private_working_set_min:?} private_working_set_peak_bytes={private_working_set_peak:?} private_commit_min_bytes={private_commit_min:?} private_commit_peak_bytes={private_commit_peak:?}"
        );
    }
}

pub(crate) fn percentile(samples: &[f32], fraction: f32) -> f32 {
    let mut sorted = samples.to_vec();
    sorted.sort_by(f32::total_cmp);
    let index = ((sorted.len() as f32 * fraction).ceil() as usize)
        .saturating_sub(1)
        .min(sorted.len() - 1);
    sorted[index]
}

#[cfg(windows)]
pub(crate) fn current_process_memory() -> Option<ProcessMemory> {
    use windows_sys::Win32::System::ProcessStatus::{
        GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS, PROCESS_MEMORY_COUNTERS_EX2,
    };
    use windows_sys::Win32::System::Threading::GetCurrentProcess;
    let mut counters = PROCESS_MEMORY_COUNTERS_EX2::default();
    counters.cb = std::mem::size_of_val(&counters) as u32;
    // SAFETY: GetCurrentProcess is valid and EX2 begins with PROCESS_MEMORY_COUNTERS.
    let success = unsafe {
        GetProcessMemoryInfo(
            GetCurrentProcess(),
            (&mut counters as *mut PROCESS_MEMORY_COUNTERS_EX2).cast(),
            counters.cb,
        )
    };
    if success != 0 {
        return Some(ProcessMemory {
            working_set_bytes: counters.WorkingSetSize as u64,
            private_working_set_bytes: Some(counters.PrivateWorkingSetSize as u64),
            private_commit_bytes: Some(counters.PrivateUsage as u64),
        });
    }
    let mut basic = PROCESS_MEMORY_COUNTERS::default();
    basic.cb = std::mem::size_of_val(&basic) as u32;
    // SAFETY: The fallback structure and process handle satisfy the API contract.
    let success = unsafe { GetProcessMemoryInfo(GetCurrentProcess(), &mut basic, basic.cb) };
    (success != 0).then_some(ProcessMemory {
        working_set_bytes: basic.WorkingSetSize as u64,
        private_working_set_bytes: None,
        private_commit_bytes: None,
    })
}

#[cfg(not(windows))]
pub(crate) fn current_process_memory() -> Option<ProcessMemory> {
    None
}

pub(crate) struct ProcessMemory {
    pub(crate) working_set_bytes: u64,
    pub(crate) private_working_set_bytes: Option<u64>,
    pub(crate) private_commit_bytes: Option<u64>,
}
