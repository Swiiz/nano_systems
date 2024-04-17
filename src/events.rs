use any_key::AnyHash;

pub struct EventQueue {
    events: Vec<Box<dyn AnyHash + Send + Sync>>,
}

impl EventQueue {
    pub fn new() -> Self {
        Self { events: Vec::new() }
    }

    pub fn push<T: AnyHash + Send + Sync>(&mut self, event: T) {
        self.events.push(Box::new(event))
    }

    pub(crate) fn drain(&mut self) -> Vec<Box<dyn AnyHash + Send + Sync>> {
        let new_cap = self.events.len() / 3 * 2;
        std::mem::replace(&mut self.events, Vec::with_capacity(new_cap))
    }
}
