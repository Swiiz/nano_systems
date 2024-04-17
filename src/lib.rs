pub mod events;
pub mod globals;
pub mod macros;
pub mod systems;
pub(crate) mod threadpool;

// TODO: Globals
//   - Thread locals
// TODO: Events system
//   - Before/After events?
//TODO: Module system (is it needed?)

//TODO: The "GlobalInsertionBuffer" and "GlobalRemovalBuffer" type:
// Insertion and removal of globals is done through buffers stored as globals
// which are drained by the scheduler to then modify the Globals with the changes.

//TODO: Remove the event queue from the globals, and use system return value to emit new ones ( src/systems.rs:40 )
