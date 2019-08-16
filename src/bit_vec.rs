#![allow(unstable_name_collisions)]

use std::cell::Cell;
use std::ops::{Deref, DerefMut};

pub mod slice;

fn index_to_slot(index: usize) -> (usize, u8) {
    let slot = index >> 3;
    let offset = (index & 0b0111) as u8;

    (slot, offset)
}

fn set_bit(slot: &mut u8, offset: u8, value: bool) {
    *slot = (*slot & !(1 << offset)) | ((value as u8) << offset);
}

fn get_bit(slot: u8, offset: u8) -> bool {
    (slot & (1 << offset)) != 0
}

#[derive(Default, Clone)]
pub struct BitVec {
    data: Vec<u8>,
    len: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AllocInfo {
    pub len: usize,
    pub cap: usize,
    _priv: (),
}

pub struct BitProxy<'a> {
    slot: &'a Cell<u8>,
    offset: u8,
    value: bool,
}

impl Deref for BitProxy<'_> {
    type Target = bool;

    fn deref(&self) -> &bool {
        &self.value
    }
}

impl DerefMut for BitProxy<'_> {
    fn deref_mut(&mut self) -> &mut bool {
        &mut self.value
    }
}

impl Drop for BitProxy<'_> {
    fn drop(&mut self) {
        self.flush();
    }
}

impl BitProxy<'_> {
    pub fn flush(&self) {
        let mut value = self.slot.get();
        set_bit(&mut value, self.offset, self.value);
        self.slot.set(value);
    }
}

impl BitVec {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_capacity(cap: usize) -> Self {
        Self {
            data: Vec::with_capacity(cap),
            len: 0,
        }
    }

    pub fn alloc_info(&self) -> AllocInfo {
        AllocInfo {
            len: self.data.len(),
            cap: self.data.capacity(),
            _priv: (),
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn reserve(&mut self, additional: usize) {
        self.data.reserve((additional >> 3) + 1);
    }

    pub fn reserve_exact(&mut self, additional: usize) {
        self.data.reserve_exact((additional >> 3) + 1);
    }

    pub fn push(&mut self, value: bool) -> BitProxy<'_> {
        let (slot, offset) = index_to_slot(self.len);

        self.len += 1;

        if slot >= self.data.len() {
            self.data.push(0);
        }

        let slot = unsafe { self.data.get_unchecked_mut(slot) };

        set_bit(slot, offset, value);

        BitProxy {
            slot: Cell::from_mut(slot),
            offset,
            value,
        }
    }

    pub fn pop(&mut self) -> Option<bool> {
        self.len = self.len.checked_sub(1)?;

        let (slot, offset) = index_to_slot(self.len);

        unsafe { Some(get_bit(*self.data.get_unchecked(slot), offset)) }
    }

    pub fn get<'a, S: slice::SliceIndex<slice::BitSlice<'a>>>(
        &'a self,
        index: S,
    ) -> Option<S::Output> {
        self.as_slice().get(index)
    }

    pub unsafe fn get_unchecked<'a, S: slice::SliceIndex<slice::BitSlice<'a>>>(
        &'a self,
        index: S,
    ) -> S::Output {
        self.as_slice().get_unchecked(index)
    }

    pub fn get_mut<'a, S: slice::SliceIndexMut<slice::BitSliceMut<'a>>>(
        &'a mut self,
        index: S,
    ) -> Option<S::Output> {
        self.as_mut_slice().into_get_mut(index)
    }

    pub unsafe fn get_unchecked_mut<'a, S: slice::SliceIndexMut<slice::BitSliceMut<'a>>>(
        &'a mut self,
        index: S,
    ) -> S::Output {
        self.as_mut_slice().into_get_unchecked_mut(index)
    }

    pub unsafe fn set_len(&mut self, len: usize) {
        self.len = len;
    }

    pub fn set(&mut self, index: usize, value: bool) {
        self.as_mut_slice().set(index, value);
    }

    pub fn grow(&mut self, additional: usize, value: bool) {
        let new_len = self
            .len
            .checked_add(additional)
            .expect("Capacity overflow!");

        self.data.resize(((self.len + additional) >> 3) + 1, 0);

        let (slot, offset) = index_to_slot(self.len);

        {
            let slot = unsafe { self.data.get_unchecked_mut(slot) };

            for offset in offset..8 {
                set_bit(slot, offset, value);
            }
        }

        let blocks = new_len.saturating_sub(self.len + (8u8 - offset) as usize);
        self.len = new_len;

        let block_value: u8 = if value { !0 } else { 0 };

        if blocks != 0 {
            let blocks = (blocks >> 3) + 1;
            let slot = slot + 1;
            let iter = unsafe { self.data.get_unchecked_mut(slot..).iter_mut().take(blocks) };

            for block in iter {
                *block = block_value;
            }
        }
    }

    pub fn clear(&mut self) {
        self.data.clear();
        self.len = 0;
    }

    pub fn set_all(&mut self, value: bool) {
        let value = if value { !0 } else { 0 };

        for i in &mut self.data {
            *i = value;
        }
    }

    pub fn iter(&self) -> slice::Iter<'_> {
        self.as_slice().iter()
    }

