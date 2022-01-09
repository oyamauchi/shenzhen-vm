use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use crate::xbus::{TSource, XBus};

enum InputSourceType {
  Blocking,
  NonBlocking,
}

/// Puts program input onto an XBus. Internally maintains a queue of values, and can be created as
/// either blocking or nonblocking.
pub struct InputSource {
  source_type: InputSourceType,
  queue: Mutex<VecDeque<i32>>,
}

fn make(source_type: InputSourceType) -> (Arc<InputSource>, XBus) {
  let source = Arc::new(InputSource {
    source_type,
    queue: Mutex::new(VecDeque::new()),
  });
  let bus = XBus::new();
  bus.connect_source(Arc::clone(&source) as Arc<InputSource>);

  (source, bus)
}

/// Creates a blocking source. Reading while the queue is empty will block. Returns an `Arc` of the
/// source itself (to call `inject` on), and an XBus with the source connected.
pub fn blocking() -> (Arc<InputSource>, XBus) {
  make(InputSourceType::Blocking)
}

/// Creates a nonblocking source. Reading while the queue is empty will produce the value -999.
/// Returns an `Arc` of the source itself (to call `inject` on), and an XBus with the source
/// connected.
pub fn nonblocking() -> (Arc<InputSource>, XBus) {
  make(InputSourceType::NonBlocking)
}

impl InputSource {
  /// Add a value to the queue. Unlike controllers' XBus writes, it's not an error for these values
  /// to stay in the queue across timesteps.
  pub fn inject(&self, value: i32) {
    self.queue.lock().unwrap().push_back(value);
  }
}

impl TSource for InputSource {
  fn can_read(&self) -> bool {
    match &self.source_type {
      InputSourceType::Blocking => !self.queue.lock().unwrap().is_empty(),
      InputSourceType::NonBlocking => true,
    }
  }

  fn read(&self) -> i32 {
    let mut queue = self.queue.lock().unwrap();
    match &self.source_type {
      InputSourceType::Blocking => queue.pop_front().expect("Cannot read from empty queue"),
      InputSourceType::NonBlocking => queue.pop_front().unwrap_or(-999),
    }
  }
}
