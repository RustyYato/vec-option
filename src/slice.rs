use crate::bit_vec::slice::{BitSlice, BitSliceMut};
use crate::{OptionProxy, VecOption};

use std::ops::Deref;

use std::marker::PhantomData;
use std::mem::MaybeUninit;
use std::ptr::NonNull;

#[repr(C)]
pub struct Slice<'a, T: 'a> {
    data: NonNull<T>,
    flag: BitSlice<'a>,
    lt: PhantomData<&'a [T]>,
}

#[repr(C)]
pub struct SliceMut<'a, T: 'a> {
    data: NonNull<T>,
    flag: BitSliceMut<'a>,
    lt: PhantomData<&'a mut [T]>,
}

unsafe impl<T: Send + Sync> Send for Slice<'_, T> {}
unsafe impl<T: Send + Sync> Sync for Slice<'_, T> {}

unsafe impl<T: Send> Send for SliceMut<'_, T> {}
unsafe impl<T: Send + Sync> Sync for SliceMut<'_, T> {}

impl<T> Copy for Slice<'_, T> {}
impl<T> Clone for Slice<'_, T> {
    fn clone(&self) -> Self {
        *self
    }
}

use std::fmt;

impl<T: fmt::Debug> fmt::Debug for Slice<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

impl<T: fmt::Debug> fmt::Debug for SliceMut<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

impl<'a, T> Deref for SliceMut<'a, T> {
    type Target = Slice<'a, T>;

    fn deref(&self) -> &Self::Target {
        unsafe { &*(self as *const _ as *const _) }
    }
}

impl<T> Default for Slice<'_, T> {
    fn default() -> Self {
        Self::empty()
    }
}

impl<T> Default for SliceMut<'_, T> {
    fn default() -> Self {
        Self::empty()
    }
}

impl<T> VecOption<T> {
    pub fn as_slice(&self) -> Slice<'_, T> {
        Slice {
            data: NonNull::from(&*self.data).cast(),
            flag: self.flag.as_slice(),
            lt: PhantomData,
        }
    }

    pub fn as_mut_slice(&mut self) -> SliceMut<'_, T> {
        SliceMut {
            data: NonNull::from(&mut *self.data).cast(),
            flag: self.flag.as_mut_slice(),
            lt: PhantomData,
        }
    }
}

impl<'a, T> SliceMut<'a, T> {
    pub fn into_slice(self) -> Slice<'a, T> {
        *self
    }
}

impl<'a, T> Slice<'a, T> {
    pub const fn empty() -> Self {
        Self {
            data: NonNull::dangling(),
            flag: BitSlice::empty(),
            lt: PhantomData,
        }
    }

    pub fn as_ref(&self) -> Slice<'_, T> {
        *self
    }

    pub const fn len(self) -> usize {
        self.flag.len()
    }

    pub const fn is_empty(self) -> bool {
        self.flag.is_empty()
    }

    pub unsafe fn get_unchecked<I: SliceIndex<Self>>(self, index: I) -> I::Output {
        index.get_unchecked(self)
    }

    pub fn get<I: SliceIndex<Self>>(self, index: I) -> Option<I::Output> {
        index.get(self)
    }

    pub fn iter(self) -> Iter<'a, T> {
        self.into_iter()
    }

    pub fn split_at(self, index: usize) -> Option<(Self, Self)> {
        if index <= self.len() {
            unsafe { Some(self.split_at_unchecked(index)) }
        } else {
            None
        }
    }

    pub fn split_first(self) -> Option<(Option<&'a T>, Self)> {
        let (first, rest) = self.split_at(1)?;

        unsafe { Some((first.get_unchecked(0), rest)) }
    }

    pub fn split_last(self) -> Option<(Self, Option<&'a T>)> {
        let len = self.len().checked_sub(1)?;

        unsafe {
            let (rest, last) = self.split_at_unchecked(len);

            Some((rest, last.get_unchecked(0)))
        }
    }

    pub unsafe fn split_at_unchecked(self, index: usize) -> (Self, Self) {
        let data = NonNull::new_unchecked(self.data.as_ptr().add(index));

        let (left, right) = self.flag.split_at_unchecked(index);

        (
            Self {
                data: self.data,
                flag: left,
                lt: PhantomData,
            },
            Self {
                data,
                flag: right,
                lt: PhantomData,
            },
        )
    }
}

