use std::any::{Any, TypeId};
use std::fmt::{Debug, Formatter};
use std::mem::swap;
use std::ops::{Deref, DerefMut};

pub type DynamicResult<Ok> = Result<Ok, ErrorDesc>;

/// The basic error descriptions for why a dynamically typed resource operation didn't work. It does
/// not contain however, the description for unit-related errors which handled with a `UnitError` by
/// using the `Unit` variant of `ErrorDesc`.
#[derive(Debug, PartialEq)]
pub enum ErrorDesc {
    /// Returned if there is an incompatible borrow on the contents of the unit. It follows the same
    /// rules for runtime checking as a `RefCell<T>`. Usually bundled with a `Ref<T>`/`RefMut<T>` in
    /// a `Result<RefVariant<T>, ErrorDesc>`.
    /// ## Example:
    /// ```
    /// # use restor::*;
    /// # fn main() {
    /// let mut storage = DynamicStorage::new();
    /// storage.allocate_for::<usize>();
    /// storage.insert(0usize);
    /// let x = storage.get::<usize>().unwrap();
    /// let y = storage.get_mut::<usize>();
    /// assert!(y.is_err());
    /// # }
    /// ```
    BorrowedIncompatibly,
    /// Returned when there is no unit allocated for the type that was requested. Allocate a unit to
    /// contain a `<T>` with `DynamicStorage::allocate_for::<T>(&mut self)`. Note that `<T>` must be
    /// `T: Sized + Any + 'static`.
    /// ## Example:
    /// ```
    /// # use restor::*;
    /// # fn main() {
    /// let mut storage = DynamicStorage::new();
    /// let x = storage.get::<usize>();
    /// assert!(x.is_err());
    /// // Error, there is no unit for `usize` allocated!
    /// storage.allocate_for::<usize>();
    /// storage.insert::<usize>(10);
    /// let x = storage.get::<usize>().unwrap();
    /// assert_eq!(*x, 10);
    /// # }
    /// ```
    NoAllocatedUnit,
    /// This is an internal error that should be ignored by the user. This should never be created.
    NoMatchingType,
    /// This holds an inner `ErrorDesc`. Call `unwrap` on an `Inner` variant to get the inner error.
    Inner(Box<ErrorDesc>),
    /// Contains an error specific to unit operations. Please refer to the `UnitError` documentation
    /// for more information.
    Unit(UnitError),
}

