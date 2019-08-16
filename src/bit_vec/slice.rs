use super::{get_bit, index_to_slot, set_bit, BitProxy, BitVec};
pub(super) use crate::slice::{Seal, SliceIndex, SliceIndexMut};

use std::cell::Cell;
use std::marker::PhantomData;
use std::ptr::NonNull;

use std::ops::Deref;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct BitSlice<'a> {
    ptr: NonNull<u8>,
    offset: u8,
    len: usize,
    lt: PhantomData<&'a u8>,
}

#[repr(C)]
pub struct BitSliceMut<'a> {
    ptr: NonNull<u8>,
    offset: u8,
    len: usize,
    lt: PhantomData<&'a mut u8>,
}

unsafe impl Send for BitSlice<'_> where [u8]: Send + Sync {}
unsafe impl Sync for BitSlice<'_> where [u8]: Send + Sync {}

unsafe impl Send for BitSliceMut<'_> where [u8]: Send {}
unsafe impl Sync for BitSliceMut<'_> where [u8]: Send + Sync {}

impl Default for BitSlice<'_> {
    fn default() -> Self {
        Self::empty()
    }
}

impl Default for BitSliceMut<'_> {
    fn default() -> Self {
        Self::empty()
    }
}

impl BitVec {
    pub fn as_slice(&self) -> BitSlice<'_> {
        BitSlice {
            ptr: NonNull::from(self.data.as_slice()).cast(),
            offset: 0,
            len: self.len,
            lt: PhantomData,
        }
    }

    pub fn as_mut_slice(&mut self) -> BitSliceMut<'_> {
        BitSliceMut {
            ptr: NonNull::from(self.data.as_mut_slice()).cast(),
            offset: 0,
            len: self.len,
            lt: PhantomData,
        }
    }
}

impl<'a> BitSliceMut<'a> {
    pub fn into_slice(self) -> BitSlice<'a> {
        *self
    }
}

impl<'a> BitSlice<'a> {
    pub const fn empty() -> Self {
        Self {
            ptr: NonNull::dangling(),
            offset: 0,
            len: 0,
            lt: PhantomData,
        }
    }

    pub const fn len(self) -> usize {
        self.len
    }

    pub const fn is_empty(self) -> bool {
        self.len == 0
    }

    pub unsafe fn get_unchecked<I: SliceIndex<Self>>(self, index: I) -> I::Output {
        index.get_unchecked(self)
    }

    pub fn get<I: SliceIndex<Self>>(self, index: I) -> Option<I::Output> {
        index.get(self)
    }

    pub fn as_ref(&self) -> BitSlice<'_> {
        *self
    }

    pub fn iter(self) -> Iter<'a> {
        self.into_iter()
    }

    pub fn split_at(self, index: usize) -> Option<(Self, Self)> {
        if index <= self.len {
            unsafe { Some(self.split_at_unchecked(index)) }
        } else {
            None
        }
    }

    pub fn split_first(self) -> Option<(bool, Self)> {
        let (first, rest) = self.split_at(1)?;

        let first = unsafe { first.get_unchecked(0) };

        Some((first, rest))
    }

    pub fn split_last(self) -> Option<(Self, bool)> {
        let len = self.len.checked_sub(1)?;

        unsafe {
            let (rest, last) = self.split_at_unchecked(len);

            Some((rest, last.get_unchecked(0)))
        }
    }

    pub unsafe fn split_at_unchecked(self, index: usize) -> (Self, Self) {
        let BitSlice {
            ptr, len, offset, ..
        } = self.get_unchecked(index..);

        let right = BitSlice {
            ptr,
            len,
            offset,
            lt: PhantomData,
        };

        (BitSlice { len: index, ..self }, right)
    }
}

impl<'a> BitSliceMut<'a> {
    #[cfg(feature = "nightly")]
    pub const fn empty() -> Self {
        Self {
            ptr: NonNull::dangling(),
            offset: 0,
            len: 0,
            lt: PhantomData,
        }
    }

