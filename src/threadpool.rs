use std::{
    sync::{
        atomic::{AtomicU32, Ordering},
        mpsc, Arc, Mutex,
    },
    thread,
};

use crate::systems::{GlobalsCell, System};

type Sender = Arc<mpsc::Sender<Arc<System>>>;
type Receiver = Arc<Mutex<mpsc::Receiver<Arc<System>>>>;

pub struct ThreadPool {
    workers: Vec<Worker>,
    sender: Sender,
    receiver: Receiver,
    running: Arc<AtomicU32>,
}

impl ThreadPool {
    pub fn new(globals: &GlobalsCell) -> Self {
        let (sender, receiver) = mpsc::channel();

        let sender = Arc::new(sender);
        let receiver = Arc::new(Mutex::new(receiver));

        let running = Arc::new(AtomicU32::new(0));

        let workers = (0..thread::available_parallelism()
            .map(|nzu| nzu.into())
            .unwrap_or(num_cpus::get()))
            .map(|id| {
                Worker::new(
                    id,
                    sender.clone(),
                    receiver.clone(),
                    running.clone(),
                    globals.clone(),
                )
            })
            .collect();

        ThreadPool {
            workers,
            sender,
            receiver,
            running,
        }
    }

    pub fn execute(&self, system: Arc<System>) {
        self.running.fetch_add(1, Ordering::SeqCst);
        self.sender.send(system).unwrap()
    }

    pub fn finished_executing(&self) -> bool {
        self.running.load(Ordering::SeqCst) == 0
    }

    pub fn shutdown(mut self) {
        let handles = self
            .workers
            .iter_mut()
            .filter_map(|w| w.thread.take())
            .collect::<Vec<_>>();
        drop(self);
        handles.into_iter().for_each(|t| t.join().unwrap());
    }
}

struct Worker {
    id: usize,
    thread: Option<thread::JoinHandle<()>>,
}

impl Worker {
    fn new(
        id: usize,
        _sender: Sender,
        receiver: Receiver,
        running: Arc<AtomicU32>,
        globals_cell: GlobalsCell,
    ) -> Worker {
        let thread = Some(thread::spawn(move || loop {
            let Ok(system) = receiver.lock().unwrap().recv() else {
                break;
            };

            if let Err(e) = system.run(globals_cell.borrow()) {
                panic!("System errored: {e:?}")
            }
            running.fetch_sub(1, Ordering::SeqCst);
        }));

        Worker { id, thread }
    }
}
