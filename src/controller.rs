//! A trait representing controllers, plus a few macros mimicking complex game instructions.

use std::cell::RefCell;
use std::mem::MaybeUninit;
use std::sync::mpsc::Sender;
use std::thread;

use crate::scheduler::{Scheduler, SleepMessage, SleepToken};

/// A controller's state that persists across repeated executions of its `execute` function.
#[derive(Debug)]
pub struct Regs {
  pub acc: i32,
  pub dat: i32,
}

/// Represents a controller with code.
///
/// Each controller is run on its own thread, so they have to implement `Send`. If a controller is
/// implemented in the spirit of the game, its only fields will be of `Send` types `XBus` and
/// `Arc<AtomicI32>`, so this will take care of itself.
pub trait Controller {
  /// Returns the name of the controller. This is used to name the thread, and as a unique key for
  /// when the thread is queueing in the scheduler.
  fn name(&self) -> &'static str;

  /// The controller's code. The `acc` and `dat` registers are passed in as a struct. It should
  /// return `Ok(())` at the end, and propagate errors from any Result-returning function it calls
  /// (i.e. `sleep`, `XBus::sleep`, `XBus::read`, and `XBus::write`).
  ///
  /// This function will be executed repeatedly until the Scheduler running the controller ends.
  fn execute(&self, _: &mut Regs) -> Result<(), ()>;
}

thread_local! {
  /// The name of the current controller
  static CONTROLLER_NAME: RefCell<&'static str> = RefCell::new("");

  /// The sending half of a channel that the current controller should use to communicate with the
  /// scheduler.
  static SENDER: RefCell<MaybeUninit<Sender<SleepMessage>>> = RefCell::new(MaybeUninit::uninit());
}

pub(crate) fn current_name() -> &'static str {
  CONTROLLER_NAME.with(|cell| *cell.borrow())
}

pub(crate) fn send_to_scheduler(message: SleepMessage) {
  SENDER.with(|cell| {
    unsafe { cell.borrow().assume_init_ref() }
      .send(message)
      .unwrap()
  })
}

pub(crate) fn start(
  ctrl: Box<dyn Controller + Send>,
  sender: Sender<SleepMessage>,
) -> thread::JoinHandle<()> {
  thread::Builder::new()
    .name(ctrl.name().into())
    .spawn(move || {
      // Set up thread-local state
      CONTROLLER_NAME.with(|cell| *cell.borrow_mut() = ctrl.name());
      SENDER.with(|cell| {
        cell.borrow_mut().write(sender);
      });

      // Don't start executing the body until the first advance() call
      Scheduler::sleep(SleepToken::Time(0)).unwrap();

      let mut state = Regs { acc: 0, dat: 0 };

      loop {
        match ctrl.execute(&mut state) {
          Ok(_) => (),
          Err(_) => break,
        }
      }
    })
    .unwrap()
}

#[macro_export]
macro_rules! dgt {
  ($acc:expr, $index:expr) => {
    $acc = match $index {
      0 => $acc % 10,
      1 => ($acc / 10) % 10,
      2 => $acc / 100,
      _ => 0,
    }
  };
}

#[macro_export]
macro_rules! dst {
  ($acc:expr, $index:expr, $value:expr) => {
    let digit = $value % 10;
    $acc = match $index {
      0 => ($acc / 10) * 10 + digit,
      1 => ($acc / 100) * 100 + (digit * 10) + $acc % 10,
      2 => (digit * 100) + ($acc % 100),
      _ => $acc,
    }
  };
}

#[macro_export]
macro_rules! gen {
  ($pin:expr, $on_steps:expr, $off_steps:expr) => {
    if $on_steps > 0 {
      $pin.store(100, Ordering::Relaxed);
      sleep($on_steps)?;
    }
    $pin.store(0, Ordering::Relaxed);
    if $off_steps > 0 {
      sleep($off_steps)?;
    }
  };
}

#[macro_export]
macro_rules! rd {
  ($arc_atomic:expr) => {
    $arc_atomic.load(Ordering::Relaxed)
  };
}
