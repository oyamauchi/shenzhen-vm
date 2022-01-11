# shenzhen-vm

An environment that mimics the behavior of the game SHENZHEN I/O by Zachtronics.
<https://www.zachtronics.com/shenzhen-io/>

This library isn't intended to strictly reimplement the game, but rather to
provide a similar and more flexible environment, so you can solve harder levels
by first writing a more natural program and gradually evolving it into the
game's restrictive form.

To mimic a game level, create one or more structs implementing
`controller::Controller`, and then run them using `scheduler::Scheduler`.
Controller structs will generally contain fields for the buses connected to
them. Simple I/O is modeled as `Arc<AtomicI32>`. XBus has more complex behavior
and is modeled by `xbus::XBus`.

## How to Use

See the `examples`. The general idea is:

1. Define one struct per controller, and make it implement `Controller`. Give it
   fields representing the buses connected to it. Write its code in the
   `execute` associated function. You can write whatever code and define
   whatever fields you want (as long as the struct remains `Send`). To stay
   within the spirit of the game, don't use local variables; only use the `acc`
   and `dat` registers which are passed in to `execute`. Don't use complex
   expressions. Only call `sleep`, `XBus::sleep`, `XBus::read`, and
   `XBus::write`. Use the `?` operator on any call to those functions.
2. In `main()`, instantiate any other components you need (RAM/ROM modules,
   expanders) and any buses needed to communicate between the controllers. The
   components from `components` generally provide their own XBuses. If you need
   a controller-to-controller XBus, just call `XBus::new()`.
3. Also in `main()`, instantiate your structs, passing in clones of the buses
   you just created.
4. Pass those structs to `Scheduler::new`, then call `Scheduler::advanced` to
   run them.
5. Call `Scheduler::end` to shut down the threads.

## Known Issues

- Simple I/O is modeled as an `AtomicI32`, i.e. a single value that can be
  freely read and written by anything that can see it, and that's not exactly
  how it works in-game. In-game, simple I/O pins are in "read" or "write" mode,
  and the value read from a bus is the _highest_ one currently being written,
  not the last one as will happen in `shenzhen-vm`.

- All arithmetic in the game is clamped to `[-999, 999]`. Here, it's full 32-bit
  signed arithmetic. Values on simple I/O in the game are clamped to `[0, 100]`,
  while here they're also full 32-bit signed ints.
