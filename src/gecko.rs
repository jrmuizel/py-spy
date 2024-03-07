use std::collections::HashMap;
use std::io::Write;
use std::time::Instant;
use std::time::SystemTime;

use anyhow::Error;
use remoteprocess::Pid;
use serde_derive::Serialize;
use serde_json::to_writer;

use crate::stack_trace::StackTrace;

use fxprof_processed_profile::{Timestamp, ReferenceTimestamp, ThreadHandle, Profile, SamplingInterval, CategoryPairHandle, ProcessHandle, FrameInfo, FrameFlags, CpuDelta};


#[derive(Clone, Debug, Serialize)]
struct Args {
    pub filename: String,
    pub line: Option<u32>,
}

pub struct Geckotrace {
    start_ts: Instant,
    show_linenumbers: bool,
    category: CategoryPairHandle,
    last_ts: Instant,
    processes: HashMap<Pid, ProcessHandle>,
    threads: HashMap<u64, ThreadHandle>,
    profile: Profile,

}

impl Geckotrace {
    pub fn new(show_linenumbers: bool) -> Geckotrace {
        let now = Instant::now();
        let mut profile = Profile::new("python", ReferenceTimestamp::from_system_time(SystemTime::now()),  SamplingInterval::from_hz(100.));
        let category = profile.add_category("Python", fxprof_processed_profile::CategoryColor::Yellow).into();
        Geckotrace {
            start_ts: now,
            last_ts: now,
            show_linenumbers,
            processes: HashMap::new(),
            threads: HashMap::new(),
            profile,
            category,
        }
    }

    pub fn increment(&mut self, trace: &StackTrace) -> std::io::Result<()> {
        let now = Instant::now();
        let now_ts = Timestamp::from_nanos_since_reference((now - self.start_ts).as_nanos() as u64);

        let process = self.processes.entry(trace.pid).or_insert_with(|| {
            self.profile.add_process(&"python", trace.pid as u32, now_ts)
        });
        let thread = self.threads.entry(trace.thread_id).or_insert_with(|| {
            let thread = self.profile.add_thread(*process, trace.thread_id as u32, now_ts, true);
            if let Some(thread_name) = &trace.thread_name {
                self.profile.set_thread_name(thread, &thread_name);
            }
            thread
        });
        let frames = trace.frames.iter().map(|f| FrameInfo {
            frame: fxprof_processed_profile::Frame::Label(self.profile.intern_string(&if self.show_linenumbers { format!("{} {}:{}", f.name, f.filename, f.line) } else { f.name.clone() } )),
            category_pair: self.category,
            flags: FrameFlags::empty(),
        }).rev().collect::<Vec<_>>();
        self.profile.add_sample(*thread, now_ts, frames.into_iter(), CpuDelta::from_millis((now - self.last_ts).as_secs_f64() * 1000.), 1);
        self.last_ts = now;
        Ok(())
    }

    pub fn write(&self, w: &mut dyn Write) -> Result<(), Error> {
        to_writer(w, &self.profile)?;
        Ok(())
    }
}
