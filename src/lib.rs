#![cfg_attr(feature = "nightly", feature(specialization, try_trait))]
#![allow(clippy::option_option)]
#![forbid(missing_docs)]

/*!
# vec-option

A space optimized version of `Vec<Option<T>>` that stores the discriminant seperately.

## Feature flags

`nightly` - This turns on a few optimizations (makes `Clone`ing `Copy` elements much cheaper) and extends `try_fold` and `try_for_each` to work with all `Try` types.

## Pros

* Can have a smaller memory footprint compared to `Vec<Option<T>>` if `Option<T>`'s space optimizations don't take effect
* More cache-friendly if `Option<T>`'s space optimizations don't take effect
* Quickly set the entire collection to contain `None`
* Fast extend with `None`

## Cons

* 2 allocations, instead of a single allocation
* Cannot remove elements from the middle of the vector
* Cannot work on the option's directly

## Example

Just like a normal vector, you can push and pop elements from the end of the vector

```rust
let mut vec = VecOption::new();

vec.push(10);

assert_eq!(vec, [Some(10)]);

vec.push(20);
vec.push(None);
vec.push(Some(30));

assert_eq!(vec, [Some(10), Some(20), None, Some(30)]);

assert_eq!(vec.pop(), Some(Some(30)));
assert_eq!(vec.pop(), Some(None));
assert_eq!(vec.pop(), Some(Some(20)));
assert_eq!(vec.pop(), Some(Some(10)));
assert_eq!(vec.pop(), None);
assert_eq!(vec, []);
```

You can get elements from the vector

```rust
let mut vec = VecOption::from(vec![0, 1, 2, 3, 4]);

assert_eq!(vec.get(2), Some(Some(&2)));
assert_eq!(vec.get_mut(4), Some(Some(&mut 4)));
assert_eq!(vec.get(5), None);
```

You can swap and replace elements

```rust
vec.swap(2, 1);

assert_eq!(vec, [Some(0), Some(2), Some(1), Some(3), Some(4)]);

assert_eq!(vec.replace(3, None), Some(Some(3)));
assert_eq!(vec.replace(1, Some(10)), Some(Some(1)));

assert_eq!(vec, [Some(0), Some(10), Some(1), None, Some(4)]);
```

or if `vec.replace(index, None)` is too much, you can do

```rust
assert_eq!(vec.take(1), Some(Some(10)));

assert_eq!(vec, [Some(0), None, Some(1), None, Some(4)]);
```

Of course, you can also truncate or clear the vector

```rust
let mut vec = VecOption::from(vec![0, 1, 3, 4]);

assert_eq!(vec.len(), 4);

vec.truncate(2);

assert_eq!(vec, [0, 1]);

vec.clear();

assert!(vec.is_empty());
```

But due to the limitations imposed by spliting the representation of the vector, you can't really get a
`&Option<T>`/`&mut Option<T>` outside of a closure.
In fact, you can't get an `&Option<T>` at all, it would be fairly useless, as the only thing you can really do with it is convert it to a `Option<&T>`. But `&mut Option<T>` is usefull, so there are a handful of functions that allow you to operate with them.

```rust
// This one allows you to edit a single value however you want, and the updates will
// be reflected once the closure returns. If the closure panics, then it is as if you took the
// option out of the vector.
vec.with_mut(index, |element: &mut Option<T>| {
    ...
});
```

These functions below are like the corrosponding functions in `Iterator`, they iterate over the vector and allow you to do stuff based on which one you call. The only difference is that you get to operate on `&mut Option<T>` directly. Again, if the closure panics, it will be as if you took the value out of the vector.

```rust
vec.try_fold(...);

vec.fold(...);

vec.try_for_each(...);

vec.for_each(...);
```

But because of these limitations, you can very quickly fill up your vector with `None` and set all of the elements in your vector to `None`! This can compile down to just a `memset` if your types don't have drop glue!

```rust
let mut vec = VecOption::from(vec![0, 1, 2, 3, 4]);

assert_eq!(vec, [Some(0), Some(2), Some(1), Some(3), Some(4)]);

vec.extend_none(5);

assert_eq!(vec, [Some(0), Some(2), Some(1), Some(3), Some(4), None, None, None, None, None]);

vec.set_all_none();

assert_eq!(vec, [None, None, None, None, None, None, None, None, None, None]);
```
*/


