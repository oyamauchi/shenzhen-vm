//! A very simple controller that's mostly to demonstrate [FileRunner].

use std::collections::HashMap;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::Arc;

use shenzhen_vm::components::{inputsource, outputsink};
use shenzhen_vm::controller::{Controller, Regs};
use shenzhen_vm::filerunner::{FileRunner, InputBus, OutputBus};
use shenzhen_vm::rd;
use shenzhen_vm::scheduler::Scheduler;
use shenzhen_vm::xbus::XBus;

struct Math {
  input_a: XBus,
  input_b: Arc<AtomicI32>,
  output_added: XBus,
  output_subtracted: Arc<AtomicI32>,
}

impl Controller for Math {
  fn name(&self) -> &'static str {
    "math"
  }
  fn execute(&self, _reg: &mut Regs) -> Result<(), ()> {
    self.input_a.sleep()?;
    let a = self.input_a.read()?;
    let b = rd!(self.input_b);

    self.output_added.write(a + b)?;
    self.output_subtracted.store(a - b, Ordering::Relaxed);
    Ok(())
  }
}

const CSV: &[u8] = b"in input_a,in input_b,out added,out subtracted
2,3,5,-1
10,7,17,3
,,,3
3 4 5,10,13 14 15,-5
";

fn main() {
  let (input_a, input_a_bus) = inputsource::blocking();
  let input_b = Arc::new(AtomicI32::new(0));

  let (added, added_bus) = outputsink::new("added", true);
  let subtracted = Arc::new(AtomicI32::new(0));

  let mut scheduler = Scheduler::new(vec![Box::new(Math {
    input_a: input_a_bus,
    input_b: input_b.clone(),
    output_added: added_bus,
    output_subtracted: subtracted.clone(),
  })]);

  let mut csv = CSV;
  let mut runner = FileRunner::new(&mut csv).unwrap();

  let time_count = runner
    .verify(
      &mut scheduler,
      HashMap::from([
        ("input_a", InputBus::XBus(&input_a)),
        ("input_b", InputBus::Simple(&input_b)),
      ]),
      HashMap::from([
        ("added", OutputBus::XBus(&added)),
        ("subtracted", OutputBus::Simple(&subtracted)),
      ]),
    )
    .unwrap();

  println!("Verified {} timesteps", time_count);

  scheduler.end();
}
