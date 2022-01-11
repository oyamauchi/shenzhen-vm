//! An environment that mimics the behavior of the game SHENZHEN I/O by Zachtronics.
//!
//! In the game, you write code in "controllers". Controllers can be connected to each other, and
//! to other types of component, using two different types of bus: simple I/O and "XBus".
//!
//! To mimic a game level, create one or more structs implementing [controller::Controller], and
//! then run them using [scheduler::Scheduler]. Controller structs will generally contain fields
//! for the buses connected to them. Simple I/O is modeled here as `Arc<AtomicI32>`. XBus has more
//! complex behavior and is modeled by [xbus::XBus].
//!
//! In controller code, you can write pretty much anything you want, including stuff that wouldn't
//! be possible within the game. This library isn't intended to strictly reimplement the game, but
//! rather to provide a similar but more flexible environment so you can write a more natural
//! program and gradually evolve it into the game's restrictive form.

pub mod components;
pub mod controller;
pub mod scheduler;
pub mod xbus;
