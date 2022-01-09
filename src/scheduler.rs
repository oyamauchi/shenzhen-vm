use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread::JoinHandle;
use std::time::Duration;

use crate::controller::{current_name, send_to_scheduler, start, Controller};
use crate::xbus::XBus;

pub enum SleepToken {
  Time(i32),
  XBusSleep(XBus),
  XBusRead(XBus),
  XBusWrite(XBus),
}

impl Debug for SleepToken {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Time(arg0) => f.debug_tuple("Time").field(arg0).finish(),
      Self::XBusSleep(_) => f.debug_tuple("XBusSleep").finish(),
      Self::XBusRead(_) => f.debug_tuple("XBusRead").finish(),
      Self::XBusWrite(_) => f.debug_tuple("XBusWrite").finish(),
    }
  }
}

fn is_blocking(token: &SleepToken) -> bool {
  match token {
    SleepToken::Time(_) | SleepToken::XBusSleep(_) => false,
    SleepToken::XBusRead(_) | SleepToken::XBusWrite(_) => true,
  }
}

pub type SleepMessage = (&'static str, SleepToken, Sender<bool>);

pub struct Scheduler {
  time: i32,
  join_handles: Vec<JoinHandle<()>>,
  receiver: Receiver<SleepMessage>,
  sleepers: HashMap<&'static str, (SleepToken, Sender<bool>)>,
}

pub fn sleep(steps: i32) -> Result<(), ()> {
  Scheduler::sleep(SleepToken::Time(steps))?;
  Ok(())
}

impl Scheduler {
  /// Sleep until the condition described by the SleepToken is true. The reply is a boolean
  /// indicating whether the system is terminating; if so, this function returns an Err result to
  /// be propagated up to the top level of the thread.
  ///
  /// This function runs on controller threads.
  pub fn sleep(token: SleepToken) -> Result<(), ()> {
    let (wakeup_sender, wakeup_receiver) = channel();
    let name = current_name();

    send_to_scheduler((name, token, wakeup_sender));

    let keep_going = wakeup_receiver.recv().unwrap();

    if keep_going {
      Ok(())
    } else {
      Err(())
    }
  }

  /// Create a new scheduler of the given controllers. All the controller threads will be given a
  /// `Sender` to send sleep messages to the scheduler, and the threads will be started.
  pub fn new(controllers: Vec<Box<dyn Controller + Send>>) -> Scheduler {
    let (sender, receiver) = channel();
    let join_handles: Vec<JoinHandle<()>> = controllers
      .into_iter()
      .map(|ctrl| start(ctrl, sender.clone()))
      .collect();

    Scheduler {
      time: 0,
      receiver,
      sleepers: HashMap::with_capacity(join_handles.len()),
      join_handles,
    }
  }

  /// Wait until we've heard from `expected_count` controllers over the channel, storing their
  /// sleep tokens and response senders.
  fn await_sleepers(&mut self, expected_count: usize) {
    let mut receive_count = 0;

    while receive_count < expected_count {
      // Wait with a timeout to catch infinite loops in controllers.
      let (name, token, wakeup) = self
        .receiver
        .recv_timeout(Duration::from_millis(500))
        .unwrap();

      // Timestep sleep tokens come in as "for N timestep" -- we need to add the current timestep
      // number to know when to wake up.
      let real_token = match token {
        SleepToken::Time(t) => SleepToken::Time(self.time + t),
        tok => tok,
      };

      self.sleepers.insert(name, (real_token, wakeup));
      receive_count += 1;
    }
  }

  /// Advance the current timestep number, then continuously wake up controller threads whose
  /// sleep conditions are fulfilled (right time reached, XBus now readable, etc.) until none of
  /// them are runnable.
  ///
  /// When a controller is created with `Controller::start`, its body will not execute until this
  /// function is called for the first time.
  ///
  /// This function must be called on the main thread.
  pub fn advance(&mut self) {
    if self.time == 0 {
      // If this is the first timestep, wait until we've populated "sleepers" by waiting until all
      // controllers have reached their initial sleep.
      self.await_sleepers(self.join_handles.len());
    }

    self.time += 1;

    let mut run_count = 1;
    while run_count > 0 {
      run_count = 0;

      for (name, (token, wakeup)) in self.sleepers.iter() {
        let can_run = match token {
          SleepToken::Time(t) => self.time >= *t,
          SleepToken::XBusSleep(bus) => bus.can_read(),
          SleepToken::XBusRead(bus) => !bus.is_read_pending(name),
          SleepToken::XBusWrite(bus) => !bus.is_write_pending(name),
        };

        if can_run {
          wakeup.send(true).unwrap();
          run_count += 1;
        }
      }

      // Wait until we've heard from as many threads as we just woke up.
      self.await_sleepers(run_count);
    }

    // Before we can conclude the timestep, all controllers must be sleeping until a target time
    // ("slp") or sleeping on an XBus ("slx"); they can't be blocked trying to read or write a
    // value to an XBus. If some modules are blocked, there's a deadlock: fail the execution.
    let blockers: Vec<&str> = self
      .sleepers
      .iter()
      .filter(|(_, v)| is_blocking(&v.0))
      .map(|(name, _)| *name)
      .collect();
    if !blockers.is_empty() {
      panic!(
        "No modules are runnable but some are blocking: {:?}",
        self.sleepers
      );
    }
  }

  /// Tell all controller threads to terminate. This function does not actually wait for the
  /// threads to terminate, so the caller should join() the threads if it cares about this.
  pub fn end(mut self) {
    self.time = -1;

    for (_name, (_, wakeup)) in self.sleepers.iter() {
      wakeup.send(false).unwrap();
    }

    for jh in self.join_handles.into_iter() {
      jh.join().unwrap();
    }
  }
}
