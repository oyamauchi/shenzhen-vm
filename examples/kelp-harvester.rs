use std::collections::HashMap;
use std::fs::File;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::Arc;

use shenzhen_vm::components::{inputsource, memory};
use shenzhen_vm::controller::{Controller, Regs};
use shenzhen_vm::filerunner::{FileRunner, InputBus, OutputBus};
use shenzhen_vm::gen;
use shenzhen_vm::scheduler::{sleep, Scheduler};
use shenzhen_vm::xbus::XBus;

/// Read two consecutive inputs from the radio, pack them into a single int, and write them into
/// RAM at the write pointer. Send the updated write pointer to the Peeker.
struct InputConverter {
  radio_bus: XBus,
  ram_write_data: XBus,
  ram_write_addr: XBus,
  to_peeker: XBus,
}
impl Controller for InputConverter {
  fn name(&self) -> &'static str {
    "input-converter"
  }
  fn execute(&self, reg: &mut Regs) -> Result<(), ()> {
    reg.acc = self.radio_bus.read()?;
    if reg.acc != -999 {
      reg.acc *= 10;
      reg.acc += self.radio_bus.read()?;
      self.ram_write_data.write(reg.acc)?;
    }
    self.to_peeker.write(self.ram_write_addr.read()?)?;
    sleep(1)?;

    Ok(())
  }
}

/// Peeker peeks the head of the queue in RAM. It finds the first nonzero entry at or after the
/// original read pointer, sets the read pointer to point to it, and sends the value to Splitter.
struct Peeker {
  from_input_converter: XBus,
  ram_read_addr: XBus,
  ram_read_data: XBus,
  to_splitter: XBus,
}
impl Controller for Peeker {
  fn name(&self) -> &'static str {
    "peeker"
  }
  fn execute(&self, reg: &mut Regs) -> Result<(), ()> {
    self.from_input_converter.sleep()?;
    reg.acc = self.from_input_converter.read()?;

    // In-game, you accomplish this with clever use of conditional execution.
    let mut flag = true;
    while self.ram_read_addr.read()? != reg.acc {
      reg.dat = self.ram_read_data.read()?;
      if reg.dat != 0 {
        flag = false;
        reg.acc = self.ram_read_addr.read()?;
        reg.acc -= 1;
        self.ram_read_addr.write(reg.acc)?;
        break;
      }
    }

    if flag {
      // If the queue is empty, send zero to Splitter.
      reg.dat = 0;
    }

    self.to_splitter.write(reg.dat)?;
    Ok(())
  }
}

/// Splitter takes the destination from Peeker, splits it into x and y components, and sends those
/// to the motor controllers. It keeps track of the current position, and after updating it, sends
/// it to Searcher.
struct Splitter {
  from_peeker: XBus,
  to_motor_x: XBus,
  to_motor_y: XBus,
  to_searcher: XBus,
}
impl Controller for Splitter {
  fn name(&self) -> &'static str {
    "splitter"
  }
  fn execute(&self, reg: &mut Regs) -> Result<(), ()> {
    self.from_peeker.sleep()?;

    // dat is destination. acc is current position.
    reg.dat = self.from_peeker.read()?;

    if reg.dat == 0 {
      // Queue was empty. Pretend current position is the destination so that motors stop.
      reg.dat = reg.acc;
    } else {
      // Queue was nonempty. Get ready to separate destination into components. Overwriting the
      // position in acc is fine; we'll reconstruct it from the motor controllers' replies.
      reg.acc = reg.dat;
    }

    reg.dgt(1);
    self.to_motor_x.write(reg.acc)?;
    reg.dst(0, reg.dat);
    self.to_motor_y.write(reg.acc)?;

    reg.dst(1, self.to_motor_x.read()?);
    reg.dst(0, self.to_motor_y.read()?);

    if reg.dat != 0 {
      self.to_searcher.write(reg.acc)?;
    }

    Ok(())
  }
}

