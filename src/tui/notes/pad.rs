use std::fs;
use std::ops::{Deref, DerefMut};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Persist {
    Skipped,
    Written,
    Blocked,
    Failed(String),
}

#[derive(Debug)]
pub struct Guarded<T> {
    value: T,
    saved: Option<T>,
    warned: bool,
}

impl<T: Clone + Default> Default for Guarded<T> {
    fn default() -> Self {
        Self::loaded(T::default())
    }
}

impl<T: Clone> Guarded<T> {
    pub fn loaded(value: T) -> Self {
        Self {
            saved: Some(value.clone()),
            value,
            warned: false,
        }
    }

    pub fn unreadable() -> Self
    where
        T: Default,
    {
        Self {
            value: T::default(),
            saved: None,
            warned: false,
        }
    }

    pub fn blocked(&self) -> bool {
        self.saved.is_none()
    }
}

impl<T: Clone + PartialEq> Guarded<T> {
    #[cfg(test)]
    pub fn is_dirty(&self) -> bool {
        matches!(&self.saved, Some(saved) if saved != &self.value)
    }

    pub fn persist_with(&mut self, write: impl FnOnce(&T) -> Result<(), String>) -> Persist {
        let Some(saved) = &self.saved else {
            if self.warned {
                return Persist::Skipped;
            }
            self.warned = true;
            return Persist::Blocked;
        };
        if saved == &self.value {
            return Persist::Skipped;
        }
        match write(&self.value) {
            Ok(()) => {
                self.saved = Some(self.value.clone());
                Persist::Written
            }
            Err(err) => Persist::Failed(err),
        }
    }
}

impl<T> Deref for Guarded<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.value
    }
}

impl<T> DerefMut for Guarded<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.value
    }
}

pub fn load_guarded<T: Clone + Default>(
    path: &Path,
    parse: impl FnOnce(&str) -> Option<T>,
) -> Guarded<T> {
    match fs::read_to_string(path) {
        Ok(raw) => match parse(&raw) {
            Some(value) => Guarded::loaded(value),
            None => Guarded::unreadable(),
        },
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Guarded::default(),
        Err(_) => Guarded::unreadable(),
    }
}
