use std::error::Error;

pub trait DatabaseInput { }

pub trait DatabaseObject<T, U> {
    // // database operations
    fn create (&self, value: T) -> Result<U, Box<dyn Error>>;
    fn read (&self, value: T) -> Result<U, Box<dyn Error>>;
    fn update (&self, value: T) -> Result<U, Box<dyn Error>>;
    fn delete (&self, value: T) -> Result<U, Box<dyn Error>>;
}
