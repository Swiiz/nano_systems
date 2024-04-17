use std::{
    any::Any,
    collections::HashMap,
    fmt::Debug,
    ops::{Deref, DerefMut},
    sync::Arc,
};

use any_key::AnyHash;
use atomic_refcell::{AtomicRef, AtomicRefCell};

use crate::{
    events::EventQueue,
    globals::{
        GlobalMut, GlobalRef, Globals, GlobalsCommandQueue, IntoGlobalKey, IntoSingletonKey,
        Singleton, SingletonKey,
    },
    threadpool::ThreadPool,
};

/// The `Scheduler` allows for systems declaration, scheduling and execution
/// Systems can be scheduled to run when the scheduler receives a certain event
/// Events can be anything Hashable and Send + Sync, see: https://docs.rs/any_key/latest/any_key/
pub struct Scheduler {
    systems: HashMap<Box<dyn AnyHash>, Vec<Arc<System>>>, // Event typeid to system
}

pub(crate) type GlobalsCell = Arc<AtomicRefCell<Globals>>;
impl Scheduler {
    pub fn new() -> Self {
        Self {
            systems: HashMap::new(),
        }
    }

    pub fn on<T>(&mut self, event: impl AnyHash + Send + Sync, sys: impl IntoSystem<T>) {
        self.systems
            .entry(Box::new(event))
            .or_default()
            .push(Arc::new(sys.into_system()));
    }

    pub fn run<T: AnyHash + Send + Sync>(self, start_event: T, globals: Globals) -> Globals {
        let globals_cell: GlobalsCell = Arc::new(AtomicRefCell::new(globals));
        {
            let thread_pool = ThreadPool::new(&globals_cell);
            let mut globals = globals_cell.borrow_mut();

            let mut event_queue = EventQueue::new();
            event_queue.push(start_event);
            globals.insert(Singleton(event_queue));

            drop(globals); // release mutable borrow

            while let Some(events) = {
                let globals = globals_cell.borrow(); // Not in scope when the loop runs
                let mut event_queue = globals
                    .get_mut(Singleton::<EventQueue>::key())
                    .expect("Could not retrieve event queue global!");

                Some(event_queue.drain())
                    .filter(|events| events.len() > 0 || !thread_pool.finished_executing())
            } {
                events
                    .into_iter()
                    .filter_map(|event| self.systems.get(&(event as Box<dyn AnyHash>)))
                    .for_each(|eventsyss| {
                        while !thread_pool.finished_executing() {}

                        globals_cell.borrow_mut().update_command_queue();

                        for system in eventsyss {
                            thread_pool.execute(system.clone());
                        }
                    });
            }

            thread_pool.shutdown();
        }
        Arc::into_inner(globals_cell).unwrap().into_inner()
    }
}

pub struct System {
    wrapped_fn:
        Arc<dyn Fn(AtomicRef<Globals>) -> Result<(), Box<dyn CustomSystemError>> + Send + Sync>, // Box<dyn Any> is the event
}

impl System {
    pub fn run(&self, globals: AtomicRef<Globals>) -> Result<(), Box<dyn CustomSystemError>> {
        (self.wrapped_fn)(globals)
    }
}

pub trait CustomSystemError: Any + Debug {}
impl<T: Any + Debug> CustomSystemError for T {}

pub trait IntoSystem<T> {
    fn into_system(self) -> System;
}

/// Wraps a `&Globals` to avoid deadlocks.
/// Use with the `access!` macro
pub struct GlobalAccess<'a> {
    pub inner_may_deadlock: &'a Globals,
}

impl<T: CustomSystemError, F: Fn(GlobalAccess) -> Result<(), T> + 'static + Send + Sync>
    IntoSystem<(T, ())> for F
{
    fn into_system(self) -> System {
        System {
            wrapped_fn: Arc::new(move |g| {
                self(GlobalAccess {
                    inner_may_deadlock: g.deref(),
                })
                .map_err(|e| Box::new(e) as Box<dyn CustomSystemError>)
            }),
        }
    }
}

impl<F: Fn(GlobalAccess) + 'static + Send + Sync> IntoSystem<()> for F {
    fn into_system(self) -> System {
        System {
            wrapped_fn: Arc::new(move |g| {
                Ok(self(GlobalAccess {
                    inner_may_deadlock: g.deref(),
                }))
            }),
        }
    }
}