    #[cfg(not(feature = "nightly"))]
    pub fn empty() -> Self {
        Self {
            ptr: NonNull::dangling(),
            offset: 0,
            len: 0,
            lt: PhantomData,
        }
    }

    pub unsafe fn into_get_unchecked_mut<I: SliceIndexMut<Self>>(self, index: I) -> I::Output {
        index.get_unchecked_mut(self)
    }

    pub fn into_get_mut<I: SliceIndexMut<Self>>(self, index: I) -> Option<I::Output> {
        index.get_mut(self)
    }

    pub unsafe fn get_unchecked_mut<'b, I: SliceIndexMut<BitSliceMut<'b>>>(
        &'b mut self,
        index: I,
    ) -> I::Output {
        index.get_unchecked_mut(self.as_mut())
    }

    pub fn get_mut<'b, I: SliceIndexMut<BitSliceMut<'b>>>(
        &'b mut self,
        index: I,
    ) -> Option<I::Output> {
        index.get_mut(self.as_mut())
    }

    pub fn iter_mut(self) -> IterMut<'a> {
        self.into_iter()
    }

    pub fn split_at_mut(self, index: usize) -> Result<(Self, Self), Self> {
        if index <= self.len {
            unsafe { Ok(self.split_at_mut_unchecked(index)) }
        } else {
            Err(self)
        }
    }

    pub fn split_first_mut(self) -> Result<(BitProxy<'a>, Self), Self> {
        let (first, rest) = self.split_at_mut(1)?;

        let first = unsafe { first.into_get_unchecked_mut(0) };

        Ok((first, rest))
    }

    pub fn split_last_mut(self) -> Result<(Self, BitProxy<'a>), Self> {
        let len = match self.len.checked_sub(1) {
            Some(len) => len,
            None => return Err(self),
        };

        unsafe {
            let (rest, last) = self.split_at_mut_unchecked(len);

            let last = last.into_get_unchecked_mut(0);

            Ok((rest, last))
        }
    }

    pub unsafe fn split_at_mut_unchecked(mut self, index: usize) -> (Self, Self) {
        let BitSliceMut {
            ptr, len, offset, ..
        } = self.get_unchecked_mut(index..);

        let right = BitSliceMut {
            ptr,
            len,
            offset,
            lt: PhantomData,
        };

        (BitSliceMut { len: index, ..self }, right)
    }

    pub fn set(&mut self, index: usize, value: bool) {
        assert!(index < self.len, "Index is out of bounds!");

        let (slot, offset) = index_to_slot(index + self.offset as usize);

        let slot = unsafe { &mut *self.ptr.as_ptr().add(slot) };

        set_bit(slot, offset, value);
    }

    pub fn set_all(&mut self, value: bool) {
        let block_value = if value { !0 } else { 0 };

        let (blocks, last) = index_to_slot(self.offset as usize + self.len);
        let ptr = self.ptr.as_ptr();

        let (ptr, blocks) = if self.offset == 0 {
            (ptr, blocks)
        } else {
            unsafe {
                // first byte
                for i in self.offset..8 {
                    set_bit(&mut *ptr, i, value);
                }

                (ptr.add(1), blocks - 1)
            }
        };

        unsafe {
            // last byte
            let ptr = ptr.add(blocks);

            for i in 0..last {
                set_bit(&mut *ptr, i, value);
            }
        }

        unsafe {
            // middle bytes
            std::ptr::write_bytes(ptr, block_value, blocks);
        }
    }

    pub fn as_mut(&mut self) -> BitSliceMut<'_> {
        BitSliceMut {
            ptr: self.ptr,
            offset: self.offset,
            len: self.len,
            lt: PhantomData,
        }
    }
}

impl<'a> Deref for BitSliceMut<'a> {
    type Target = BitSlice<'a>;