impl<'a, T> SliceMut<'a, T> {
    #[cfg(feature = "nightly")]
    pub const fn empty() -> Self {
        Self {
            data: NonNull::dangling(),
            flag: BitSliceMut::empty(),
            lt: PhantomData,
        }
    }

    #[cfg(not(feature = "nightly"))]
    pub fn empty() -> Self {
        Self {
            data: NonNull::dangling(),
            flag: BitSliceMut::empty(),
            lt: PhantomData,
        }
    }

    pub fn as_mut(&mut self) -> SliceMut<'_, T> {
        SliceMut {
            data: self.data,
            flag: self.flag.as_mut(),
            lt: PhantomData,
        }
    }

    pub unsafe fn into_get_unchecked_mut<I: SliceIndexMut<Self>>(self, index: I) -> I::Output {
        index.get_unchecked_mut(self)
    }

    pub fn into_get_mut<I: SliceIndexMut<Self>>(self, index: I) -> Option<I::Output> {
        index.get_mut(self)
    }

    pub unsafe fn get_unchecked_mut<'b, I: SliceIndexMut<SliceMut<'b, T>>>(
        &'b mut self,
        index: I,
    ) -> I::Output {
        index.get_unchecked_mut(self.as_mut())
    }

    pub fn get_mut<'b, I: SliceIndexMut<SliceMut<'b, T>>>(
        &'b mut self,
        index: I,
    ) -> Option<I::Output> {
        index.get_mut(self.as_mut())
    }

    pub fn iter_mut(self) -> IterMut<'a, T> {
        self.into_iter()
    }

    pub fn split_at_mut(self, index: usize) -> Result<(Self, Self), Self> {
        if index <= self.len() {
            unsafe { Ok(self.split_at_mut_unchecked(index)) }
        } else {
            Err(self)
        }
    }

    pub fn split_first_mut(self) -> Result<(OptionProxy<'a, T>, Self), Self> {
        let (first, rest) = self.split_at_mut(1)?;

        unsafe { Ok((first.into_get_unchecked_mut(0), rest)) }
    }

    pub fn split_last_mut(self) -> Result<(Self, OptionProxy<'a, T>), Self> {
        let len = match self.len().checked_sub(1) {
            Some(len) => len,
            None => return Err(self),
        };

        unsafe {
            let (rest, last) = self.split_at_mut_unchecked(len);

            Ok((rest, last.into_get_unchecked_mut(0)))
        }
    }

    pub unsafe fn split_at_mut_unchecked(self, index: usize) -> (Self, Self) {
        let data = NonNull::new_unchecked(self.data.as_ptr().add(index));

        let (left, right) = self.flag.split_at_mut_unchecked(index);

        (
            Self {
                data: self.data,
                flag: left,
                lt: PhantomData,
            },
            Self {
                data,
                flag: right,
                lt: PhantomData,
            },
        )
    }

    /// Returns the element at `index` or None if out of bounds.
    ///
    /// Replaces the element at `index` with None.
    pub fn take(&mut self, index: usize) -> Option<Option<T>> {
        self.replace(index, None)
    }

    /// Replace the element at `index` with `value`
    pub fn replace<O: Into<Option<T>>>(&mut self, index: usize, value: O) -> Option<Option<T>> {
        unsafe {
            let value = value.into();

            let flag = self.flag.get(index)?;

            // index was checked by flag.get
            let data = self.data.as_ptr().add(index);

            let out = if flag {
                // flag corrosponds to data
                Some(data.read())
            } else {
                None
            };

            match value {
                Some(value) => {
                    self.flag.set(index, true);

                    // data is valid, use write to prevent
                    // dropping uninitialized memory
                    data.write(value);
                }
                None => self.flag.set(index, false),
            }

            Some(out)
        }
    }

    // pub fn set(&mut self, index: usize, value: bool) {
    //     assert!(index < self.len, "Index is out of bounds!");

    //     let (slot, offset) = index_to_slot(index + self.offset as usize);

    //     let slot = unsafe { &mut *self.ptr.as_ptr().add(slot) };

    //     set_bit(slot, offset, value);
    // }

    // pub fn set_all(&mut self, value: bool) {
    //     let block_value = if value { !0 } else { 0 };

    //     let (blocks, last) = index_to_slot(self.offset as usize + self.len);
    //     let ptr = self.ptr.as_ptr();

    //     let (ptr, blocks) = if self.offset == 0 {
    //         (ptr, blocks)
    //     } else {
    //         unsafe {
    //             // first byte
    //             for i in self.offset..8 {
    //                 set_bit(&mut *ptr, i, value);
    //             }

    //             (ptr.add(1), blocks - 1)
    //         }
    //     };

    //     unsafe {
    //         // last byte
    //         let ptr = ptr.add(blocks);

    //         for i in 0..last {
    //             set_bit(&mut *ptr, i, value);
    //         }
    //     }

    //     unsafe {
    //         // middle bytes
    //         std::ptr::write_bytes(ptr, block_value, blocks);
    //     }
    // }
}

