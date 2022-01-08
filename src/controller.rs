use std::sync::mpsc::Sender;
use std::thread;
use std::{cell::RefCell, mem::MaybeUninit};

use crate::scheduler::{Scheduler, SleepMessage, SleepToken};

/// A controller's state that persists across repeated executions of its body closure.
#[derive(Debug)]
pub struct State {
  pub acc: i32,
  pub dat: i32,
}

#[derive(Debug)]
pub struct Controller<T>
where
  T: FnMut(&mut State) -> Result<(), ()> + Send + 'static,
{
  pub name: &'static str,
  pub state: State,
  pub execute: T,
}

thread_local! {
  /// The name of the current controller
  static CONTROLLER_NAME: RefCell<&'static str> = RefCell::new("");

  /// The sending half of a channel that the current controller should use to communicate with the
  /// scheduler.
  static SENDER: RefCell<MaybeUninit<Sender<SleepMessage>>> = RefCell::new(MaybeUninit::uninit());
}

pub fn current_name() -> &'static str {
  CONTROLLER_NAME.with(|cell| *cell.borrow())
}

pub fn send_to_scheduler(message: SleepMessage) {
  SENDER.with(|cell| {
    unsafe { cell.borrow().assume_init_ref() }
      .send(message)
      .unwrap()
  })
}

/// Represents a controller (a component with code) in the game.
impl<T> Controller<T>
where
  T: FnMut(&mut State) -> Result<(), ()> + Send + 'static,
{
  pub fn new(name: &'static str, execute: T) -> Controller<T> {
    Controller {
      name,
      state: State { acc: 0, dat: 0 },
      execute,
    }
  }

  pub fn start(mut ctrl: Controller<T>, sender: Sender<SleepMessage>) -> thread::JoinHandle<()> {
    thread::Builder::new()
      .name(ctrl.name.into())
      .spawn(move || {
        // Set up thread-local state
        CONTROLLER_NAME.with(|cell| *cell.borrow_mut() = ctrl.name);
        SENDER.with(|cell| {
          cell.borrow_mut().write(sender);
        });

        // Don't start executing the body until the first advance() call
        Scheduler::sleep(SleepToken::Time(0)).unwrap();

        loop {
          match (ctrl.execute)(&mut ctrl.state) {
            Ok(_) => (),
            Err(_) => break,
          }
        }
      })
      .unwrap()
  }
}
