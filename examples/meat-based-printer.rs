extern crate shenzhen_vm;

use std::collections::HashMap;
use std::fs::File;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::Arc;

use shenzhen_vm::components::{expander, inputsource, memory};
use shenzhen_vm::controller::{Controller, Regs};
use shenzhen_vm::filerunner::{FileRunner, InputBus, OutputBus};
use shenzhen_vm::gen;
use shenzhen_vm::scheduler::{sleep, Scheduler};
use shenzhen_vm::xbus::XBus;

fn main() {
  let p0 = Arc::new(AtomicI32::new(0));
  let p1 = Arc::new(AtomicI32::new(0));
  let p2 = Arc::new(AtomicI32::new(0));
  let extrude = Arc::new(AtomicI32::new(0));

  let (keypad, keypad_bus) = inputsource::blocking();

  let rom = memory::rom([111, 0, 101, 0, 101, 0, 111, 0, 10, 10, 0, 10, 10, 0]);

  let transfer = XBus::new();

  let expander_bus = expander::new(Some(p0.clone()), Some(p1.clone()), Some(p2.clone()));

  struct Main {
    keypad_bus: XBus,
    rom_addr: XBus,
    to_outputter: XBus,
    to_expander: XBus,
    extrude: Arc<AtomicI32>,
  }
  impl Controller for Main {
    fn name(&self) -> &'static str {
      "main"
    }
    fn execute(&self, _: &mut Regs) -> Result<(), ()> {
      self.keypad_bus.sleep()?;
      let value = self.keypad_bus.read()?;
      match value {
        1 => {
          self.rom_addr.write(0)?;
          self.to_outputter.write(7)?;
        }
        2 => {
          self.rom_addr.write(7)?;
          self.to_outputter.write(7)?;
        }
        3 => {
          self.to_expander.write(11)?;
        }
        _ => panic!("{} is not a valid keypad input", value),
      }

      gen!(self.extrude, 7, 0);
      self.to_expander.write(0)?;

      Ok(())
    }
  }

  struct Outputter {
    from_main: XBus,
    rom_data: XBus,
    to_expander: XBus,
  }
  impl Controller for Outputter {
    fn name(&self) -> &'static str {
      "output"
    }
    fn execute(&self, reg: &mut Regs) -> Result<(), ()> {
      self.from_main.sleep()?;
      reg.acc = self.from_main.read()?;
      while reg.acc > 0 {
        self.to_expander.write(self.rom_data.read()?)?;
        sleep(1)?;
        reg.acc -= 1;
      }

      Ok(())
    }
  }

  let mut scheduler = Scheduler::new(vec![
    Box::new(Main {
      keypad_bus,
      rom_addr: rom.addr0,
      to_outputter: transfer.clone(),
      to_expander: expander_bus.clone(),
      extrude: extrude.clone(),
    }),
    Box::new(Outputter {
      from_main: transfer,
      rom_data: rom.data0,
      to_expander: expander_bus,
    }),
  ]);

  let mut f = File::open("examples/meat-based-printer.csv").unwrap();
  let mut runner = FileRunner::new(&mut f).unwrap();

  let num_steps_verified = runner
    .verify(
      &mut scheduler,
      HashMap::from([("keypad", InputBus::XBus(&keypad))]),
      HashMap::from([
        ("p0", OutputBus::Simple(&p0)),
        ("p1", OutputBus::Simple(&p1)),
        ("p2", OutputBus::Simple(&p2)),
        ("extrude", OutputBus::Simple(&extrude)),
      ]),
    )
    .unwrap();

  println!("Verified {} timesteps", num_steps_verified);

  scheduler.end();
}
