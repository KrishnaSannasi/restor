use parking_lot::{MappedMutexGuard, MappedRwLockReadGuard, MappedRwLockWriteGuard};
use std::any::{Any, TypeId};
use std::cell::{Ref, RefMut};
use std::collections::HashMap;
use std::ops::{Deref, DerefMut};

mod unit;

pub use crate::black_box::unit::{DynamicResult, ErrorDesc, StorageUnit, Unit, UnitError};
use crate::concurrent_black_box::{MutexUnit, RwLockUnit};

mod refcell_unit;

pub use crate::black_box::refcell_unit::*;
use std::marker::PhantomData;

pub type RefCellUnitTrait = dyn for<'a> Unit<
    'a,
    Borrowed = Ref<'a, dyn Any>,
    MutBorrowed = RefMut<'a, dyn Any>,
    Owned = Box<dyn Any>,
>;
pub type MutexUnitTrait = dyn for<'a> Unit<
    'a,
    Borrowed = MappedMutexGuard<'a, dyn Any>,
    MutBorrowed = MappedMutexGuard<'a, dyn Any>,
    Owned = Box<dyn Any>,
>;
pub type RwLockUnitTrait = for<'a> Unit<
    'a,
    Borrowed = MappedRwLockReadGuard<'a, dyn Any>,
    MutBorrowed = MappedRwLockWriteGuard<'a, dyn Any>,
    Owned = Box<dyn Any>,
>;

pub trait Map<I: ?Sized, O: ?Sized>: Deref<Target = I> + Sized {
    type Output: Deref<Target = O>;
    type Func: Sized + 'static;
    fn map(self, f: Self::Func) -> Self::Output;
}

impl<'a, I: 'static + ?Sized, O: 'static + ?Sized> Map<I, O> for Ref<'a, I> {
    type Output = Ref<'a, O>;
    type Func = for<'b> fn(&'b I) -> &'b O;
    fn map(self, f: Self::Func) -> Ref<'a, O> {
        Ref::map(self, f)
    }
}

impl<'a, I: 'static + Sync + Send + ?Sized, O: 'static + Sync + Send + ?Sized> Map<I, O>
    for MappedMutexGuard<'a, I>
{
    type Output = MappedMutexGuard<'a, O>;
    type Func = for<'b> fn(&'b mut I) -> &'b mut O;
    fn map(self, f: Self::Func) -> MappedMutexGuard<'a, O> {
        MappedMutexGuard::map(self, f)
    }
}

impl<'a, I: 'static + Sync + Send + ?Sized, O: 'static + Sync + Send + ?Sized> Map<I, O>
    for MappedRwLockReadGuard<'a, I>
{
    type Output = MappedRwLockReadGuard<'a, O>;
    type Func = for<'b> fn(&'b I) -> &'b O;
    fn map(self, f: Self::Func) -> MappedRwLockReadGuard<'a, O> {
        MappedRwLockReadGuard::map(self, f)
    }
}

pub trait MapMut<I: ?Sized, O: ?Sized>: Deref<Target = I> + Sized + DerefMut {
    type Output: Deref<Target = O> + DerefMut;
    type Func: Sized + 'static;
    fn map(self, f: Self::Func) -> Self::Output;
}

impl<'a, I: 'static + ?Sized, O: 'static + ?Sized> MapMut<I, O> for RefMut<'a, I> {
    type Output = RefMut<'a, O>;
    type Func = for<'b> fn(&'b mut I) -> &'b mut O;
    fn map(self, f: Self::Func) -> RefMut<'a, O> {
        RefMut::map(self, f)
    }
}

impl<'a, I: 'static + Sync + Send + ?Sized, O: 'static + Sync + Send + ?Sized> MapMut<I, O>
    for MappedRwLockWriteGuard<'a, I>
{
    type Output = MappedRwLockWriteGuard<'a, O>;
    type Func = for<'b> fn(&'b mut I) -> &'b mut O;
    fn map(self, f: Self::Func) -> MappedRwLockWriteGuard<'a, O> {
        MappedRwLockWriteGuard::map(self, f)
    }
}

impl<'a, I: 'static + Sync + Send + ?Sized, O: 'static + Sync + Send + ?Sized> MapMut<I, O>
    for MappedMutexGuard<'a, I>
{
    type Output = MappedMutexGuard<'a, O>;
    type Func = for<'b> fn(&'b mut I) -> &'b mut O;
    fn map(self, f: Self::Func) -> MappedMutexGuard<'a, O> {
        MappedMutexGuard::map(self, f)
    }
}

pub struct BlackBox<U: ?Sized> {
    data: HashMap<TypeId, Box<U>>,
}

type Borrowed<'a, T: Unit<'a>> = <T as Unit<'a>>::Borrowed;
type MutBorrowed<'a, T: Unit<'a>> = <T as Unit<'a>>::MutBorrowed;

impl<U: ?Sized + for<'a> Unit<'a, Owned = Box<dyn Any>>> BlackBox<U>
{
    pub fn new() -> Self {
        Self {
            data: HashMap::new(),
        }
    }

