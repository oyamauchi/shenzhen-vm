//! For printing program output, and storing it for verification.

use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::Mutex;

use crate::xbus::{TSink, XBus};

pub struct OutputSink {
  name: &'static str,
  printing: bool,
  queue: Mutex<VecDeque<i32>>,
}

/// Create a new sink, returning it and an XBus that it's connected to. If `printing` is true,
/// each value written will be printed with `println!`.
pub fn new(name: &'static str, printing: bool) -> (Arc<OutputSink>, XBus) {
  let xbus = XBus::new();
  let sink = Arc::new(OutputSink {
    name,
    printing,
    queue: Mutex::new(VecDeque::new()),
  });

  xbus.connect_sink(Arc::clone(&sink) as Arc<OutputSink>);
  (sink, xbus)
}

impl OutputSink {
  /// Move the contents of the internal queue into the given Vec.
  pub fn queue_into(&self, dest: &mut Vec<i32>) {
    let mut queue = self.queue.lock().unwrap();

    while !queue.is_empty() {
      dest.push(queue.pop_front().expect(""));
    }
  }
}

impl TSink for OutputSink {
  fn write(&self, val: i32) {
    if self.printing {
      println!("{}: {}", self.name, val)
    }

    self.queue.lock().unwrap().push_back(val);
  }
}