/// Searcher takes the current position from Splitter, searches the queue for it, and sets the
/// harvest output appropriately.
struct Searcher {
  io: XBus,
  ram_read_addr: XBus,
  ram_read_data: XBus,
  ram_write_addr: XBus,
  harvest: Arc<AtomicI32>,
}
impl Controller for Searcher {
  fn name(&self) -> &'static str {
    "searcher"
  }
  fn execute(&self, reg: &mut Regs) -> Result<(), ()> {
    self.io.sleep()?;
    reg.acc = self.io.read()?;
    reg.dat = self.ram_read_addr.read()?;

    loop {
      if reg.acc == self.ram_read_data.read()? {
        // Found the value. Go back and overwrite it with zero.
        reg.acc = self.ram_read_addr.read()?;
        reg.acc -= 1;
        self.ram_read_addr.write(reg.acc)?;
        self.ram_read_data.write(0)?;
        gen!(self.harvest, 1, 0);
        break;
      }

      // Stop once we hit the write pointer.
      if self.ram_read_addr.read()? == self.ram_write_addr.read()? {
        break;
      }
    }

    self.ram_read_addr.write(reg.dat)?;

    Ok(())
  }
}

/// Each motor controller takes in the x or y component of the current position from Splitter,
/// determines which direction to move and sets the output accordingly. It tracks the position
/// in acc and sends it back to Splitter.
struct MotorController {
  name: &'static str,
  io: XBus,
  output: Arc<AtomicI32>,
}
impl Controller for MotorController {
  fn name(&self) -> &'static str {
    self.name
  }
  fn execute(&self, reg: &mut Regs) -> Result<(), ()> {
    self.io.sleep()?;

    let input = self.io.read()?;
    let compare = input.cmp(&reg.acc);
    self.output.store(50, Ordering::Relaxed);

    // Do this with tcp
    match compare {
      std::cmp::Ordering::Equal => (),
      std::cmp::Ordering::Greater => {
        self.output.store(100, Ordering::Relaxed);
        reg.acc += 1;
      }
      std::cmp::Ordering::Less => {
        self.output.store(0, Ordering::Relaxed);
        reg.acc -= 1;
      }
    }

    self.io.write(reg.acc)?;

    Ok(())
  }
}

fn main() {
  // Input
  let (radio, radio_bus) = inputsource::nonblocking();

  // Output
  let harvest = Arc::new(AtomicI32::new(0));
  let motor_x = Arc::new(AtomicI32::new(0));
  let motor_y = Arc::new(AtomicI32::new(0));

  // Internal
  let ram = memory::ram();
  let input_to_peeker = XBus::new();
  let peeker_to_splitter = XBus::new();
  let searcher_io = XBus::new();
  let motor_x_io = XBus::new();
  let motor_y_io = XBus::new();

  let mut scheduler = Scheduler::new(vec![
    Box::new(InputConverter {
      radio_bus,
      ram_write_data: ram.data0,
      ram_write_addr: ram.addr0.clone(),
      to_peeker: input_to_peeker.clone(),
    }),
    Box::new(Peeker {
      from_input_converter: input_to_peeker,
      ram_read_addr: ram.addr1.clone(),
      ram_read_data: ram.data1.clone(),
      to_splitter: peeker_to_splitter.clone(),
    }),
    Box::new(Searcher {
      io: searcher_io.clone(),
      ram_read_addr: ram.addr1,
      ram_read_data: ram.data1,
      ram_write_addr: ram.addr0,
      harvest: harvest.clone(),
    }),
    Box::new(Splitter {
      from_peeker: peeker_to_splitter,
      to_motor_x: motor_x_io.clone(),
      to_motor_y: motor_y_io.clone(),
      to_searcher: searcher_io,
    }),
    Box::new(MotorController {
      name: "motor-x",
      io: motor_x_io,
      output: motor_x.clone(),
    }),
    Box::new(MotorController {
      name: "motor-y",
      io: motor_y_io,
      output: motor_y.clone(),
    }),
  ]);

  let mut file = File::open("examples/kelp-harvester.csv").unwrap();
  let mut runner = FileRunner::new(&mut file).unwrap();

  let count = runner
    .verify(
      &mut scheduler,
      HashMap::from([("radio", InputBus::XBus(&radio))]),
      HashMap::from([
        ("x", OutputBus::Simple(&motor_x)),
        ("y", OutputBus::Simple(&motor_y)),
        ("harvest", OutputBus::Simple(&harvest)),
      ]),
    )
    .unwrap();

  println!("Verified {} timesteps", count);

  scheduler.end();
}
