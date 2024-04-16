use std::thread::{self, JoinHandle};

pub struct Server<T> {
    handler: JoinHandle<T>,
}
impl<T: Send + 'static> Server<T> {
    pub fn new<F: FnOnce() -> T + Send + 'static>(task: F) -> std::io::Result<Self> {
        Ok(Self {handler: thread::Builder::new().name("ServerInstance".to_owned()).spawn(task)?}) 
    }
    pub fn has_closed(&self) -> bool {
        self.handler.is_finished()
    }
    pub fn block_until_closed(self) -> std::thread::Result<T> {
        self.handler.join()
    }
}