pub(crate) use seal::Seal;
pub(crate) mod seal {
    pub trait Seal<S>: Sized {}
}

pub trait SliceIndex<S>: Seal<S> {
    type Output;

    fn check(&self, slice: &S) -> bool;

    unsafe fn get_unchecked(self, slice: S) -> Self::Output;

    fn get(self, slice: S) -> Option<Self::Output> {
        if self.check(&slice) {
            unsafe { Some(self.get_unchecked(slice)) }
        } else {
            None
        }
    }
}

pub trait SliceIndexMut<S>: Seal<S> {
    type Output;

    fn check_mut(&self, slice: &S) -> bool;

    unsafe fn get_unchecked_mut(self, slice: S) -> Self::Output;

    fn get_mut(self, slice: S) -> Option<Self::Output> {
        if self.check_mut(&slice) {
            unsafe { Some(self.get_unchecked_mut(slice)) }
        } else {
            None
        }
    }
}

impl<T> Seal<Slice<'_, T>> for usize {}

impl<'a, T> SliceIndex<Slice<'a, T>> for usize {
    type Output = Option<&'a T>;

    fn check(&self, slice: &Slice<'a, T>) -> bool {
        self.check(&slice.flag)
    }

    unsafe fn get_unchecked(self, slice: Slice<'a, T>) -> Self::Output {
        let flag = self.get_unchecked(slice.flag);

        if flag {
            Some(&*slice.data.as_ptr().add(self))
        } else {
            None
        }
    }
}

impl<T> Seal<SliceMut<'_, T>> for usize {}

impl<'a, T> SliceIndexMut<SliceMut<'a, T>> for usize {
    type Output = OptionProxy<'a, T>;

    fn check_mut(&self, slice: &SliceMut<'a, T>) -> bool {
        self.check_mut(&slice.flag)
    }

    unsafe fn get_unchecked_mut(self, slice: SliceMut<'a, T>) -> Self::Output {
        let flag = self.get_unchecked_mut(slice.flag);
        let data = &mut *slice.data.cast::<MaybeUninit<T>>().as_ptr().add(self);

        OptionProxy::new(flag, data)
    }
}

use std::ops::{Range, RangeFrom, RangeFull, RangeInclusive, RangeTo, RangeToInclusive};

impl<T> Seal<Slice<'_, T>> for RangeFull {}

impl<'a, T> SliceIndex<Slice<'a, T>> for RangeFull {
    type Output = Slice<'a, T>;

    fn check(&self, _: &Slice<'a, T>) -> bool {
        true
    }

    unsafe fn get_unchecked(self, slice: Slice<'a, T>) -> Self::Output {
        slice
    }
}

impl<T> Seal<SliceMut<'_, T>> for RangeFull {}

impl<'a, T> SliceIndexMut<SliceMut<'a, T>> for RangeFull {
    type Output = SliceMut<'a, T>;

    fn check_mut(&self, _: &SliceMut<'a, T>) -> bool {
        true
    }

    unsafe fn get_unchecked_mut(self, slice: SliceMut<'a, T>) -> Self::Output {
        slice
    }
}

impl<T> Seal<Slice<'_, T>> for RangeTo<usize> {}

impl<'a, T> SliceIndex<Slice<'a, T>> for RangeTo<usize> {
    type Output = Slice<'a, T>;

