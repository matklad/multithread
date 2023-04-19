use std::{
    num::NonZeroUsize,
    sync::{mpsc, Condvar, Mutex},
    thread::JoinHandle,
};

pub struct MultiThread {
    current_thread: bool,
    senders: Vec<mpsc::Sender<Job<'static>>>,
    handles: Vec<JoinHandle<()>>,
}

impl MultiThread {
    pub fn new(n_threads: NonZeroUsize) -> MultiThread {
        let n_threads = n_threads.get();
        let mut result = MultiThread {
            current_thread: false,
            senders: Vec::with_capacity(n_threads),
            handles: Vec::with_capacity(n_threads),
        };
        for _ in 0..n_threads {
            let (sender, receiver) = mpsc::channel::<Job>();
            let handle = std::thread::spawn(move || {
                for job in receiver {
                    (job.f)()
                }
            });
            result.senders.push(sender);
            result.handles.push(handle)
        }
        result
    }

    pub fn new_current_thread() -> MultiThread {
        MultiThread { current_thread: true, senders: Vec::new(), handles: Vec::new() }
    }

    pub fn run<F>(&self, job: F)
    where
        F: Fn() + Sync,
    {
        if self.current_thread {
            job();
            return;
        }
        self.run_par(&job);
    }

    fn run_par(&self, f: &(dyn Fn() + Sync)) {
        let job_count = JobCount::new();
        for s in &self.senders {
            let job = Job { f, _g: job_count.inc() };
            s.send(unsafe { job.erase_lifetime() }).unwrap();
        }
    }
}

impl Drop for MultiThread {
    fn drop(&mut self) {
        self.senders.clear();
        for h in self.handles.drain(..) {
            let _ = h.join();
        }
    }
}

struct Job<'a> {
    f: &'a (dyn Fn() + Sync),
    _g: JobGuard<'a>,
}

struct JobCount {
    mux: Mutex<usize>,
    cv: Condvar,
}

struct JobGuard<'a> {
    count: &'a JobCount,
}

impl<'a> Job<'a> {
    unsafe fn erase_lifetime(self) -> Job<'static> {
        std::mem::transmute(self)
    }
}

impl JobCount {
    fn new() -> JobCount {
        JobCount { mux: Mutex::new(0), cv: Condvar::new() }
    }
    fn inc(&self) -> JobGuard<'_> {
        *self.mux.lock().unwrap() += 1;
        JobGuard { count: self }
    }
    fn dec(&self) {
        let mut g = self.mux.lock().unwrap();
        *g -= 1;
        if *g == 0 {
            self.cv.notify_all()
        }
    }
}

impl Drop for JobCount {
    fn drop(&mut self) {
        let mut g = self.mux.lock().unwrap();
        while *g > 0 {
            g = self.cv.wait(g).unwrap();
        }
    }
}

impl<'a> Drop for JobGuard<'a> {
    fn drop(&mut self) {
        self.count.dec()
    }
}
