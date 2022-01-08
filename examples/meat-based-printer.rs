extern crate shenzhen_vm;

use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::Arc;

use shenzhen_vm::components::{expander, inputsource, memory};
use shenzhen_vm::controller::Controller;
use shenzhen_vm::scheduler::{sleep, Scheduler};
use shenzhen_vm::xbus::XBus;

fn gen(pin: &AtomicI32, on_steps: i32, off_steps: i32) -> Result<(), ()> {
  if on_steps > 0 {
    pin.store(100, Ordering::Relaxed);
    sleep(on_steps)?;
  }
  pin.store(0, Ordering::Relaxed);
  if off_steps > 0 {
    sleep(off_steps)?;
  }
  Ok(())
}

fn main() {
  let p0 = Arc::new(AtomicI32::new(0));
  let p1 = Arc::new(AtomicI32::new(0));
  let p2 = Arc::new(AtomicI32::new(0));
  let extrude = Arc::new(AtomicI32::new(0));
  let extrude_clone = extrude.clone();

  let (keypad, keypad_bus) = inputsource::blocking();

  let rom = memory::rom([111, 0, 101, 0, 101, 0, 111, 0, 10, 10, 0, 10, 10, 0]);

  let transfer_in = XBus::new();
  let transfer_out = transfer_in.clone();

  let expander = expander::new(Some(p0.clone()), Some(p1.clone()), Some(p2.clone()));

  let main_ctrl = Controller::new("main", move |_| {
    keypad_bus.sleep()?;
    let value = keypad_bus.read()?;
    match value {
      1 => {
        rom.addr0.write(0)?;
        transfer_out.write(7)?;
      }
      2 => {
        rom.addr0.write(7)?;
        transfer_out.write(7)?;
      }
      3 => {
        expander.x1.write(11)?;
      }
      _ => panic!("{} is not a valid keypad input", value),
    }

    gen(&extrude, 7, 0)?;
    expander.x1.write(0)?;

    Ok(())
  });

  let output_ctrl = Controller::new("output", move |reg| {
    transfer_in.sleep()?;
    reg.acc = transfer_in.read()?;
    while reg.acc > 0 {
      expander.x0.write(rom.data0.read()?)?;
      sleep(1)?;
      reg.acc -= 1;
    }

    Ok(())
  });

  let (sender, mut scheduler) = Scheduler::new(2);

  let t = Controller::start(main_ctrl, sender.clone());
  let t2 = Controller::start(output_ctrl, sender);

  let mut timestep = 0;

  while timestep < 40 {
    match timestep {
      2 => keypad.inject(1),
      13 => keypad.inject(2),
      25 => keypad.inject(3),
      _ => (),
    };

    println!(
      "output: {:3} {:3} {:3} {:3}",
      p0.load(Ordering::Relaxed),
      p1.load(Ordering::Relaxed),
      p2.load(Ordering::Relaxed),
      extrude_clone.load(Ordering::Relaxed)
    );
    scheduler.advance();
    timestep += 1;
  }

  scheduler.end();

  t.join().unwrap();
  t2.join().unwrap();
}