    fn check(&self, slice: &Slice<'a, T>) -> bool {
        self.check(&slice.flag)
    }

    unsafe fn get_unchecked(self, slice: Slice<'a, T>) -> Self::Output {
        Slice {
            flag: self.get_unchecked(slice.flag),
            ..slice
        }
    }
}

impl<T> Seal<SliceMut<'_, T>> for RangeTo<usize> {}

impl<'a, T> SliceIndexMut<SliceMut<'a, T>> for RangeTo<usize> {
    type Output = SliceMut<'a, T>;

    fn check_mut(&self, slice: &SliceMut<'a, T>) -> bool {
        self.check_mut(&slice.flag)
    }

    unsafe fn get_unchecked_mut(self, slice: SliceMut<'a, T>) -> Self::Output {
        SliceMut {
            flag: self.get_unchecked_mut(slice.flag),
            ..slice
        }
    }
}

impl<T> Seal<Slice<'_, T>> for RangeToInclusive<usize> {}

impl<'a, T> SliceIndex<Slice<'a, T>> for RangeToInclusive<usize> {
    type Output = Slice<'a, T>;

    fn check(&self, slice: &Slice<'a, T>) -> bool {
        self.check(&slice.flag)
    }

    unsafe fn get_unchecked(self, slice: Slice<'a, T>) -> Self::Output {
        Slice {
            flag: self.get_unchecked(slice.flag),
            ..slice
        }
    }
}

impl<T> Seal<SliceMut<'_, T>> for RangeToInclusive<usize> {}

impl<'a, T> SliceIndexMut<SliceMut<'a, T>> for RangeToInclusive<usize> {
    type Output = SliceMut<'a, T>;

    fn check_mut(&self, slice: &SliceMut<'a, T>) -> bool {
        self.check_mut(&slice.flag)
    }

    unsafe fn get_unchecked_mut(self, slice: SliceMut<'a, T>) -> Self::Output {
        SliceMut {
            flag: self.get_unchecked_mut(slice.flag),
            ..slice
        }
    }
}

impl<T> Seal<Slice<'_, T>> for RangeFrom<usize> {}

impl<'a, T> SliceIndex<Slice<'a, T>> for RangeFrom<usize> {
    type Output = Slice<'a, T>;

    fn check(&self, slice: &Slice<'a, T>) -> bool {
        self.check(&slice.flag)
    }

    unsafe fn get_unchecked(self, slice: Slice<'a, T>) -> Self::Output {
        Slice {
            data: NonNull::new_unchecked(slice.data.as_ptr().add(self.start)),
            flag: self.get_unchecked(slice.flag),
            lt: PhantomData,
        }
    }
}

impl<T> Seal<SliceMut<'_, T>> for RangeFrom<usize> {}

impl<'a, T> SliceIndexMut<SliceMut<'a, T>> for RangeFrom<usize> {
    type Output = SliceMut<'a, T>;

    fn check_mut(&self, slice: &SliceMut<'a, T>) -> bool {
        self.check_mut(&slice.flag)
    }

    unsafe fn get_unchecked_mut(self, slice: SliceMut<'a, T>) -> Self::Output {
        SliceMut {
            data: NonNull::new_unchecked(slice.data.as_ptr().add(self.start)),
            flag: self.get_unchecked_mut(slice.flag),
            lt: PhantomData,
        }
    }
}

impl<T> Seal<Slice<'_, T>> for Range<usize> {}

impl<'a, T> SliceIndex<Slice<'a, T>> for Range<usize> {
    type Output = Slice<'a, T>;

    fn check(&self, slice: &Slice<'a, T>) -> bool {
        self.check(&slice.flag)
    }

    unsafe fn get_unchecked(self, slice: Slice<'a, T>) -> Self::Output {
        slice.get_unchecked(..self.end).get_unchecked(self.start..)
    }
}

impl<T> Seal<SliceMut<'_, T>> for Range<usize> {}

impl<'a, T> SliceIndexMut<SliceMut<'a, T>> for Range<usize> {
    type Output = SliceMut<'a, T>;

    fn check_mut(&self, slice: &SliceMut<'a, T>) -> bool {
        self.check_mut(&slice.flag)
    }

