use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::Arc;

use shenzhen_vm::components::{inputsource, memory};
use shenzhen_vm::controller::Controller;
use shenzhen_vm::scheduler::{sleep, Scheduler};
use shenzhen_vm::xbus::XBus;
use shenzhen_vm::{dgt, dst, rd};

fn get_input() -> Vec<(i32, i32, i32)> {
  vec![
    (2, 7, 4),
    (2, 9, 4),
    (2, 8, 5),
    (2, 7, 6),
    (3, 8, 2),
    (2, 7, 3),
    (2, 1, 1),
    (2, 1, 0),
    (2, 4, 5),
    (2, 1, 3),
    (2, 0, 4),
    (3, 1, 7),
    (4, 6, 8),
    (2, 7, 7),
    (2, 9, 2),
    (2, 8, 1),
    (0, 9, 5),
    (2, 7, 0),
  ]
}

fn main() {
  // Input
  let (radio, radio_bus) = inputsource::nonblocking();
  let ram = memory::ram();
  let a0 = ram.addr0.clone();
  let a1 = ram.addr1.clone();
  let d1 = ram.data1.clone();

  // Output
  let harvest = Arc::new(AtomicI32::new(0));
  let motor_x = Arc::new(AtomicI32::new(0));
  let motor_y = Arc::new(AtomicI32::new(0));

  let harvest_out = harvest.clone();
  let x_out = motor_x.clone();
  let y_out = motor_y.clone();

  let to_peeker = XBus::new();
  let from_input_converter = to_peeker.clone();

  let to_splitter = XBus::new();
  let from_peeker = to_splitter.clone();

  let searcher_io = XBus::new();
  let to_searcher = searcher_io.clone();

  let motor_x_io = XBus::new();
  let to_motor_x = motor_x_io.clone();

  let motor_y_io = XBus::new();
  let to_motor_y = motor_y_io.clone();

  // Read two consecutive inputs from the radio, pack them into a single int, and write them into
  // RAM at the write pointer. Send the updated write pointer to the Peeker.
  let input_converter = Controller::new("input_converter", move |reg| {
    reg.acc = radio_bus.read()?;
    if reg.acc != -999 {
      reg.acc *= 10;
      reg.acc += radio_bus.read()?;
      ram.data0.write(reg.acc)?;
    }
    to_peeker.write(ram.addr0.read()?)?;
    sleep(1)?;

    Ok(())
  });

  // Peeker peeks the head of the queue in RAM. It finds the first nonzero entry at or after the
  // original read pointer, sets the read pointer to point to it, and sends the value to Splitter.
  let peeker = Controller::new("peeker", move |reg| {
    from_input_converter.sleep()?;
    reg.acc = from_input_converter.read()?;

    // In-game, you accomplish this with clever use of conditional execution.
    let mut flag = true;
    while ram.addr1.read()? != reg.acc {
      reg.dat = ram.data1.read()?;
      if reg.dat != 0 {
        flag = false;
        reg.acc = ram.addr1.read()?;
        reg.acc -= 1;
        ram.addr1.write(reg.acc)?;
        break;
      }
    }

    if flag {
      // If the queue is empty, send zero to Splitter.
      reg.dat = 0;
    }

    to_splitter.write(reg.dat)?;
    Ok(())
  });

  // Splitter takes the destination from Peeker, splits it into x and y components, and sends those
  // to the motor controllers. It keeps track of the current position, and after updating it, sends
  // it to Searcher.
  let splitter = Controller::new("splitter", move |reg| {
    from_peeker.sleep()?;

    // dat is destination. acc is current position.
    reg.dat = from_peeker.read()?;

    if reg.dat == 0 {
      // Queue was empty. Pretend current position is the destination so that motors stop.
      reg.dat = reg.acc;
    } else {
      // Queue was nonempty. Get ready to separate destination into components. Overwriting the
      // position in acc is fine; we'll reconstruct it from the motor controllers' replies.
      reg.acc = reg.dat;
    }

    dgt!(reg.acc, 1);
    to_motor_x.write(reg.acc)?;
    dst!(reg.acc, 0, reg.dat);
    to_motor_y.write(reg.acc)?;

    dst!(reg.acc, 1, to_motor_x.read()?);
    dst!(reg.acc, 0, to_motor_y.read()?);

    if reg.dat == 0 {
      to_searcher.write(999)?;
    } else {
      to_searcher.write(reg.acc)?;
    }

    Ok(())
  });

  // Searcher takes the current position from Splitter, searches the queue for it, and sets the
  // harvest output appropriately.
  let searcher = Controller::new("searcher", move |reg| {
    searcher_io.sleep()?;
    reg.acc = searcher_io.read()?;
    reg.dat = a1.read()?;

    harvest.store(0, Ordering::Relaxed);

    loop {
      if reg.acc == d1.read()? {
        // Found the value. Go back and overwrite it with zero.
        reg.acc = a1.read()?;
        reg.acc -= 1;
        a1.write(reg.acc)?;
        d1.write(0)?;
        harvest.store(100, Ordering::Relaxed);
        break;
      }

      // Stop once we hit the write pointer.
      if a1.read()? == a0.read()? {
        break;
      }
    }

    a1.write(reg.dat)?;

    Ok(())
  });

  // Each motor controller takes in the x or y component of the current position from Splitter,
  // determines which direction to move and sets the output accordingly. It tracks the position
  // in acc and sends it back to Splitter.
  macro_rules! motor_controller {
    ($name:literal, $io:ident, $output:ident) => {
      Controller::new($name, move |reg| {
        $io.sleep()?;

        let input = $io.read()?;
        let compare = input.cmp(&reg.acc);
        $output.store(50, Ordering::Relaxed);

        // Do this with tcp
        match compare {
          std::cmp::Ordering::Equal => (),
          std::cmp::Ordering::Greater => {
            $output.store(100, Ordering::Relaxed);
            reg.acc += 1;
          }
          std::cmp::Ordering::Less => {
            $output.store(0, Ordering::Relaxed);
            reg.acc -= 1;
          }
        }

        $io.write(reg.acc)?;

        Ok(())
      })
    };
  }

  let motor_controller_x = motor_controller!("motor-x", motor_x_io, motor_x);
  let motor_controller_y = motor_controller!("motor-y", motor_y_io, motor_y);

  let (sender, mut scheduler) = Scheduler::new(6);

  let join_handles = [
    Controller::start(input_converter, sender.clone()),
    Controller::start(peeker, sender.clone()),
    Controller::start(searcher, sender.clone()),
    Controller::start(splitter, sender.clone()),
    Controller::start(motor_controller_x, sender.clone()),
    Controller::start(motor_controller_y, sender),
  ];

  let input = get_input();

  for (wait, xin, yin) in input.iter() {
    for _ in 0..wait - 1 {
      scheduler.advance();
      println!("{:3} {:3} {:3}", rd!(x_out), rd!(y_out), rd!(harvest_out));
    }

    radio.inject(*xin);
    radio.inject(*yin);
    scheduler.advance();
    println!("{:3} {:3} {:3}", rd!(x_out), rd!(y_out), rd!(harvest_out));
  }

  for _ in 0..12 {
    scheduler.advance();
    println!("{:3} {:3} {:3}", rd!(x_out), rd!(y_out), rd!(harvest_out));
  }

  scheduler.end();

  for jh in join_handles.into_iter() {
    jh.join().unwrap();
  }
}