use bit_vec::BitVec;

use std::mem::MaybeUninit;

/// # Safety
///
/// This code must never be run
#[cold]
unsafe fn unreachable_unchecked() -> ! {
    use std::hint::unreachable_unchecked;

    debug_assert!(false, "unreachable");
    unreachable_unchecked()
}

trait UnwrapUnchecked {
    type Output;

    /// # Safety
    ///
    /// The Option<T> must be in the `Some` variant
    unsafe fn unwrap_unchecked(self) -> Self::Output;
}

impl<T> UnwrapUnchecked for Option<T> {
    type Output = T;

    unsafe fn unwrap_unchecked(self) -> Self::Output {
        match self {
            Some(value) => value,
            None => unreachable_unchecked(),
        }
    }
}
trait GetUnchecked {
    /// # Safety
    ///
    /// `i` must be in bounds
    unsafe fn get_unchecked(&self, i: usize) -> bool;
}

impl GetUnchecked for BitVec {
    unsafe fn get_unchecked(&self, i: usize) -> bool {
        self.get(i).unwrap_unchecked()
    }
}

/// # Safety
///
/// The flag must corrospond to the data
///
/// i.e. if flag is true, then data must be initialized
unsafe fn from_raw_parts<T>(flag: bool, data: MaybeUninit<T>) -> Option<T> {
    if flag {
        Some(data.assume_init())
    } else {
        None
    }
}

/// # Safety
///
/// The flag must corrospond to the data
///
/// i.e. if flag is true, then data must be initialized
unsafe fn ref_mut_from_raw_parts<T>(flag: bool, data: &mut MaybeUninit<T>) -> Option<&mut T> {
    if flag {
        Some(&mut *data.as_mut_ptr())
    } else {
        None
    }
}

/// # Safety
///
/// The flag must corrospond to the data
///
/// i.e. if flag is true, then data must be initialized
unsafe fn ref_from_raw_parts<T>(flag: bool, data: &MaybeUninit<T>) -> Option<&T> {
    if flag {
        Some(&*data.as_ptr())
    } else {
        None
    }
}

/// A space optimized version of `Vec<Option<T>>` that stores the discriminant seperately
/// 
/// See crate-level docs for more information
/// 
pub struct VecOption<T> {
    data: Vec<MaybeUninit<T>>,
    flag: BitVec,
}

/// The capacity information of the given `VecOption<T>`
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CapacityInfo {
    /// The capacity of the data vector that holds `T`s
    pub data: usize,

    /// The capacity of the `BitVec` that holds the discriminants
    pub flag: usize,

    _priv: ()
}

impl<T> Default for VecOption<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> VecOption<T> {
    /// Creates an empty vector, does not allocate
    pub fn new() -> Self {
        Self {
            data: Vec::new(),
            flag: BitVec::new(),
        }
    }

    /// Creates an empty vector
    /// 
    /// allocates at least `cap` elements of space
    pub fn with_capacity(cap: usize) -> Self {
        Self {
            data: Vec::with_capacity(cap),
            flag: BitVec::with_capacity(cap),
        }
    }

    /// reserves at least `amount` elements
    /// 
    /// if there is already enough space, this does nothing
    pub fn reserve(&mut self, amount: usize) {
        self.data.reserve(amount);
        self.flag.reserve(amount);
    }

    /// reserves exactly `amount` elements
    /// 
    /// if there is already enough space, this does nothing
    pub fn reserve_exact(&mut self, amount: usize) {
        self.data.reserve_exact(amount);
        self.flag.reserve_exact(amount);
    }

    /// The length of this vector
    pub fn len(&self) -> usize {
        self.data.len()
    }
    
    /// The capacity of the vector
    pub fn capacity(&self) -> CapacityInfo {
        CapacityInfo {
            data: self.data.capacity(),
            flag: self.flag.capacity(),
            _priv: ()
        }
    }
    
    /// Is this vector empty
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
    
    /// Put a value at the end of the vector
    /// 
    /// Reallocates if there is not enough space
    pub fn push<V: Into<Option<T>>>(&mut self, value: V) {
        let value = value.into();

        match value {
            Some(value) => {
                self.data.push(MaybeUninit::new(value));
                self.flag.push(true);
            }
            None => {
                self.data.push(MaybeUninit::uninit());
                self.flag.push(false);
            }
        }
    }

