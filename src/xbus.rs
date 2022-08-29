//! Logic to model reading from and writing to an XBus.

use std::collections::HashMap;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Arc, Mutex};

use crate::controller::current_name;
use crate::scheduler::{Scheduler, SleepToken};

pub(crate) trait TSource {
  fn can_read(&self) -> bool;
  fn read(&self) -> i32;
}

pub(crate) trait TSink {
  fn write(&self, _: i32);
}

/// Represents XBus connections between components, and the logic of reading, writing, and sleeping
/// on them.
///
/// By nature, XBuses have to be shared between components. To do this, call `clone` on them.
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
  /// Create a new XBus.
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

  /// For controller code: sleep until there is a value readable from this XBus.
  ///
  /// If there is already a value readable, because there's a source connected or another component
  /// has written one, this returns immediately.
  ///
  /// NB: even after returning from this, immediately reading from the same XBus may block!
  /// This behavior is the same as in the game: every controller `slx`-ing on a bus will wake up
  /// when something writes a value onto the bus, even though only one will get to read that value.
  #[allow(clippy::result_unit_err)]
  pub fn sleep(&self) -> Result<(), ()> {
    if !self.can_read() {
      Scheduler::sleep(SleepToken::XBusSleep(self.clone()))?;
    }
    Ok(())
  }

  /// For controller code: read from the bus, blocking until a value is available.
  #[allow(clippy::result_unit_err)]
  pub fn read(&self) -> Result<i32, ()> {
    // The eventual writer will put its value in here.
    let cell: Arc<AtomicI32>;

    {
      let mut xbus = self.inner.lock().unwrap();

      // If there's a pending write from another component, just take it.
      if !xbus.pending_writers.is_empty() {
        let key = *xbus.pending_writers.iter().next().unwrap().0;
        let value = xbus.pending_writers.remove(key).unwrap();
        return Ok(value);
      }

      // TODO: pick a source randomly
      for source in xbus.sources.iter() {
        if source.can_read() {
          return Ok(source.read());
        }
      }

      // Put ourselves into the pending readers queue.
      let name = current_name();
      cell = Arc::new(AtomicI32::new(0));
      xbus.pending_readers.insert(name, cell.clone());
    } // Unlock the mutex before sleeping.

    Scheduler::sleep(SleepToken::XBusRead(self.clone()))?;
    Ok(cell.load(Ordering::Relaxed))
  }

  /// For controller code: write to the bus, blocking until something else consumes it.
  #[allow(clippy::result_unit_err)]
  pub fn write(&self, val: i32) -> Result<(), ()> {
    {
      let mut xbus = self.inner.lock().unwrap();

      // If there's a reader already waiting, give it our value.
      if !xbus.pending_readers.is_empty() {
        let key = *xbus.pending_readers.iter().next().unwrap().0;
        let cell = xbus.pending_readers.remove(key).unwrap();
        cell.store(val, Ordering::Relaxed);
        return Ok(());
      }

      // TODO: pick a sink randomly
      if !xbus.sinks.is_empty() {
        xbus.sinks[0].write(val);
        return Ok(());
      }

      // Put our value into the pending writers queue.
      let name = current_name();
      xbus.pending_writers.insert(name, val);
    } // Unlock the mutex before sleeping.

    Scheduler::sleep(SleepToken::XBusWrite(self.clone()))?;
    Ok(())
  }

  // Everything below here is crate-internal only.

  pub(crate) fn connect_source(&self, source: Arc<dyn TSource + Send + Sync>) {
    self.inner.lock().unwrap().sources.push(source);
  }

  pub(crate) fn connect_sink(&self, sink: Arc<dyn TSink + Send + Sync>) {
    self.inner.lock().unwrap().sinks.push(sink);
  }

  pub(crate) fn can_read(&self) -> bool {
    let inner = self.inner.lock().unwrap();
    !inner.pending_writers.is_empty() || inner.sources.iter().any(|src| src.can_read())
  }

  pub(crate) fn is_read_pending(&self, controller_name: &'static str) -> bool {
    self
      .inner
      .lock()
      .unwrap()
      .pending_readers
      .contains_key(controller_name)
  }

  pub(crate) fn is_write_pending(&self, controller_name: &'static str) -> bool {
    self
      .inner
      .lock()
      .unwrap()
      .pending_writers
      .contains_key(controller_name)
  }
}
