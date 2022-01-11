//! An environment that mimics the behavior of the game SHENZHEN I/O by Zachtronics.
//! <https://www.zachtronics.com/shenzhen-io/>
//!
//! This library isn't intended to strictly reimplement the game, but rather to provide a similar
//! and more flexible environment, so you can solve harder levels by first writing a more natural
//! program and gradually evolving it into the game's restrictive form.
//!
//! To mimic a game level, create one or more structs implementing [controller::Controller], and
//! then run them using [scheduler::Scheduler]. Controller structs will generally contain fields
//! for the buses connected to them. Simple I/O is modeled as `Arc<AtomicI32>`. XBus has more
//! complex behavior and is modeled by [xbus::XBus].

pub mod components;
pub mod controller;
pub mod filerunner;
pub mod scheduler;
pub mod xbus;
