use std::collections::HashMap;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Arc, Mutex};

use crate::controller::current_name;
use crate::scheduler::{Scheduler, SleepToken};

pub trait TSource {
  fn can_read(&self) -> bool;
  fn read(&self) -> i32;
}

pub trait TSink {
  fn write(&self, _: i32);
}

/// Represents XBus connections between components, and the logic of reading, writing, and sleeping
/// on them.
#[derive(Clone)]
pub struct XBus {
  inner: Arc<Mutex<Inner>>,
}

struct Inner {
  sources: Vec<Arc<dyn TSource + Send + Sync>>,
  sinks: Vec<Arc<dyn TSink + Send + Sync>>,

  pending_readers: HashMap<&'static str, Arc<AtomicI32>>,
  pending_writers: HashMap<&'static str, i32>,
}

impl XBus {
  pub fn new() -> XBus {
    let inner = Mutex::new(Inner {
      sources: vec![],
      sinks: vec![],
      pending_readers: HashMap::new(),
      pending_writers: HashMap::new(),
    });
    XBus {
      inner: Arc::new(inner),
    }
  }

  pub fn connect_source(&self, source: Arc<dyn TSource + Send + Sync>) {
    self.inner.lock().unwrap().sources.push(source);
  }

  pub fn connect_sink(&self, sink: Arc<dyn TSink + Send + Sync>) {
    self.inner.lock().unwrap().sinks.push(sink);
  }

  pub fn can_read(&self) -> bool {
    let inner = self.inner.lock().unwrap();
    !inner.pending_writers.is_empty() || inner.sources.iter().any(|src| src.can_read())
  }

  pub fn sleep(&self) -> Result<(), ()> {
    if !self.can_read() {
      Scheduler::sleep(SleepToken::XBusSleep(self.clone()))?;
    }
    Ok(())
  }

  /// Read from the bus, blocking until a value is available. If a value doesn't become available
  /// until all other Controllers are either blocked or sleeping, panic.
  pub fn read(&self) -> Result<i32, ()> {
    let cell = Arc::new(AtomicI32::new(0));

    {
      let mut xbus = self.inner.lock().unwrap();
      if !xbus.pending_writers.is_empty() {
        let key = xbus.pending_writers.iter().next().unwrap().0.clone();
        let value = xbus.pending_writers.remove(&key).unwrap();
        return Ok(value);
      }

      // TODO: pick a source randomly
      for source in xbus.sources.iter() {
        if source.can_read() {
          return Ok(source.read());
        }
      }

      let name = current_name();
      xbus.pending_readers.insert(name, cell.clone());
    }

    Scheduler::sleep(SleepToken::XBusRead(self.clone()))?;
    Ok(cell.load(Ordering::Relaxed))
  }

  /// Write to the bus, blocking until something else consumes it. If nothing else consumes it
  /// before all other Controllers are blocked or sleeping, panic.
  pub fn write(&self, val: i32) -> Result<(), ()> {
    {
      let mut xbus = self.inner.lock().unwrap();

      if !xbus.pending_readers.is_empty() {
        let key = xbus.pending_readers.iter().next().unwrap().0.clone();
        let cell = xbus.pending_readers.remove(&key).unwrap();
        cell.store(val, Ordering::Relaxed);
        return Ok(());
      }

      // TODO: pick a sink randomly
      if !xbus.sinks.is_empty() {
        xbus.sinks[0].write(val);
        return Ok(());
      }

      let name = current_name();
      xbus.pending_writers.insert(name, val);
    }

    Scheduler::sleep(SleepToken::XBusWrite(self.clone()))?;
    Ok(())
  }

  pub fn is_read_pending(&self, controller_name: &'static str) -> bool {
    self
      .inner
      .lock()
      .unwrap()
      .pending_readers
      .contains_key(controller_name)
  }

  pub fn is_write_pending(&self, controller_name: &'static str) -> bool {
    self
      .inner
      .lock()
      .unwrap()
      .pending_writers
      .contains_key(controller_name)
  }
}
