use std::sync::{Arc, Mutex};

use crate::xbus::{TSink, TSource, XBus};

struct AddrPin {
  mem: Arc<Mutex<Memory>>,
  index: usize,
}

struct DataPin {
  mem: Arc<Mutex<Memory>>,
  index: usize,
}

struct Memory {
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

pub struct MemoryPins {
  pub addr0: XBus,
  pub addr1: XBus,
  pub data0: XBus,
  pub data1: XBus,
}

pub fn rom(contents: [i32; 14]) -> MemoryPins {
  let (addr0, addr1, data0, data1) = (XBus::new(), XBus::new(), XBus::new(), XBus::new());
  let mem = Arc::new(Mutex::new(Memory {
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

  MemoryPins {
    addr0,
    addr1,
    data0,
    data1,
  }
}

pub fn ram() -> MemoryPins {
  let (addr0, addr1, data0, data1) = (XBus::new(), XBus::new(), XBus::new(), XBus::new());
  let mem = Arc::new(Mutex::new(Memory {
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

  MemoryPins {
    addr0,
    addr1,
    data0,
    data1,
  }
}