    unsafe fn get_unchecked_mut(self, slice: SliceMut<'a, T>) -> Self::Output {
        slice
            .into_get_unchecked_mut(..self.end)
            .into_get_unchecked_mut(self.start..)
    }
}

impl<T> Seal<Slice<'_, T>> for RangeInclusive<usize> {}

impl<'a, T> SliceIndex<Slice<'a, T>> for RangeInclusive<usize> {
    type Output = Slice<'a, T>;

    fn check(&self, slice: &Slice<'a, T>) -> bool {
        self.check(&slice.flag)
    }

    unsafe fn get_unchecked(self, slice: Slice<'a, T>) -> Self::Output {
        slice
            .get_unchecked(..=*self.end())
            .get_unchecked(*self.start()..)
    }
}

impl<T> Seal<SliceMut<'_, T>> for RangeInclusive<usize> {}

impl<'a, T> SliceIndexMut<SliceMut<'a, T>> for RangeInclusive<usize> {
    type Output = SliceMut<'a, T>;

    fn check_mut(&self, slice: &SliceMut<'a, T>) -> bool {
        self.check_mut(&slice.flag)
    }

    unsafe fn get_unchecked_mut(self, slice: SliceMut<'a, T>) -> Self::Output {
        slice
            .into_get_unchecked_mut(..=*self.end())
            .into_get_unchecked_mut(*self.start()..)
    }
}

pub struct Iter<'a, T> {
    slice: Slice<'a, T>,
}

impl<'a, T> Iter<'a, T> {
    pub fn into_slice(self) -> Slice<'a, T> {
        self.slice
    }

    pub fn as_slice(&self) -> Slice<'_, T> {
        self.slice.as_ref()
    }
}

impl<'a, T> Iterator for Iter<'a, T> {
    type Item = Option<&'a T>;

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
        (self.slice.len(), Some(self.slice.len()))
    }
}

impl<'a, T> DoubleEndedIterator for Iter<'a, T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        let (rest, next) = self.slice.split_last()?;

        self.slice = rest;

        Some(next)
    }

    #[cfg(feature = "nightly")]
    fn nth_back(&mut self, n: usize) -> Option<Self::Item> {
        let index = self.slice.len().checked_sub(n)?;

        self.slice = unsafe { self.slice.get_unchecked(..index) };

        self.next()
    }
}

impl<T> ExactSizeIterator for Iter<'_, T> {}
impl<T> std::iter::FusedIterator for Iter<'_, T> {}

pub struct IterMut<'a, T> {
    slice: SliceMut<'a, T>,
}

impl<'a, T> IterMut<'a, T> {
    pub fn into_slice(self) -> Slice<'a, T> {
        *self.slice
    }

    pub fn into_slice_mut(self) -> SliceMut<'a, T> {
        self.slice
    }

    pub fn as_slice(&self) -> Slice<'_, T> {
        self.slice.as_ref()
    }

    pub fn as_slice_mut(&mut self) -> SliceMut<'_, T> {
        self.slice.as_mut()
    }
}

impl<'a, T> Iterator for IterMut<'a, T> {
    type Item = OptionProxy<'a, T>;

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
        (self.slice.len(), Some(self.slice.len()))
    }
}

impl<'a, T> DoubleEndedIterator for IterMut<'a, T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        let slice = std::mem::replace(&mut self.slice, Default::default());

        let (rest, next) = slice.split_last_mut().ok()?;

        self.slice = rest;

        Some(next)
    }

    #[cfg(feature = "nightly")]
    fn nth_back(&mut self, n: usize) -> Option<Self::Item> {
        let slice = std::mem::replace(&mut self.slice, Default::default());

        let index = slice.len().checked_sub(n)?;

        self.slice = unsafe { slice.into_get_unchecked_mut(..index) };

        self.next_back()
    }
}

impl<T> ExactSizeIterator for IterMut<'_, T> {}
impl<T> std::iter::FusedIterator for IterMut<'_, T> {}

impl<'a, T> IntoIterator for Slice<'a, T> {
    type Item = Option<&'a T>;
    type IntoIter = Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        Iter { slice: self }
    }
}

impl<'a, T> IntoIterator for SliceMut<'a, T> {
    type Item = OptionProxy<'a, T>;
    type IntoIter = IterMut<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        IterMut { slice: self }
    }
}