    /// Remove the last element of the vector
    /// 
    /// returns `None` if the vector is empty
    pub fn pop(&mut self) -> Option<Option<T>> {
        unsafe {
            let flag = self.flag.pop()?;

            // This is safe because flag pop does the necessary checks to make sure that
            // there are more elements
            // This relies on the fact that `flag.len() == data.len()`
            let data = self.data.pop().unwrap_unchecked();

            // The flag and data are a pair, (same index)
            Some(from_raw_parts(flag, data))
        }
    }

    /// Returns a mutable reference to the element at `index` or None if out of bounds.
    pub fn get_mut(&mut self, index: usize) -> Option<Option<&mut T>> {
        unsafe {
            let flag = self.flag.get(index)?;

            // This is safe because flag pop does the necessary checks to make sure that
            // there are more elements
            // This relies on the fact that `flag.len() == data.len()`
            let data = self.data.get_unchecked_mut(index);

            // The flag and data are a pair, (same index)
            Some(ref_mut_from_raw_parts(flag, data))
        }
    }

    /// Returns a reference to the element at `index` or None if out of bounds.
    pub fn get(&self, index: usize) -> Option<Option<&T>> {
        unsafe {
            let flag = self.flag.get(index)?;

            // This is safe because flag pop does the necessary checks to make sure that
            // there are more elements
            // This relies on the fact that `flag.len() == data.len()`
            let data = self.data.get_unchecked(index);

            // The flag and data are a pair, (same index)
            Some(ref_from_raw_parts(flag, data))
        }
    }

    /// Yields a mutable reference to the `Option` to the closure, and updates the value in the vector
    /// once the closure completes, if the closure panics, the element will be in the `None` state
    pub fn with_mut<F: FnOnce(&mut Option<T>) -> R, R>(&mut self, index: usize, f: F) -> Option<R> {
        unsafe {
            let flag = self.flag.get(index)?;
            
            // index was checked with flag
            let data_slot = self.data.get_unchecked_mut(index);

            self.flag.set(index, false);

            let data = std::mem::replace(data_slot, MaybeUninit::uninit());

            // flag and data are a pair (same index)
            let mut opt = from_raw_parts(flag, data);

            let ret = f(&mut opt);

            if let Some(value) = opt {
                // data_slot is valid and contains uninitialized memory
                // so do not drop it, but it is valid to write to
                data_slot.as_mut_ptr().write(value);
                self.flag.set(index, true);
            }

            Some(ret)
        }
    }

