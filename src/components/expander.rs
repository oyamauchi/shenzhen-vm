use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::Arc;

use crate::xbus::{TSink, TSource, XBus};

struct Expander {
  p0: Option<Arc<AtomicI32>>,
  p1: Option<Arc<AtomicI32>>,
  p2: Option<Arc<AtomicI32>>,
}

/// Creates an expander, the component that converts between XBus I/O and three simple I/O pins.
///
/// When writing to the XBus, each simple pin is set to 100 if the corresponding digit of the XBus
/// value is nonzero, or 0 otherwise. (p2 = hundreds digit, p1 = tens, p0 = ones.) The sign of the
/// XBus value is ignored.
///
/// When reading from the XBus, each digit is 1 if the corresponding simple pin's value is >= 50,
/// or 0 otherwise.
///
/// This just returns a single XBus, even though the in-game component has three XBus pins. They
/// all do exactly the same thing, so the effect is the same as if there were just a single XBus
/// pin.
pub fn new(
  p0: Option<Arc<AtomicI32>>,
  p1: Option<Arc<AtomicI32>>,
  p2: Option<Arc<AtomicI32>>,
) -> XBus {
  let xbus = XBus::new();
  let expander = Arc::new(Expander { p0, p1, p2 });
  xbus.connect_sink(Arc::clone(&expander) as Arc<Expander>);
  xbus.connect_source(expander);

  xbus
}

impl TSource for Expander {
  fn can_read(&self) -> bool {
    true
  }

  fn read(&self) -> i32 {
    let to_bit = |atom: &Arc<AtomicI32>| (atom.load(Ordering::Relaxed) >= 50) as i32;
    let mut total = 0;

    total += 100 * self.p2.as_ref().map_or(0, to_bit);
    total += 10 * self.p1.as_ref().map_or(0, to_bit);
    total += self.p0.as_ref().map_or(0, to_bit);

    total
  }
}
impl TSink for Expander {
  fn write(&self, val: i32) {
    let abs_val = val.abs();
    if let Some(atom) = &self.p2 {
      atom.store(if abs_val >= 100 { 100 } else { 0 }, Ordering::Relaxed);
    }
    if let Some(atom) = &self.p1 {
      atom.store(if abs_val % 100 >= 10 { 100 } else { 0 }, Ordering::Relaxed);
    }
    if let Some(atom) = &self.p0 {
      atom.store(if abs_val % 10 >= 1 { 100 } else { 0 }, Ordering::Relaxed);
    }
  }
}
