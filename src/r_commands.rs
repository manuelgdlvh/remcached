use std::fmt::Display;
use std::future::Future;
use std::hash::Hash;

use crate::types::GenericError;

pub trait RCommands: Send + Sync
{
    type Key: Eq + Hash + Clone + Display + Send + Sync;
    type Value: Clone + Send + Sync;

    fn get(&self, key: &Self::Key) -> impl Future<Output=Result<Option<Self::Value>, GenericError>> + Send;
    fn put(&self, key: &Self::Key, value: &Self::Value) -> impl Future<Output=Result<(), GenericError>> + Send;
}