impl ErrorDesc {
    /// Consumes the `ErrorDesc` and returns an `ErrorDesc` if it's an `Inner` variant. Panics if it
    /// is not an `ErrorDesc::Inner` variant.
    pub fn unwrap(self) -> ErrorDesc {
        if let ErrorDesc::Inner(inner) = self {
            *inner
        } else {
            panic!("Try to unwrap a non-inner ErrorDesc value!")
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum UnitError {
    IsNotOne,
    IsNotMany,
    OutOfBounds,
}

pub enum StorageUnit<T: Sized + 'static> {
    Nope,
    One(T),
    Many(Vec<T>),
}

impl<T: Sized> StorageUnit<T> {
    pub fn new() -> Self {
        StorageUnit::Nope
    }

    pub fn insert(&mut self, new: T) {
        match self {
            StorageUnit::Nope => {
                *self = StorageUnit::One(new);
            }
            StorageUnit::One(_) => {
                let mut rep = StorageUnit::Many(vec![new]);
                swap(self, &mut rep);
                if let StorageUnit::One(prev) = rep {
                    if let StorageUnit::Many(v) = self {
                        v.insert(0, prev);
                    } else {
                        unreachable!()
                    }
                } else {
                    unreachable!()
                }
            }
            StorageUnit::Many(many) => {
                many.push(new);
            }
        }
    }

    pub fn insert_many(&mut self, new: Vec<T>) {
        match self {
            StorageUnit::Nope => {
                *self = StorageUnit::Many(new.into());
            }
            StorageUnit::One(_) => {
                let mut rep = StorageUnit::Many(new.into());
                swap(&mut rep, self);
                if let StorageUnit::One(val) = rep {
                    if let StorageUnit::Many(vec) = self {
                        vec.insert(0, val);
                    } else {
                        unreachable!()
                    }
                } else {
                    unreachable!()
                }
            }
            StorageUnit::Many(arr) => {
                arr.append(&mut new.into());
            }
        }
    }

    pub fn one(&self) -> DynamicResult<&T> {
        if let StorageUnit::One(x) = self {
            Ok(x)
        } else {
            Err(ErrorDesc::Unit(UnitError::IsNotOne))
        }
    }

    pub fn one_mut(&mut self) -> DynamicResult<&mut T> {
        if let StorageUnit::One(x) = self {
            Ok(x)
        } else {
            Err(ErrorDesc::Unit(UnitError::IsNotOne))
        }
    }

    pub fn many(&self) -> DynamicResult<&[T]> {
        if let StorageUnit::Many(x) = self {
            Ok(x)
        } else {
            Err(ErrorDesc::Unit(UnitError::IsNotMany))
        }
    }

    pub fn many_mut(&mut self) -> DynamicResult<&mut Vec<T>> {
        if let StorageUnit::Many(x) = self {
            Ok(x)
        } else {
            Err(ErrorDesc::Unit(UnitError::IsNotMany))
        }
    }

    pub fn extract_one(&mut self) -> DynamicResult<T> {
        match self {
            StorageUnit::Nope => Err(ErrorDesc::Unit(UnitError::IsNotOne)),
            StorageUnit::Many(_) => Err(ErrorDesc::Unit(UnitError::IsNotOne)),
            StorageUnit::One(_) => {
                let mut repl = StorageUnit::Nope;
                swap(&mut repl, self);
                if let StorageUnit::One(data) = repl {
                    Ok(data)
                } else {
                    unreachable!()
                }
            }
        }
    }

    pub fn extract_many(&mut self) -> DynamicResult<Vec<T>> {
        match self {
            StorageUnit::Nope => Err(ErrorDesc::Unit(UnitError::IsNotMany)),
            StorageUnit::One(_) => Err(ErrorDesc::Unit(UnitError::IsNotMany)),
            StorageUnit::Many(_) => {
                let mut repl = StorageUnit::Nope;
                swap(&mut repl, self);
                if let StorageUnit::Many(data) = repl {
                    Ok(data)
                } else {
                    unreachable!()
                }
            }
        }
    }

    pub fn extract_many_boxed(&mut self) -> DynamicResult<Box<[T]>> {
        match self {
            StorageUnit::Nope => Err(ErrorDesc::Unit(UnitError::IsNotMany)),
            StorageUnit::One(_) => Err(ErrorDesc::Unit(UnitError::IsNotMany)),
            StorageUnit::Many(_) => {
                let mut repl = StorageUnit::Nope;
                swap(&mut repl, self);
                if let StorageUnit::Many(data) = repl {
                    Ok(data.into_boxed_slice())
                } else {
                    unreachable!()
                }
            }
        }
    }
}

impl<T: Clone> Clone for StorageUnit<T> {
    fn clone(&self) -> Self {
        match self {
            StorageUnit::Nope => StorageUnit::Nope,
            StorageUnit::One(data) => StorageUnit::One(data.clone()),
            StorageUnit::Many(data) => StorageUnit::Many(data.clone()),
        }
    }
}

pub trait Unit<'a> {
    type Borrowed: Deref<Target = dyn Any> + 'a;
    type MutBorrowed: Deref<Target = dyn Any> + DerefMut + 'a;
    type Owned: Deref<Target = dyn Any> + DerefMut;

    fn one(&'a self) -> DynamicResult<Self::Borrowed>;
    fn one_mut(&'a self) -> DynamicResult<Self::MutBorrowed>;

    fn ind(&'a self, ind: usize) -> DynamicResult<Self::Borrowed>;
    fn ind_mut(&'a self, ind: usize) -> DynamicResult<Self::MutBorrowed>;

    fn extract(&self) -> DynamicResult<Self::Owned>;
    fn extract_ind(&self, ind: usize) -> DynamicResult<Self::Owned>;
    fn extract_many(&self) -> DynamicResult<Self::Owned>;

    fn insert_any(&self, new: Self::Owned) -> Option<(Self::Owned, ErrorDesc)>;

    fn id(&self) -> TypeId;
}

impl<
        'a,
        R: Deref<Target = dyn Any> + 'a,
        RM: Deref<Target = dyn Any> + DerefMut + 'a,
        O: Deref<Target = dyn Any> + DerefMut,
    > PartialEq for dyn Unit<'a, Borrowed = R, MutBorrowed = RM, Owned = O>
{
    fn eq(&self, other: &Self) -> bool {
        self.id() == other.id()
    }
}

impl<
        'a,
        R: Deref<Target = dyn Any> + 'a,
        RM: Deref<Target = dyn Any> + DerefMut + 'a,
        O: Deref<Target = dyn Any> + DerefMut,
    > Debug for dyn Unit<'a, Borrowed = R, MutBorrowed = RM, Owned = O>
{
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        write!(f, "Unit(TypeId: {:?})", self.id())
    }
}