    /// Swaps two elements of the vector, panics if either index is out of bounds
    pub fn swap(&mut self, a: usize, b: usize) {
        self.data.swap(a, b);
        unsafe {
            // Swap did the necessary length checks to make sure that
            // `a` and `b` are in bounds
            let fa = self.flag.get_unchecked(a);
            let fb = self.flag.get_unchecked(b);

            self.flag.set(a, fb);
            self.flag.set(b, fa);
        }
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
            let data = self.data.get_unchecked_mut(index);

            let out = if flag {
                // flag corrosponds to data
                Some(data.as_ptr().read())
            } else {
                None
            };

            match value {
                Some(value) => {
                    self.flag.set(index, true);

                    // data is valid, use write to prevent
                    // dropping uninitialized memory
                    data.as_mut_ptr().write(value);
                }
                None => self.flag.set(index, false),
            }

            Some(out)
        }
    }

    /// Reduces the length of the vector to `len` and drops all excess elements
    /// 
    /// If `len` is greater than the length of the vector, nothing happens
    pub fn truncate(&mut self, len: usize) {
        if self.data.len() <= len {
            return;
        }

        if std::mem::needs_drop::<T>() {
            for (i, data) in self.data.iter_mut().enumerate().skip(len) {
                unsafe {
                    // index corrosponds to the index of a data, so it is valid
                    if self.flag.get_unchecked(i) {
                        self.flag.set(i, false);

                        // data is initialized, checked by flag
                        data.as_mut_ptr().drop_in_place()
                    }
                }
            }
        }

        // decreasing the length is always fine
        unsafe {
            self.data.set_len(len);
            self.flag.set_len(len);
        }
    }

    /// Clears the vector
    pub fn clear(&mut self) {
        self.truncate(0)
    }

    /// Sets all of the elements in the vector to `None` and drops
    /// all values in the closure
    pub fn set_all_none(&mut self) {
        if std::mem::needs_drop::<T>() {
            for (i, data) in self.data.iter_mut().enumerate() {
                unsafe {
                    if self.flag.get_unchecked(i) {
                        self.flag.set(i, false);
                        data.as_mut_ptr().drop_in_place()
                    }
                }
            }
        } else {
            self.flag.clear()
        }
    }

    /// Extends the vector with `additional` number of `None`s
    pub fn extend_none(&mut self, additional: usize) {
        self.flag.grow(additional, false);

        unsafe {
            self.reserve(additional);

            let len = self.len();

            // Because this is a Vec<MaybeUninit<T>>, we only need to
            // guarantee that we have enough space in the allocatation
            // for `set_len` to be safe, and that was done with the reserve
            self.data.set_len(len + additional);
        }
    }
    
    /// returns an iterator over references to the elements in the vector
    pub fn iter(&self) -> Iter<'_, T> {
        Iter {
            data: self.data.iter(),
            flag: self.flag.iter(),
        }
    }

    /// returns an iterator over mutable references to the elements in the vector
    pub fn iter_mut(&mut self) -> IterMut<'_, T> {
        IterMut {
            data: self.data.iter_mut(),
            flag: self.flag.iter(),
        }
    }

    /// Iterates over all of the `Option<T>`s in the vector and applies the closure to each one of them until
    /// the closure short-circuits, then iteration ends
    /// 
    /// The closure is passed the `init`, `index`, and a mutable reference to the corrosponding element of the vector
    #[cfg(feature = "nightly")]
    pub fn try_fold<A, R: std::ops::Try<Ok = A>, F: FnMut(A, usize, &mut Option<T>) -> R>(&mut self, mut init: A, mut f: F) -> R {
        for (i, data_slot) in self.data.iter_mut().enumerate() {
            // index corrosponds to the index of a data, so it is valid
            let flag = unsafe { self.flag.get_unchecked(i) };

            self.flag.set(i, false);

            let data = std::mem::replace(data_slot, MaybeUninit::uninit());

            // The flag and data are a pair, (same index)
            let mut opt = unsafe { from_raw_parts(flag, data) };

            let res = f(init, i, &mut opt);

            if let Some(value) = opt {
                *data_slot = MaybeUninit::new(value);
                self.flag.set(i, true);
            }

            init = res?;
        }

        R::from_ok(init)
    }
    
    /// Iterates over all of the `Option<T>`s in the vector and applies the closure to each one of them until
    /// the closure returns `Err(..)`, then iteration ends
    /// 
    /// The closure is passed the `init`, `index`, and a mutable reference to the corrosponding element of the vector
    /// 
    /// This is similar to `Iterator::try_fold`
    #[cfg(not(feature = "nightly"))]
    pub fn try_fold<A, B, F: FnMut(A, usize, &mut Option<T>) -> Result<A, B>>(&mut self, mut init: A, mut f: F) -> Result<A, B> {
        for (i, data_slot) in self.data.iter_mut().enumerate() {
            // index corrosponds to the index of a data, so it is valid
            let flag = unsafe { self.flag.get_unchecked(i) };

            self.flag.set(i, false);

            let data = std::mem::replace(data_slot, MaybeUninit::uninit());

            // The flag and data are a pair, (same index)
            let mut opt = unsafe { from_raw_parts(flag, data) };

            let res = f(init, i, &mut opt);

            if let Some(value) = opt {
                *data_slot = MaybeUninit::new(value);
                self.flag.set(i, true);
            }

            init = res?;
        }

        Ok(init)
    }

    /// Iterates over all of the `Option<T>`s in the vector and applies the closure to each one
    /// 
    /// The closure is passed the `init`, `index`, and a mutable reference to the corrosponding element of the vector
    /// 
    /// This is similar to `Iterator::fold`
    pub fn fold<A, F: FnMut(A, usize, &mut Option<T>) -> A>(&mut self, init: A, mut f: F) -> A {
        let ret = self.try_fold(init, move |a, i, x| Ok::<_, std::convert::Infallible>(f(a, i, x)));

        match ret {
            Ok(x) => x,
            Err(x) => match x {}
        }
    }
    
    /// Iterates over all of the `Option<T>`s in the vector and applies the closure to each one of them until
    /// the closure short-circuits, then iteration ends
    /// 
    /// The closure is passed the `index`, and a mutable reference to the corrosponding element of the vector
    /// 
    /// This is similar to `Iterator::try_for_each`
    #[cfg(feature = "nightly")]
    pub fn try_for_each<R: std::ops::Try<Ok = ()>, F: FnMut(usize, &mut Option<T>) -> R>(&mut self, mut f: F) -> R {
        self.try_fold((), move |(), i, x| f(i, x))
    }

    /// Iterates over all of the `Option<T>`s in the vector and applies the closure to each one of them until
    /// the closure returns `Err(..)`, then iteration ends
    /// 
    /// The closure is passed the `index`, and a mutable reference to the corrosponding element of the vector
    /// 
    /// This is similar to `Iterator::try_for_each`
    #[cfg(not(feature = "nightly"))]
    pub fn try_for_each<B, F: FnMut(usize, &mut Option<T>) -> Result<(), B>>(&mut self, mut f: F) -> Result<(), B> {
        self.try_fold((), move |(), i, x| f(i, x))
    }

    /// Iterates over all of the `Option<T>`s in the vector and applies the closure to each one
    /// 
    /// The closure is passed the `index`, and a mutable reference to the corrosponding element of the vector
    /// 
    /// This is similar to `Iterator::for_each`
    pub fn for_each<F: FnMut(usize, &mut Option<T>)>(&mut self, mut f: F) {
        self.fold((), move |(), i, x| f(i, x))
    }
}

