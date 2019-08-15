#![allow(unstable_name_collisions)]

use std::cell::Cell;
use std::ops::{Deref, DerefMut};

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

#[cfg(not(feature = "nightly"))]
trait CellExt<T: ?Sized> {
    fn from_mut(mut_ref: &mut T) -> &Self;
}

#[cfg(not(feature = "nightly"))]
impl<T: ?Sized> CellExt<T> for Cell<T> {
    fn from_mut(mut_ref: &mut T) -> &Self {
        unsafe {
            #[allow(clippy::transmute_ptr_to_ptr)]
            std::mem::transmute::<&mut T, &Self>(mut_ref)
        }
    }
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
        BitVec::set_bit(&mut value, self.offset, self.value);
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

    pub fn push(&mut self, value: bool) -> BitProxy<'_> {
        let (slot, offset) = Self::index_to_slot(self.len);

        self.len += 1;

        if slot >= self.data.len() {
            self.data.push(0);
        }

        let slot = unsafe { self.data.get_unchecked_mut(slot) };

        Self::set_bit(slot, offset, value);

        BitProxy {
            slot: Cell::from_mut(slot),
            offset,
            value,
        }
    }

    pub fn pop(&mut self) -> Option<bool> {
        self.len = self.len.checked_sub(1)?;

        let (slot, offset) = Self::index_to_slot(self.len);

        unsafe { Some(Self::get_bit(*self.data.get_unchecked(slot), offset)) }
    }

    pub fn get(&self, index: usize) -> Option<bool> {
        if index < self.len {
            unsafe {
                Some(self.get_unchecked(index))
            }
        } else {
            None
        }
    }

    pub unsafe fn get_unchecked(&self, index: usize) -> bool {
        let (slot, offset) = Self::index_to_slot(index);

        Self::get_bit(*self.data.get_unchecked(slot), offset)
    }

    pub fn get_mut(&mut self, index: usize) -> Option<BitProxy<'_>> {
        if index < self.len {
            unsafe {
                Some(self.get_unchecked_mut(index))
            }
        } else {
            None
        }
    }

    pub unsafe fn get_unchecked_mut(&mut self, index: usize) -> BitProxy<'_> {
        let (slot, offset) = Self::index_to_slot(index);

        let slot = self.data.get_unchecked_mut(slot);
        let value = Self::get_bit(*slot, offset);

        BitProxy {
            slot: Cell::from_mut(slot),
            offset,
            value,
        }
    }

    pub unsafe fn set_len(&mut self, len: usize) {
        self.len = len;
    }

    pub fn set(&mut self, index: usize, value: bool) {
        if index < self.len {
            let (slot, offset) = Self::index_to_slot(index);

            unsafe {
                Self::set_bit(self.data.get_unchecked_mut(slot), offset, value);
            }
        } else {
            debug_assert!(false, "out of bounds!");
        }
    }

    pub fn grow(&mut self, additional: usize, value: bool) {
        let new_len = self
            .len
            .checked_add(additional)
            .expect("Capacity overflow!");

        self.data.resize(((self.len + additional) >> 3) + 1, 0);

        let (slot, offset) = Self::index_to_slot(self.len);

        {
            let slot = unsafe { self.data.get_unchecked_mut(slot) };

            for offset in offset..8 {
                Self::set_bit(slot, offset, value);
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

    pub fn iter(&self) -> Iter<'_> {
        Iter {
            vec: self,
            start: 0,
            end: self.len,
        }
    }

    pub fn iter_mut(&mut self) -> IterMut<'_> {
        IterMut {
            inner: IterBytes {
                bytes: self.data.iter_mut(),
                last_len: (self.len & 0b0111) as u8,
            }
            .flatten(),
        }
    }
}

pub struct Iter<'a> {
    vec: &'a BitVec,
    start: usize,
    end: usize,
}

impl Iterator for Iter<'_> {
    type Item = bool;

    fn next(&mut self) -> Option<Self::Item> {
        if self.start < self.end {
            let ret = self.vec.get(self.start)?;

            self.start = self.start.saturating_add(1);

            Some(ret)
        } else {
            None
        }
    }

    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        self.start = self.start.saturating_add(n);

        self.next()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let size = self.end - self.start;

        (size, Some(size))
    }
}

impl DoubleEndedIterator for Iter<'_> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.start < self.end {
            self.end = self.end.saturating_sub(1);

            self.vec.get(self.end)
        } else {
            None
        }
    }

    #[cfg(feature = "nightly")]
    fn nth_back(&mut self, n: usize) -> Option<Self::Item> {
        self.end = self.end.saturating_sub(n);

        self.next_back()
    }
}

impl ExactSizeIterator for Iter<'_> {}
impl std::iter::FusedIterator for Iter<'_> {}

pub struct IterMut<'a> {
    inner: std::iter::Flatten<IterBytes<'a>>,
}

struct IterBytes<'a> {
    bytes: std::slice::IterMut<'a, u8>,
    last_len: u8,
}

struct ByteIter<'a> {
    byte: &'a Cell<u8>,
    offset: u8,
    max: u8,
}

impl<'a> Iterator for IterMut<'a> {
    type Item = BitProxy<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

impl<'a> DoubleEndedIterator for IterMut<'a> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.inner.next_back()
    }
}

impl<'a> Iterator for IterBytes<'a> {
    type Item = ByteIter<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let byte = self.bytes.next()?;

        let max = if self.bytes.size_hint().0 == 0 {
            // if last
            std::mem::replace(&mut self.last_len, 8)
        } else {
            8
        };

        Some(ByteIter {
            byte: Cell::from_mut(byte),
            offset: 0,
            max,
        })
    }
}

impl DoubleEndedIterator for IterBytes<'_> {
    fn next_back(&mut self) -> Option<Self::Item> {
        let byte = self.bytes.next_back()?;

        let max = std::mem::replace(&mut self.last_len, 8);

        Some(ByteIter {
            byte: Cell::from_mut(byte),
            offset: 0,
            max,
        })
    }
}

impl<'a> Iterator for ByteIter<'a> {
    type Item = BitProxy<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let offset = self.offset;

        if offset < self.max {
            let value = BitVec::get_bit(self.byte.get(), offset);
            self.offset += 1;

            Some(BitProxy {
                slot: self.byte,
                offset,
                value,
            })
        } else {
            None
        }
    }
}

impl<'a> DoubleEndedIterator for ByteIter<'a> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.offset < self.max {
            self.max -= 1;
            let value = BitVec::get_bit(self.byte.get(), self.max);

            Some(BitProxy {
                slot: self.byte,
                offset: self.max,
                value,
            })
        } else {
            None
        }
    }
}

impl std::iter::FusedIterator for IterMut<'_> {}
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
    type IntoIter = Iter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a> IntoIterator for &'a mut BitVec {
    type Item = BitProxy<'a>;
    type IntoIter = IterMut<'a>;

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