    fn deref(&self) -> &Self::Target {
        unsafe { &*(self as *const Self as *const BitSlice<'a>) }
    }
}

impl Seal<BitSlice<'_>> for usize {}

impl<'a> SliceIndex<BitSlice<'a>> for usize {
    type Output = bool;

    fn check(&self, slice: &BitSlice<'a>) -> bool {
        *self < slice.len
    }

    unsafe fn get_unchecked(self, slice: BitSlice<'_>) -> Self::Output {
        let (slot, offset) = index_to_slot(self + slice.offset as usize);

        let slot = *slice.ptr.as_ptr().add(slot);

        get_bit(slot, offset)
    }
}

impl Seal<BitSliceMut<'_>> for usize {}

impl<'a> SliceIndexMut<BitSliceMut<'a>> for usize {
    type Output = BitProxy<'a>;

    fn check_mut(&self, slice: &BitSliceMut) -> bool {
        *self < slice.len
    }

    unsafe fn get_unchecked_mut(self, slice: BitSliceMut<'a>) -> Self::Output {
        let (slot, offset) = index_to_slot(self + slice.offset as usize);

        let slot = slice.ptr.as_ptr().add(slot);

        let value = get_bit(*slot, offset);

        BitProxy {
            slot: Cell::from_mut(&mut *slot),
            offset,
            value,
        }
    }
}

use std::ops::{Range, RangeFrom, RangeFull, RangeInclusive, RangeTo, RangeToInclusive};

impl Seal<BitSlice<'_>> for RangeFull {}

impl<'a> SliceIndex<BitSlice<'a>> for RangeFull {
    type Output = BitSlice<'a>;

    fn check(&self, _: &BitSlice) -> bool {
        true
    }

    unsafe fn get_unchecked(self, slice: BitSlice<'a>) -> Self::Output {
        slice
    }
}

impl Seal<BitSliceMut<'_>> for RangeFull {}

impl<'a> SliceIndexMut<BitSliceMut<'a>> for RangeFull {
    type Output = BitSliceMut<'a>;

    fn check_mut(&self, _: &BitSliceMut) -> bool {
        true
    }

    unsafe fn get_unchecked_mut(self, slice: BitSliceMut<'a>) -> Self::Output {
        slice
    }
}

impl Seal<BitSlice<'_>> for RangeTo<usize> {}

impl<'a> SliceIndex<BitSlice<'a>> for RangeTo<usize> {
    type Output = BitSlice<'a>;

    fn check(&self, slice: &BitSlice) -> bool {
        self.end <= slice.len
    }

    unsafe fn get_unchecked(self, slice: BitSlice<'a>) -> Self::Output {
        debug_assert!(self.end <= slice.len);

        BitSlice {
            len: self.end,
            ..slice
        }
    }
}

impl Seal<BitSliceMut<'_>> for RangeTo<usize> {}

impl<'a> SliceIndexMut<BitSliceMut<'a>> for RangeTo<usize> {
    type Output = BitSliceMut<'a>;

    fn check_mut(&self, slice: &BitSliceMut) -> bool {
        self.end <= slice.len
    }

    unsafe fn get_unchecked_mut(self, slice: BitSliceMut<'a>) -> Self::Output {
        debug_assert!(self.end <= slice.len);

        BitSliceMut {
            len: self.end,
            ..slice
        }
    }
}

impl Seal<BitSlice<'_>> for RangeToInclusive<usize> {}

impl<'a> SliceIndex<BitSlice<'a>> for RangeToInclusive<usize> {
    type Output = BitSlice<'a>;

    fn check(&self, slice: &BitSlice) -> bool {
        self.end < slice.len
    }

    unsafe fn get_unchecked(self, slice: BitSlice<'a>) -> Self::Output {
        debug_assert!(self.end < slice.len);

        BitSlice {
            len: self.end + 1,
            ..slice
        }
    }
}

impl Seal<BitSliceMut<'_>> for RangeToInclusive<usize> {}

impl<'a> SliceIndexMut<BitSliceMut<'a>> for RangeToInclusive<usize> {
    type Output = BitSliceMut<'a>;

    fn check_mut(&self, slice: &BitSliceMut) -> bool {
        self.end < slice.len
    }

    unsafe fn get_unchecked_mut(self, slice: BitSliceMut<'a>) -> Self::Output {
        debug_assert!(self.end < slice.len);

        BitSliceMut {
            len: self.end + 1,
            ..slice
        }
    }
}

impl Seal<BitSlice<'_>> for RangeFrom<usize> {}

impl<'a> SliceIndex<BitSlice<'a>> for RangeFrom<usize> {
    type Output = BitSlice<'a>;

    fn check(&self, slice: &BitSlice) -> bool {
        self.start <= slice.len
    }

    unsafe fn get_unchecked(self, slice: BitSlice<'a>) -> Self::Output {
        debug_assert!(self.start <= slice.len);

        let index = slice.offset as usize + self.start;

        let (slot, offset) = index_to_slot(index);

        BitSlice {
            ptr: NonNull::new_unchecked(slice.ptr.as_ptr().add(slot)),
            len: slice.len - self.start,
            offset,
            lt: PhantomData,
        }
    }
}

impl Seal<BitSliceMut<'_>> for RangeFrom<usize> {}

impl<'a> SliceIndexMut<BitSliceMut<'a>> for RangeFrom<usize> {
    type Output = BitSliceMut<'a>;

    fn check_mut(&self, slice: &BitSliceMut) -> bool {
        self.start <= slice.len
    }

    unsafe fn get_unchecked_mut(self, slice: BitSliceMut<'a>) -> Self::Output {
        debug_assert!(self.start <= slice.len);

        let index = slice.offset as usize + self.start;

        let (slot, offset) = index_to_slot(index);

        BitSliceMut {
            ptr: NonNull::new_unchecked(slice.ptr.as_ptr().add(slot)),
            len: slice.len - self.start,
            offset,
            lt: PhantomData,
        }
    }
}

impl Seal<BitSlice<'_>> for Range<usize> {}

impl<'a> SliceIndex<BitSlice<'a>> for Range<usize> {
    type Output = BitSlice<'a>;

    fn check(&self, slice: &BitSlice) -> bool {
        self.start < self.end && self.end <= slice.len
    }

    unsafe fn get_unchecked(self, slice: BitSlice<'a>) -> Self::Output {
        slice.get_unchecked(..self.end).get_unchecked(self.start..)
    }
}

impl Seal<BitSliceMut<'_>> for Range<usize> {}

impl<'a> SliceIndexMut<BitSliceMut<'a>> for Range<usize> {
    type Output = BitSliceMut<'a>;

    fn check_mut(&self, slice: &BitSliceMut) -> bool {
        self.start < self.end && self.end <= slice.len
    }

    unsafe fn get_unchecked_mut(self, slice: BitSliceMut<'a>) -> Self::Output {
        slice
            .into_get_unchecked_mut(..self.end)
            .into_get_unchecked_mut(self.start..)
    }
}

impl Seal<BitSlice<'_>> for RangeInclusive<usize> {}

impl<'a> SliceIndex<BitSlice<'a>> for RangeInclusive<usize> {
    type Output = BitSlice<'a>;

    fn check(&self, slice: &BitSlice) -> bool {
        self.start() <= self.end() && *self.end() <= slice.len
    }

    unsafe fn get_unchecked(self, slice: BitSlice<'a>) -> Self::Output {
        slice
            .get_unchecked(..=*self.end())
            .get_unchecked(*self.start()..)
    }
}

impl Seal<BitSliceMut<'_>> for RangeInclusive<usize> {}

impl<'a> SliceIndexMut<BitSliceMut<'a>> for RangeInclusive<usize> {
    type Output = BitSliceMut<'a>;

    fn check_mut(&self, slice: &BitSliceMut) -> bool {
        self.start() <= self.end() && *self.end() <= slice.len
    }

    unsafe fn get_unchecked_mut(self, slice: BitSliceMut<'a>) -> Self::Output {
        slice
            .into_get_unchecked_mut(..=*self.end())
            .into_get_unchecked_mut(*self.start()..)
    }
}

#[cfg(test)]
fn from_bytes(slice: &mut [u8], range: std::ops::Range<usize>) -> BitSliceMut<'_> {
    let offset = (range.start & 0b0111) as u8;
    let len = range.end - range.start;
    let slice = &mut slice[range.start >> 3..(range.end + 7) >> 3];

    BitSliceMut {
        ptr: NonNull::from(slice).cast(),
        offset,
        len,
        lt: PhantomData,
    }
}

pub struct Iter<'a> {
    slice: BitSlice<'a>,
}