    pub fn iter_mut(&mut self) -> slice::IterMut<'_> {
        self.as_mut_slice().iter_mut()
    }
}

pub struct IntoIter {
    vec: BitVec,
    index: usize,
}

impl Iterator for IntoIter {
    type Item = bool;

    fn next(&mut self) -> Option<Self::Item> {
        let ret = self.vec.get(self.index)?;

        self.index = self.index.saturating_add(1);

        Some(ret)
    }

    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        self.index = self.index.saturating_add(n);

        self.next()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let size = self.vec.len - self.index;

        (size, Some(size))
    }
}

impl DoubleEndedIterator for IntoIter {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.vec.len > self.index {
            self.vec.pop()
        } else {
            None
        }
    }

    #[cfg(feature = "nightly")]
    fn nth_back(&mut self, n: usize) -> Option<Self::Item> {
        self.vec.len = self.vec.len.saturating_sub(n);

        self.next_back()
    }
}

impl ExactSizeIterator for IntoIter {}
impl std::iter::FusedIterator for IntoIter {}

impl IntoIterator for BitVec {
    type Item = bool;
    type IntoIter = IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        IntoIter {
            vec: self,
            index: 0,
        }
    }
}

impl<'a> IntoIterator for &'a BitVec {
    type Item = bool;
    type IntoIter = slice::Iter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a> IntoIterator for &'a mut BitVec {
    type Item = BitProxy<'a>;
    type IntoIter = slice::IterMut<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

#[test]
fn bit_vec() {
    fn _print(vec: &BitVec) {
        for i in vec.data.iter() {
            print!("{:08b} ", i);
        }
        println!();
    }

    let mut vec = BitVec::new();

    vec.push(true);
    vec.push(true);
    vec.push(false);

    assert_eq!(vec.get(0), Some(true));
    assert_eq!(vec.get(1), Some(true));
    assert_eq!(vec.get(2), Some(false));
    assert_eq!(vec.get(3), None);

    vec.set(2, true);
    vec.set(1, false);

    assert_eq!(vec.pop(), Some(true));
    assert_eq!(vec.pop(), Some(false));
    assert_eq!(vec.pop(), Some(true));
    assert_eq!(vec.pop(), None);

    vec.grow(10, true);
    vec.grow(70, false);
    vec.grow(50, true);

    assert!(vec.iter().eq((0..10)
        .map(|_| true)
        .chain((0..70).map(|_| false))
        .chain((0..50).map(|_| true))));

    assert!(vec.clone().into_iter().eq((0..10)
        .map(|_| true)
        .chain((0..70).map(|_| false))
        .chain((0..50).map(|_| true))));

    assert!(vec.iter().rev().eq((0..10)
        .map(|_| true)
        .chain((0..70).map(|_| false))
        .chain((0..50).map(|_| true))
        .rev()));

    assert!(vec.clone().into_iter().rev().eq((0..10)
        .map(|_| true)
        .chain((0..70).map(|_| false))
        .chain((0..50).map(|_| true))
        .rev()));

    for _ in 0..50 {
        assert_eq!(vec.pop(), Some(true));
    }

    for _ in 0..70 {
        assert_eq!(vec.pop(), Some(false));
    }

    for _ in 0..10 {
        assert_eq!(vec.pop(), Some(true));
    }

    assert_eq!(vec.pop(), None);

    vec.grow(100, true);

    assert!((0..100).map(|_| true).eq(vec.iter()));

    vec.set_all(false);

    assert!((0..100).map(|_| false).eq(vec.iter()));
}
