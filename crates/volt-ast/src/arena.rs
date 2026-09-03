//! Tip-güvenli arena ve indeks tipleri (ast-nodes.md §1).

use std::marker::PhantomData;

/// Tip-güvenli arena indeksi.
pub struct Idx<T> {
    raw: u32,
    _marker: PhantomData<fn() -> T>,
}

impl<T> Idx<T> {
    pub fn raw(self) -> u32 {
        self.raw
    }
}

// Manuel impl: T: Clone gerektirmemek için (ast-nodes.md §1)
impl<T> Clone for Idx<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for Idx<T> {}
impl<T> PartialEq for Idx<T> {
    fn eq(&self, other: &Self) -> bool {
        self.raw == other.raw
    }
}
impl<T> Eq for Idx<T> {}
impl<T> std::hash::Hash for Idx<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.raw.hash(state)
    }
}
impl<T> std::fmt::Debug for Idx<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Idx({})", self.raw)
    }
}

/// Düğümleri ardışık bellekte tutan arena.
#[derive(Debug, Clone)]
pub struct Arena<T> {
    items: Vec<T>,
}

impl<T> Default for Arena<T> {
    fn default() -> Self {
        Self { items: Vec::new() }
    }
}

impl<T> Arena<T> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn alloc(&mut self, value: T) -> Idx<T> {
        let idx = self.items.len() as u32;
        self.items.push(value);
        Idx {
            raw: idx,
            _marker: PhantomData,
        }
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (Idx<T>, &T)> {
        self.items.iter().enumerate().map(|(i, item)| {
            (
                Idx {
                    raw: i as u32,
                    _marker: PhantomData,
                },
                item,
            )
        })
    }
}

impl<T> std::ops::Index<Idx<T>> for Arena<T> {
    type Output = T;
    fn index(&self, idx: Idx<T>) -> &T {
        &self.items[idx.raw as usize]
    }
}

impl<T> std::ops::IndexMut<Idx<T>> for Arena<T> {
    fn index_mut(&mut self, idx: Idx<T>) -> &mut T {
        &mut self.items[idx.raw as usize]
    }
}