impl<T> Drop for VecOption<T> {
    fn drop(&mut self) {
        if std::mem::needs_drop::<T>() {
            self.clear()
        }
    }
}

fn clone_impl<T: Clone>(vec: &VecOption<T>) -> VecOption<T> {
    vec.iter().map(|x| x.cloned()).collect()
}

impl<T: Clone> Clone for VecOption<T> {
    #[cfg(feature = "nightly")]
    default fn clone(&self) -> Self {
        clone_impl(self)
    }
    
    #[cfg(not(feature = "nightly"))]
    fn clone(&self) -> Self {
        clone_impl(self)
    }
}

#[cfg(feature = "nightly")]
impl<T: Copy> Clone for VecOption<T> {
    fn clone(&self) -> Self {
        let len = self.len();
        let mut new = Self {
            data: Vec::with_capacity(len),
            flag: self.flag.clone(),
        };

        unsafe {
            new.data.set_len(len);
            new.data.copy_from_slice(&self.data);
        }

        new
    }
}

impl<T: PartialEq> PartialEq for VecOption<T> {
    fn eq(&self, other: &Self) -> bool {
        self.iter().eq(other.iter())
    }
}

impl<T: PartialEq> PartialEq<[T]> for VecOption<T> {
    fn eq(&self, other: &[T]) -> bool {
        self.iter().eq(other.iter().map(Some))
    }
}

impl<T: PartialEq, S: AsRef<[Option<T>]>> PartialEq<S> for VecOption<T> {
    fn eq(&self, other: &S) -> bool {
        self.iter().eq(other.as_ref().iter().map(Option::as_ref))
    }
}

impl<T: Eq> Eq for VecOption<T> {}

impl<T: PartialOrd> PartialOrd for VecOption<T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.iter().partial_cmp(other.iter())
    }
}

impl<T: Ord> Ord for VecOption<T> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.iter().cmp(other.iter())
    }
}

use std::hash::{Hash, Hasher};

impl<T: Hash> Hash for VecOption<T> {
    fn hash<H: Hasher>(&self, hasher: &mut H) {
        self.iter().for_each(|i| i.hash(hasher))
    }
}

impl<T> std::iter::Extend<Option<T>> for VecOption<T> {
    fn extend<I: IntoIterator<Item = Option<T>>>(&mut self, iter: I) {
        let iter = iter.into_iter();

        let (additional, _) = iter.size_hint();

        self.reserve(additional);

        iter.for_each(|x| self.push(x));
    }
}

impl<T> std::iter::Extend<T> for VecOption<T> {
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        let iter = iter.into_iter();

