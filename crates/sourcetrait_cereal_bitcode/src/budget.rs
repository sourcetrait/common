//! A decode held to a budget: bounds on the collection elements it decodes, the
//! heap its values take, and how deeply its collections nest, each checked when
//! a collection's lengths are read, before its elements are read or allocated.
//!
//! A sourcetrait addition to bitcode. A decode outside [`decode_budgeted`]
//! charges nothing and runs as upstream's does.
//!
//! A decoder outside the crate charges the budget through [`enter`], [`heap`],
//! and [`unbudgeted`] from its [`View::populate`](crate::coder::View::populate),
//! before it reads or allocates what it charges. Each only spends the scope
//! [`decode_budgeted`] opens, and the [`Level`] that [`enter`] takes gives back
//! to that scope alone.
use crate::coder::Result;
use crate::error::Error;
use core::fmt::{self, Display, Formatter};
use core::marker::PhantomData;
use core::mem::size_of;

/// The bounds one decode is held to.
///
/// `heap` bounds the allocations the decoded value keeps, each collection and
/// pointer charged by its own representation's layout: a vector's elements, a
/// string's bytes, a linked list's nodes, a box's value, an `Rc`'s or an
/// `Arc`'s value with its counts. The decoder's own column buffers, freed when
/// the decode ends, are outside it, and scale with what it charges. A B-tree
/// or a hash collection has no bound yet, and a budgeted decode refuses one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Budget {
    /// The most collection elements in all, a byte buffer's bytes excepted.
    pub elements: u64,
    /// The most heap bytes the decoded value keeps.
    pub heap: u64,
    /// The deepest nesting of collections and smart pointers holding values.
    pub depth: u32,
}

/// The bound a budgeted decode passed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Exceeded {
    /// More collection elements than the budget allows.
    Elements,
    /// More heap than the budget allows.
    Heap,
    /// Collections nested deeper than the budget allows.
    Depth,
}

impl Display for Exceeded {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Elements => "elements",
            Self::Heap => "heap",
            Self::Depth => "depth",
        })
    }
}

/// Why a budgeted decode failed: the input itself, a bound it passed, or a
/// collection no budget bounds.
#[derive(Debug)]
pub enum BudgetError {
    /// The input did not decode as its type.
    Decode(Error),
    /// The input declared more than the budget allows.
    Exceeded(Exceeded),
    /// The input holds a B-tree or a hash collection, whose heap no budget
    /// bounds yet.
    Unbudgeted,
}

