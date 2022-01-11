//! RAM and ROM components from the game (14 cells, two independent pointers).

use std::fmt::{Debug, Write};
use std::sync::{Arc, Mutex};

use crate::xbus::{TSink, TSource, XBus};

struct AddrPin {
  mem: Arc<Mutex<MemInner>>,
  index: usize,
}

struct DataPin {
  mem: Arc<Mutex<MemInner>>,
  index: usize,
}

struct MemInner {
  contents: [i32; 14],
  pointers: [usize; 2],
}

fn adjust_index(index: i32) -> usize {
  let modded = index % 14;
  (if modded < 0 { modded + 14 } else { modded }) as usize
}

impl TSource for DataPin {
  fn can_read(&self) -> bool {
    true
  }

  fn read(&self) -> i32 {
    let mut mem = self.mem.lock().unwrap();
    let current_index = mem.pointers[self.index];

    let result = mem.contents[current_index];
    let new_index = adjust_index(current_index as i32 + 1);
    mem.pointers[self.index] = new_index;
    result
  }
}

impl TSink for DataPin {
  fn write(&self, val: i32) {
    let mut mem = self.mem.lock().unwrap();
    let current_index = mem.pointers[self.index];

    mem.contents[current_index] = val;
    mem.pointers[self.index] = adjust_index(current_index as i32 + 1);
  }
}

impl TSource for AddrPin {
  fn can_read(&self) -> bool {
    true
  }

  fn read(&self) -> i32 {
    self.mem.lock().unwrap().pointers[self.index] as i32
  }
}

impl TSink for AddrPin {
  fn write(&self, val: i32) {
    self.mem.lock().unwrap().pointers[self.index] = adjust_index(val);
  }
}

/// Represents a RAM or ROM module.
///
/// Internally, there's an array of 14 ints for the contents, and two indexes into that array.
/// `addr0` and `addr1` read and write those two indexes. `data0` and `data1` read the contents at
/// those two indexes respectively, and in RAMs only, write to the contents array at those two
/// indexes. Any read from, or write to, a data bus increments the corresponding index by 1
/// (wrapping around to zero when incremented past 13).
pub struct Memory {
  pub addr0: XBus,
  pub addr1: XBus,
  pub data0: XBus,
  pub data1: XBus,
  mem: Arc<Mutex<MemInner>>,
}

impl Debug for Memory {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let mem = self.mem.lock().unwrap();
    let make_cell = |index, fmt: &mut std::fmt::Formatter<'_>| {
      let left_arrow = if index == mem.pointers[0] { ">" } else { " " };
      let right_arrow = if index == mem.pointers[1] { "<" } else { " " };
      fmt.write_fmt(format_args!(
        "[ {} {:2} {} ]",
        left_arrow, mem.contents[index], right_arrow
      ))
    };

    for i in 0..7 {
      make_cell(i, f)?;
      make_cell(i + 7, f)?;
      f.write_char('\n')?;
    }

    Ok(())
  }
}

/// Create a ROM. The data pins don't have sinks connected, only sources, so writes to them will
/// block forever unless there's something else reading from the same bus.
pub fn rom(contents: [i32; 14]) -> Memory {
  let (addr0, addr1, data0, data1) = (XBus::new(), XBus::new(), XBus::new(), XBus::new());
  let mem = Arc::new(Mutex::new(MemInner {
    contents,
    pointers: [0, 0],
  }));

  let a0 = Arc::new(AddrPin {
    mem: Arc::clone(&mem),
    index: 0,
  });
  let a1 = Arc::new(AddrPin {
    mem: Arc::clone(&mem),
    index: 1,
  });
  let d0 = Arc::new(DataPin {
    mem: Arc::clone(&mem),
    index: 0,
  });
  let d1 = Arc::new(DataPin {
    mem: Arc::clone(&mem),
    index: 1,
  });

  addr0.connect_source(Arc::clone(&a0) as Arc<AddrPin>);
  addr0.connect_sink(a0);
  addr1.connect_source(Arc::clone(&a1) as Arc<AddrPin>);
  addr1.connect_sink(a1);

  data0.connect_source(d0);
  data1.connect_source(d1);

  Memory {
    addr0,
    addr1,
    data0,
    data1,
    mem,
  }
}

/// Create a RAM, initialized to all zeros.
pub fn ram() -> Memory {
  let (addr0, addr1, data0, data1) = (XBus::new(), XBus::new(), XBus::new(), XBus::new());
  let mem = Arc::new(Mutex::new(MemInner {
    contents: [0; 14],
    pointers: [0, 0],
  }));

  let a0 = Arc::new(AddrPin {
    mem: Arc::clone(&mem),
    index: 0,
  });
  let a1 = Arc::new(AddrPin {
    mem: Arc::clone(&mem),
    index: 1,
  });
  let d0 = Arc::new(DataPin {
    mem: Arc::clone(&mem),
    index: 0,
  });
  let d1 = Arc::new(DataPin {
    mem: Arc::clone(&mem),
    index: 1,
  });

  addr0.connect_source(Arc::clone(&a0) as Arc<AddrPin>);
  addr0.connect_sink(a0);
  addr1.connect_source(Arc::clone(&a1) as Arc<AddrPin>);
  addr1.connect_sink(a1);

  data0.connect_source(Arc::clone(&d0) as Arc<DataPin>);
  data0.connect_sink(d0);
  data1.connect_source(Arc::clone(&d1) as Arc<DataPin>);
  data1.connect_sink(d1);

  Memory {
    addr0,
    addr1,
    data0,
    data1,
    mem,
  }
}