    pub fn insert<T: 'static>(&self, data: T) -> Option<(T, ErrorDesc)> {
        let entry = self.data.get(&TypeId::of::<T>());
        match entry {
            Some(x) => match x.insert_any(Box::new(data)) {
                Some((x, e)) => Some((*x.downcast().unwrap(), e)),
                None => None,
            },
            None => Some((data, ErrorDesc::NoAllocatedUnit)),
        }
    }

    pub fn insert_many<T: 'static>(&self, data: Vec<T>) -> Option<(Vec<T>, ErrorDesc)> {
        if let Some(unit) = self.data.get(&TypeId::of::<T>()) {
            if let Some((ret, e)) = unit.insert_any(Box::new(data)) {
                Some((*ret.downcast().unwrap(), e))
            } else {
                None
            }
        } else {
            None
        }
    }

    #[inline]
    fn unit_get<'a, T: 'static>(
        &'a self,
    ) -> DynamicResult<&U> {
        self.data
            .get(&TypeId::of::<T>())
            .map(|x| &**x)
            .ok_or(ErrorDesc::NoAllocatedUnit)
    }

    #[inline]
    pub fn get_mut<'a, T: 'static>(&'a self) -> DynamicResult<<MutBorrowed<'a, U> as MapMut<dyn Any, T>>::Output>
    where
        MutBorrowed<'a, U>: MapMut<dyn Any, T, Func = fn(&mut dyn Any) -> &mut T>,
    {
        Ok(Self::unit_get::<T>(self)?.one_mut()?.map(|x| x.downcast_mut().unwrap()))
    }

    #[inline]
    pub fn ind_mut<'a, T: 'static>(
        &'a self,
        ind: usize,
    ) -> DynamicResult<<MutBorrowed<'a, U> as MapMut<dyn Any, T>>::Output>
    where
        MutBorrowed<'a, U>: MapMut<dyn Any, T, Func = fn(&mut dyn Any) -> &mut T>,
    {
        Ok(self.unit_get::<T>()?.ind_mut(ind)?.map(|x| {
            x.downcast_mut().unwrap()
        }))
    }

    #[inline]
    pub fn extract<T: 'static>(&self) -> DynamicResult<T> {
        Ok(*self.unit_get::<T>()?.extract()?.downcast().unwrap())
    }

    #[inline]
    pub fn extract_many<T: 'static>(&self) -> DynamicResult<Box<[T]>> {
        Ok(*self.unit_get::<T>()?.extract_many()?.downcast().unwrap())
    }

    #[inline]
    pub fn get<'a, T: 'static>(&'a self) -> DynamicResult<<Borrowed<'a, U> as Map<dyn Any, T>>::Output>
    where
        Borrowed<'a, U>: Map<dyn Any, T, Func = for<'b> fn(&'b dyn Any) -> &'b T>,
    {
        Ok(self.unit_get::<T>()?.one()?.map(|x| {
            x.downcast_ref().unwrap()
        }))
    }
    #[inline]
    pub fn ind<'a, T: 'static>(&'a self, ind: usize) -> DynamicResult<<Borrowed<'a, U> as Map<dyn Any, T>>::Output>
    where
        Borrowed<'a, U>: Map<dyn Any, T, Func = for<'b> fn(&'b dyn Any) -> &'b T>,
    {
        Ok(self.unit_get::<T>()?.ind(ind)?.map(|x| {
            x.downcast_ref().unwrap()
        }))
    }
}

impl
    BlackBox<
        dyn for<'a> Unit<
            'a,
            Borrowed = MappedRwLockReadGuard<'a, dyn Any>,
            MutBorrowed = MappedRwLockWriteGuard<'a, dyn Any>,
            Owned = Box<dyn Any>,
        >,
    >
{
    #[inline]
    pub fn allocate_for<T: 'static + Send + Sync>(&mut self) {
        if !self.data.contains_key(&TypeId::of::<T>()) {
            self.data.insert(
                TypeId::of::<T>(),
                Box::new(RwLockUnit::new(StorageUnit::<T>::new())),
            );
        }
    }
}

impl
    BlackBox<
        dyn for<'a> Unit<
            'a,
            Borrowed = MappedMutexGuard<'a, dyn Any>,
            MutBorrowed = MappedMutexGuard<'a, dyn Any>,
            Owned = Box<dyn Any>,
        >,
    >
{
    #[inline]
    pub fn allocate_for<T: 'static + Send + Sync>(&mut self) {
        if !self.data.contains_key(&TypeId::of::<T>()) {
            self.data.insert(
                TypeId::of::<T>(),
                Box::new(MutexUnit::new(StorageUnit::<T>::new())),
            );
        }
    }
}

impl
    BlackBox<
        dyn for<'a> Unit<
            'a,
            Borrowed = Ref<'a, dyn Any>,
            MutBorrowed = RefMut<'a, dyn Any>,
            Owned = Box<dyn Any>,
        >,
    >
{
    #[inline]
    pub fn allocate_for<T: 'static>(&mut self) {
        if !self.data.contains_key(&TypeId::of::<T>()) {
            self.data.insert(
                TypeId::of::<T>(),
                Box::new(RefCellUnit::new(StorageUnit::<T>::new())),
            );
        }
    }
}