        let (additional, _) = iter.size_hint();

        self.reserve(additional);

        iter.for_each(|x| self.push(x));
    }
}

impl<T> std::iter::FromIterator<Option<T>> for VecOption<T> {
    fn from_iter<I: IntoIterator<Item = Option<T>>>(iter: I) -> Self {
        let mut vec = Self::new();
        vec.extend(iter);
        vec
    }
}

impl<T> From<Vec<T>> for VecOption<T> {
    fn from(mut vec: Vec<T>) -> Self {
        let len = vec.len();

        let data = unsafe {
            Vec::from_raw_parts(vec.as_mut_ptr() as *mut MaybeUninit<T>, len, vec.capacity())
        };

        std::mem::forget(vec);

        let mut flag = BitVec::with_capacity(len);
        flag.grow(len, true);

        Self { data, flag }
    }
}

impl<T> From<Vec<Option<T>>> for VecOption<T> {
    fn from(vec: Vec<Option<T>>) -> Self {
        let mut vec_opt = VecOption::new();

        vec_opt.extend(vec);

        vec_opt
    }
}

impl<T> Drop for IntoIter<T> {
    fn drop(&mut self) {
        self.for_each(drop);
    }
}

/// This struct is created by the `into_iter` method on `VecOption` (provided by the `IntoIterator` trait).
pub struct IntoIter<T> {
    data: std::vec::IntoIter<MaybeUninit<T>>,
    flag: bit_vec::IntoIter,
}

impl<T> Iterator for IntoIter<T> {
    type Item = Option<T>;

    fn next(&mut self) -> Option<Self::Item> {
        unsafe {
            let flag = self.flag.next()?;
            let data = self.data.next().unwrap_unchecked();

            Some(from_raw_parts(flag, data))
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.data.size_hint()
    }

    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        if std::mem::needs_drop::<T>() {
            for _ in 1..n {
                self.next()?;
            }
            self.next()
        } else {
            unsafe {
                let flag = self.flag.nth(n)?;
                let data = self.data.nth(n).unwrap_unchecked();

                Some(from_raw_parts(flag, data))
            }
        }
    }
}

impl<T> DoubleEndedIterator for IntoIter<T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        unsafe {
            let flag = self.flag.next_back()?;
            let data = self.data.next_back().unwrap_unchecked();

            Some(from_raw_parts(flag, data))
        }
    }

    fn nth_back(&mut self, n: usize) -> Option<Self::Item> {
        if std::mem::needs_drop::<T>() {
            for _ in 1..n {
                self.next_back()?;
            }
            self.next_back()
        } else {
            unsafe {
                let flag = self.flag.nth_back(n)?;
                let data = self.data.nth_back(n).unwrap_unchecked();

                Some(from_raw_parts(flag, data))
            }
        }
    }
}

impl<T> ExactSizeIterator for IntoIter<T> {}
impl<T> std::iter::FusedIterator for IntoIter<T> {}

/// This struct is created by the `iter_mut` method on `VecOption`
pub struct IterMut<'a, T> {
    data: std::slice::IterMut<'a, MaybeUninit<T>>,
    flag: bit_vec::Iter<'a>,
}

impl<'a, T> Iterator for IterMut<'a, T> {
    type Item = Option<&'a mut T>;

    fn next(&mut self) -> Option<Self::Item> {
        unsafe {
            let flag = self.flag.next()?;
            let data = self.data.next().unwrap_unchecked();

            Some(ref_mut_from_raw_parts(flag, data))
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.data.size_hint()
    }

    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        unsafe {
            let data = self.data.nth(n)?;
            let flag = self.flag.nth(n).unwrap_unchecked();

            Some(ref_mut_from_raw_parts(flag, data))
        }
    }
}

impl<T> DoubleEndedIterator for IterMut<'_, T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        unsafe {
            let flag = self.flag.next_back()?;
            let data = self.data.next_back().unwrap_unchecked();

            Some(ref_mut_from_raw_parts(flag, data))
        }
    }

    fn nth_back(&mut self, n: usize) -> Option<Self::Item> {
        unsafe {
            let data = self.data.nth_back(n)?;
            let flag = self.flag.nth_back(n).unwrap_unchecked();

            Some(ref_mut_from_raw_parts(flag, data))
        }
    }
}