impl<'a> Iter<'a> {
    pub fn into_slice(self) -> BitSlice<'a> {
        self.slice
    }

    pub fn as_slice(&self) -> BitSlice<'_> {
        self.slice.as_ref()
    }
}

impl<'a> Iterator for Iter<'a> {
    type Item = bool;

    fn next(&mut self) -> Option<Self::Item> {
        let (next, rest) = self.slice.split_first()?;

        self.slice = rest;

        Some(next)
    }

    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        self.slice = self.slice.get(n..)?;

        self.next()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.slice.len, Some(self.slice.len))
    }
}

impl<'a> DoubleEndedIterator for Iter<'a> {
    fn next_back(&mut self) -> Option<Self::Item> {
        let (rest, next) = self.slice.split_last()?;

        self.slice = rest;

        Some(next)
    }

    #[cfg(feature = "nightly")]
    fn nth_back(&mut self, n: usize) -> Option<Self::Item> {
        let index = self.slice.len.checked_sub(n)?;

        self.slice = unsafe { self.slice.get_unchecked(..index) };

        self.next()
    }
}

impl ExactSizeIterator for Iter<'_> {}
impl std::iter::FusedIterator for Iter<'_> {}

pub struct IterMut<'a> {
    slice: BitSliceMut<'a>,
}

