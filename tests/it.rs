use std::{
    num::NonZeroUsize,
    sync::{Condvar, Mutex},
};

use multithread::MultiThread;

#[test]
fn quick_sort() {
    let mut xs = *b"Hello, world!";
    par_quick_sort(&mut xs);
    assert_eq!(xs, *b" !,Hdellloorw");
}

fn par_quick_sort<T: std::fmt::Debug>(xs: &mut [T])
where
    T: Ord + Send + Sync,
{
    let multithread = MultiThread::new(NonZeroUsize::new(8).unwrap());
    let queue = Queue::new();
    queue.send(xs);
    multithread.run(|| {
        while let Some(slice) = queue.recv() {
            if slice.len() > 1 {
                let (left, right) = partition(slice);
                queue.send(left);
                queue.send(right);
            }
            queue.done();
        }
    })
}

fn partition<T: Ord>(xs: &mut [T]) -> (&mut [T], &mut [T]) {
    let mut l_count = 0;
    let mut r_count = 0;
    for i in 1..xs.len() {
        if xs[i] <= xs[0] {
            xs.swap(i, l_count + 1);
            l_count += 1;
        } else {
            r_count += 1;
        }
    }
    assert!(l_count + r_count + 1 == xs.len());

    xs.swap(0, l_count);
    let (l, rest) = xs.split_at_mut(l_count);
    let (_pivot, r) = rest.split_at_mut(1);

    assert!(l_count == l.len());
    assert!(r_count == r.len());
    (l, r)
}

struct Queue<T> {
    inner: (Mutex<QueueInner<T>>, Condvar),
}

struct QueueInner<T> {
    items: Vec<T>,
    sent: usize,
    done: usize,
}

impl<T: Send> Queue<T> {
    pub fn new() -> Queue<T> {
        Queue {
            inner: (Mutex::new(QueueInner { items: Vec::new(), sent: 0, done: 0 }), Condvar::new()),
        }
    }

    pub fn send(&self, value: T) {
        let mut g = self.inner.0.lock().unwrap();
        assert!(!g.sent > g.done);
        g.sent += 1;
        g.items.push(value);
        self.inner.1.notify_one();
    }

    pub fn done(&self) {
        let mut g = self.inner.0.lock().unwrap();
        assert!(!g.sent > g.done);
        g.done += 1;
        if g.done == g.sent {
            self.inner.1.notify_all();
        }
    }

    pub fn recv(&self) -> Option<T> {
        let mut g = self.inner.0.lock().unwrap();
        loop {
            if let Some(item) = g.items.pop() {
                return Some(item);
            }
            if g.sent == g.done {
                return None;
            }
            g = self.inner.1.wait(g).unwrap();
        }
    }
}