/// This struct is created by the `iter` method on `VecOption`
pub struct Iter<'a, T> {
    data: std::slice::Iter<'a, MaybeUninit<T>>,
    flag: bit_vec::Iter<'a>,
}

impl<'a, T> Iterator for Iter<'a, T> {
    type Item = Option<&'a T>;

    fn next(&mut self) -> Option<Self::Item> {
        unsafe {
            let flag = self.flag.next()?;
            let data = self.data.next().unwrap_unchecked();

            Some(ref_from_raw_parts(flag, data))
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.data.size_hint()
    }

    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        unsafe {
            let data = self.data.nth(n)?;
            let flag = self.flag.nth(n).unwrap_unchecked();

            Some(ref_from_raw_parts(flag, data))
        }
    }
}

impl<T> DoubleEndedIterator for Iter<'_, T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        unsafe {
            let flag = self.flag.next_back()?;
            let data = self.data.next_back().unwrap_unchecked();

            Some(ref_from_raw_parts(flag, data))
        }
    }

    fn nth_back(&mut self, n: usize) -> Option<Self::Item> {
        unsafe {
            let data = self.data.nth_back(n)?;
            let flag = self.flag.nth_back(n).unwrap_unchecked();

            Some(ref_from_raw_parts(flag, data))
        }
    }
}

impl<T> ExactSizeIterator for Iter<'_, T> {}
impl<T> std::iter::FusedIterator for Iter<'_, T> {}

impl<T> IntoIterator for VecOption<T> {
    type Item = Option<T>;
    type IntoIter = IntoIter<T>;

    fn into_iter(mut self) -> Self::IntoIter {
        IntoIter {
            data: std::mem::replace(&mut self.data, Vec::new()).into_iter(),
            flag: std::mem::replace(&mut self.flag, BitVec::new()).into_iter(),
        }
    }
}

impl<'a, T> IntoIterator for &'a mut VecOption<T> {
    type Item = Option<&'a mut T>;
    type IntoIter = IterMut<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

impl<'a, T> IntoIterator for &'a VecOption<T> {
    type Item = Option<&'a T>;
    type IntoIter = Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

use std::fmt;

impl<T: fmt::Debug> fmt::Debug for VecOption<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut f = f.debug_list();

        for i in self {
            f.entry(&i);
        }

        f.finish()
    }
}

#[test]
fn test() {
    let mut vec = VecOption::new();

    vec.push(10);
    vec.push(Some(20));

    vec.extend_none(10);

    vec.push(30);
    vec.push(40);
    vec.push(50);
    vec.push(60);

    assert_eq!(
        vec,
        [
            Some(10),
            Some(20),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            Some(30),
            Some(40),
            Some(50),
            Some(60)
        ]
    );

    vec.set_all_none();

    assert!(vec.iter().eq(std::iter::repeat(None).take(16)));

    vec.clear();

    assert!(vec.is_empty());

    vec.extend(vec![10, 30, 20]);

    assert_eq!(vec, [Some(10), Some(30), Some(20)]);
    assert_eq!(vec, [10, 30, 20][..]);

    assert_eq!(vec, vec.clone());

    assert_eq!(vec.take(1), Some(Some(30)));
    assert_eq!(vec.replace(1, 40), Some(None));
    assert_eq!(vec.take(1), Some(Some(40)));
    vec.swap(0, 1);
    assert_eq!(vec, [None, Some(10), Some(20)]);

    vec.clear();

    vec.extend(0..10);

    vec.for_each(|_, opt| {
        if let Some(ref mut x) = *opt {
            if *x % 2 == 0 {
                *opt = None
            } else {
                *x *= 2
            }
        }
    });

    assert_eq!(vec, [None, Some(2), None, Some(6), None, Some(10), None, Some(14), None, Some(18)]);
    
    let mut counter = 0;
    vec.for_each(|_, opt| {
        if let Some(ref mut x) = *opt {
            if *x % 3 == 0 {
                *x /= 2
            } else {
                *opt = None
            }
        } else {
            counter += 1;
            *opt = Some(counter);
        }
    });

    assert_eq!(vec, [Some(1), None, Some(2), Some(3), Some(3), None, Some(4), None, Some(5), Some(9)]);
}
