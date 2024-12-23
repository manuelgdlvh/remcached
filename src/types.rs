use std::any::Any;
use std::error::Error;

pub type GenericError = Box<dyn Error + Send + Sync>;
pub type GenericType = Box<dyn Any + Send>;



