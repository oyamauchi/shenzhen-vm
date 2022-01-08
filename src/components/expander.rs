use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::Arc;

use crate::xbus::{TSink, TSource, XBus};

struct Expander {
  p0: Option<Arc<AtomicI32>>,
  p1: Option<Arc<AtomicI32>>,
  p2: Option<Arc<AtomicI32>>,
}

pub struct ExpanderPins {
  pub x0: XBus,
  pub x1: XBus,
  pub x2: XBus,
}

pub fn new(
  p0: Option<Arc<AtomicI32>>,
  p1: Option<Arc<AtomicI32>>,
  p2: Option<Arc<AtomicI32>>,
) -> ExpanderPins {
  let (x0, x1, x2) = (XBus::new(), XBus::new(), XBus::new());

  let expander = Arc::new(Expander { p0, p1, p2 });
  x0.connect_sink(Arc::clone(&expander) as Arc<Expander>);
  x1.connect_sink(Arc::clone(&expander) as Arc<Expander>);
  x2.connect_sink(Arc::clone(&expander) as Arc<Expander>);
  x0.connect_source(Arc::clone(&expander) as Arc<Expander>);
  x1.connect_source(Arc::clone(&expander) as Arc<Expander>);
  x2.connect_source(Arc::clone(&expander) as Arc<Expander>);

  ExpanderPins { x0, x1, x2 }
}

impl TSource for Expander {
  fn can_read(&self) -> bool {
    true
  }

  fn read(&self) -> i32 {
    let is_high = |atom: &Arc<AtomicI32>| atom.load(Ordering::Relaxed) >= 50;
    let mut total = 0;

    total += if self.p2.as_ref().map_or(false, is_high) {
      100
    } else {
      0
    };
    total += if self.p1.as_ref().map_or(false, is_high) {
      10
    } else {
      0
    };
    total += if self.p0.as_ref().map_or(false, is_high) {
      1
    } else {
      0
    };

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