impl Display for BudgetError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Decode(e) => write!(f, "{e}"),
            Self::Exceeded(bound) => write!(f, "the decode passed its {bound} bound"),
            Self::Unbudgeted => f.write_str("the input holds a collection no budget bounds"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for BudgetError {}

/// Why a scope's decode stopped.
#[cfg(feature = "std")]
#[derive(Clone, Copy)]
enum Stop {
    Exceeded(Exceeded),
    Unbudgeted,
}

/// The size of an allocation laying a value of `size` and `align` after two
/// pointer-sized words, padded to their joint alignment: a linked list's node
/// and an `Rc`'s or an `Arc`'s allocation take this or less.
pub(crate) const fn after_two_words(size: usize, align: usize) -> usize {
    let word = size_of::<usize>();
    let offset = (2 * word).div_ceil(align) * align;
    let joint = if align > word { align } else { word };
    (offset + size).div_ceil(joint) * joint
}

/// The most an allocation laying a slice of elements aligned to `align` after
/// two pointer-sized words takes besides its elements: the words padded to the
/// elements' alignment, and the most padding the size takes to the joint
/// alignment. An `Rc`'s or an `Arc`'s slice or `str` takes this or less.
pub(crate) const fn slice_after_two_words(align: usize) -> usize {
    let word = size_of::<usize>();
    let offset = (2 * word).div_ceil(align) * align;
    offset + word.saturating_sub(align)
}

/// The nesting level a collection's elements take while they populate, given
/// back to the scope it was taken from when it drops. It stays on the thread
/// that took it.
#[must_use = "a level is held while the collection's elements populate"]
pub struct Level {
    /// The scope the level was taken from, if one was.
    #[cfg(feature = "std")]
    scope: Option<u64>,
    /// Keeps the level on its thread, whose scope it gives back to.
    _thread: PhantomData<*const ()>,
}

#[cfg(feature = "std")]
mod scoped {
    use super::*;
    use crate::error::err;
    use core::cell::Cell;

    /// What a scope's decode has left of its budget.
    #[derive(Clone, Copy)]
    struct Left {
        elements: u64,
        heap: u64,
        depth: u32,
        stopped: Option<Stop>,
        /// The scope's number, unique on its thread.
        scope: u64,
    }

    std::thread_local! {
        static LEFT: Cell<Option<Left>> = const { Cell::new(None) };
        /// The number of the thread's last scope.
        static SCOPES: Cell<u64> = const { Cell::new(0) };
    }

    /// The thread's budget for one decode, the outer scope's restored on drop.
    pub(crate) struct Scope {
        outer: Option<Left>,
    }

    impl Scope {
        /// A scope holding the thread's decodes to `budget` until it drops.
        pub(crate) fn enter(budget: Budget) -> Self {
            let scope = SCOPES.with(|s| {
                let scope = s.get().wrapping_add(1);
                s.set(scope);
                scope
            });
            let left = Left {
                elements: budget.elements,
                heap: budget.heap,
                depth: budget.depth,
                stopped: None,
                scope,
            };
            Self {
                outer: LEFT.with(|l| l.replace(Some(left))),
            }
        }

        /// Why the scope's decode stopped, if a charge stopped it.
        pub(super) fn stopped(&self) -> Option<Stop> {
            LEFT.with(|l| l.get().and_then(|left| left.stopped))
        }
    }

    impl Drop for Scope {
        fn drop(&mut self) {
            LEFT.with(|l| l.set(self.outer));
        }
    }

    /// `left` changed by `take`, in one visit to the thread's budget, refused
    /// at the first stop it returns: the scope's number, or none outside one.
    #[inline(always)]
    fn charge(take: impl FnOnce(&mut Left) -> Option<Stop>) -> Result<Option<u64>> {
        LEFT.with(|l| {
            let Some(mut left) = l.get() else {
                return Ok(None);
            };
            match take(&mut left) {
                None => {
                    l.set(Some(left));
                    Ok(Some(left.scope))
                }
                Some(stop) => {
                    left.stopped = left.stopped.or(Some(stop));
                    l.set(Some(left));
                    err("budget exceeded")
                }
            }
        })
    }

    /// Charges a collection's `n` elements of `each` heap bytes, counted as
    /// elements when `counted`, and takes a nesting level while they populate
    /// when there are any: one visit to the thread's budget, refused at the
    /// first bound passed. An empty collection charges nothing and takes no
    /// level. Outside a budgeted decode it charges nothing.
    #[inline]
    pub fn enter(n: usize, each: usize, counted: bool) -> Result<Level> {
        if n == 0 {
            return Ok(Level {
                scope: None,
                _thread: PhantomData,
            });
        }
        let scope = charge(|left| {
            let n = n as u64;
            if counted {
                if n > left.elements {
                    return Some(Stop::Exceeded(Exceeded::Elements));
                }
                left.elements -= n;
            }
            let heap = n.saturating_mul(each as u64);
            if heap > left.heap {
                return Some(Stop::Exceeded(Exceeded::Heap));
            }
            left.heap -= heap;
            if left.depth == 0 {
                return Some(Stop::Exceeded(Exceeded::Depth));
            }
            left.depth -= 1;
            None
        })?;
        Ok(Level {
            scope,
            _thread: PhantomData,
        })
    }

    /// Charges `bytes` of heap alone, nesting nothing: a string's. Outside a
    /// budgeted decode it charges nothing.
    #[inline]
    pub fn heap(bytes: usize) -> Result<()> {
        if bytes == 0 {
            return Ok(());
        }
        charge(|left| {
            let bytes = bytes as u64;
            if bytes > left.heap {
                return Some(Stop::Exceeded(Exceeded::Heap));
            }
            left.heap -= bytes;
            None
        })
        .map(drop)
    }

    /// Refuses `length` collections whose heap no budget bounds inside a
    /// budgeted decode, when there are any; outside one it allows them.
    #[inline]
    pub fn unbudgeted(length: usize) -> Result<()> {
        if length == 0 {
            return Ok(());
        }
        charge(|_| Some(Stop::Unbudgeted)).map(drop)
    }

    impl Drop for Level {
        #[inline]
        fn drop(&mut self) {
            if let Some(scope) = self.scope {
                LEFT.with(|l| {
                    // A level gives back to the scope it was taken from alone.
                    if let Some(mut left) = l.get().filter(|left| left.scope == scope) {
                        left.depth += 1;
                        l.set(Some(left));
                    }
                });
            }
        }
    }
}

#[cfg(feature = "std")]
pub use scoped::{enter, heap, unbudgeted};

/// A collection's elements, unbounded without `std`'s scopes.
#[cfg(not(feature = "std"))]
#[inline(always)]
pub fn enter(_: usize, _: usize, _: bool) -> Result<Level> {
    Ok(Level {
        _thread: PhantomData,
    })
}

/// A string's bytes, unbounded without `std`'s scopes.
#[cfg(not(feature = "std"))]
#[inline(always)]
pub fn heap(_: usize) -> Result<()> {
    Ok(())
}

/// A collection no budget bounds, allowed without `std`'s scopes.
#[cfg(not(feature = "std"))]
#[inline(always)]
pub fn unbudgeted(_: usize) -> Result<()> {
    Ok(())
}

/// `T` decoded from `bytes` within `budget`, every collection's elements, heap,
/// and nesting charged as its lengths are read, before its elements are.
#[cfg(feature = "std")]
pub fn decode_budgeted<'a, T: crate::Decode<'a>>(
    bytes: &'a [u8],
    budget: Budget,
) -> core::result::Result<T, BudgetError> {
    let scope = scoped::Scope::enter(budget);
    let decoded = crate::decode::<T>(bytes);
    let stopped = scope.stopped();
    drop(scope);
    match (decoded, stopped) {
        (Ok(value), _) => Ok(value),
        (Err(_), Some(Stop::Exceeded(bound))) => Err(BudgetError::Exceeded(bound)),
        (Err(_), Some(Stop::Unbudgeted)) => Err(BudgetError::Unbudgeted),
        (Err(e), None) => Err(BudgetError::Decode(e)),
    }
}