impl<'a> IterMut<'a> {
    pub fn into_slice(self) -> BitSlice<'a> {
        *self.slice
    }

    pub fn into_slice_mut(self) -> BitSliceMut<'a> {
        self.slice
    }

    pub fn as_slice(&self) -> BitSlice<'_> {
        self.slice.as_ref()
    }

    pub fn as_slice_mut(&mut self) -> BitSliceMut<'_> {
        self.slice.as_mut()
    }
}

impl<'a> Iterator for IterMut<'a> {
    type Item = BitProxy<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let slice = std::mem::replace(&mut self.slice, Default::default());

        let (next, rest) = slice.split_first_mut().ok()?;

        self.slice = rest;

        Some(next)
    }

    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        let slice = std::mem::replace(&mut self.slice, Default::default());

        self.slice = slice.into_get_mut(n..)?;

        self.next()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.slice.len, Some(self.slice.len))
    }
}

impl<'a> DoubleEndedIterator for IterMut<'a> {
    fn next_back(&mut self) -> Option<Self::Item> {
        let slice = std::mem::replace(&mut self.slice, Default::default());

        let (rest, next) = slice.split_last_mut().ok()?;

        self.slice = rest;

        Some(next)
    }

    #[cfg(feature = "nightly")]
    fn nth_back(&mut self, n: usize) -> Option<Self::Item> {
        let slice = std::mem::replace(&mut self.slice, Default::default());

        let index = slice.len.checked_sub(n)?;

        self.slice = unsafe { slice.into_get_unchecked_mut(..index) };

        self.next_back()
    }
}

impl ExactSizeIterator for IterMut<'_> {}
impl std::iter::FusedIterator for IterMut<'_> {}

impl<'a> IntoIterator for BitSlice<'a> {
    type Item = bool;
    type IntoIter = Iter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        Iter { slice: self }
    }
}

impl<'a> IntoIterator for BitSliceMut<'a> {
    type Item = BitProxy<'a>;
    type IntoIter = IterMut<'a>;

    fn into_iter(self) -> Self::IntoIter {
        IterMut { slice: self }
    }
}

#[test]
fn slice() {
    let mut a = [0u8; 16];

    from_bytes(&mut a, 4..28).set_all(true);

    from_bytes(&mut a, 32..64).set_all(true);

    from_bytes(&mut a, 64..92).set_all(true);

    from_bytes(&mut a, 100..128).set_all(true);

    assert_eq!(
        a,
        [240, 255, 255, 15, 255, 255, 255, 255, 255, 255, 255, 15, 240, 255, 255, 255]
    );
}
