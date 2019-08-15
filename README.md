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