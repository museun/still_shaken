use crate::util;

use anyhow::Context as _;
use std::{any::Any, any::TypeId, collections::HashMap};

#[derive(Default)]
pub struct State {
    map: HashMap<TypeId, Box<dyn Any + Send + Sync + 'static>>,
}

impl State {
    pub fn contains<T>(&self) -> bool
    where
        T: Send + Sync + 'static,
    {
        self.map.contains_key(&TypeId::of::<T>())
    }

    pub fn remove<T>(&mut self) -> anyhow::Result<()>
    where
        T: Send + Sync + 'static,
    {
        self.map
            .remove(&TypeId::of::<T>())
            .map(|_| ())
            .with_context(|| format!("could not find '{}'", util::type_name::<T>()))
    }

    pub fn insert<T>(&mut self, item: T) -> anyhow::Result<()>
    where
        T: Send + Sync + 'static,
    {
        if self.map.insert(TypeId::of::<T>(), Box::new(item)).is_some() {
            anyhow::bail!("'{}' already existed in state", util::type_name::<T>())
        }

        Ok(())
    }

    pub fn get<T>(&self) -> anyhow::Result<&T>
    where
        T: Send + Sync + 'static,
    {
        self.map
            .get(&TypeId::of::<T>())
            .and_then(|item| item.downcast_ref::<T>())
            .with_context(|| format!("cannot get '{}'", util::type_name::<T>()))
    }

    pub fn get_mut<T>(&mut self) -> anyhow::Result<&mut T>
    where
        T: Send + Sync + 'static,
    {
        self.map
            .get_mut(&TypeId::of::<T>())
            .and_then(|item| item.downcast_mut::<T>())
            .with_context(|| format!("cannot get '{}'", util::type_name::<T>()))
    }
}
