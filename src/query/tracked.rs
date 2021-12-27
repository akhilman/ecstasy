use core::{
    marker::PhantomData,
    ops::{Deref, DerefMut},
    sync::atomic::{AtomicBool, Ordering},
};
use std::collections::BTreeMap;

use super::type_info::TypeInfo;

pub enum AccessMode {
    ReadOnly,
    ReadWrite,
}

pub trait Trackable {
    type Deref: 'static + Sized;

    fn count_types() -> usize;

    /// Invoke `f` for every type that may be borrowed and whether the borrow is unique
    fn for_each_type(f: impl FnMut(TypeInfo, AccessMode));
}

impl<'a, T> Trackable for &'a T
where
    T: 'static,
{
    type Deref = T;

    fn count_types() -> usize {
        1
    }

    fn for_each_type(mut f: impl FnMut(TypeInfo, AccessMode)) {
        f(TypeInfo::of::<T>(), AccessMode::ReadOnly);
    }
}

impl<'a, T> Trackable for &'a mut T
where
    T: 'static,
{
    type Deref = T;

    fn count_types() -> usize {
        1
    }

    fn for_each_type(mut f: impl FnMut(TypeInfo, AccessMode)) {
        f(TypeInfo::of::<T>(), AccessMode::ReadWrite);
    }
}

impl<T> Trackable for Option<T>
where
    T: Trackable,
{
    type Deref = Option<<T as Trackable>::Deref>;

    fn count_types() -> usize {
        <T as Trackable>::count_types()
    }

    fn for_each_type(f: impl FnMut(TypeInfo, AccessMode)) {
        <T as Trackable>::for_each_type(f)
    }
}

pub trait ToTracked<'c, C>
where
    C: Trackable,
{
    type Tracked: 'c;
    fn to_tracked(self, changes: &'c Changes<C>) -> Self::Tracked;
}

impl<'a, C, T> ToTracked<'a, C> for &'a T
where
    C: Trackable,
    T: 'static,
    &'a T: 'a + Trackable,
{
    type Tracked = Tracked<'a, Self>;
    fn to_tracked(self, changes: &'a Changes<C>) -> Self::Tracked {
        Tracked::new(self, changes.get_atomic::<&T>().expect("Type not tracked"))
    }
}

impl<'a, C, T> ToTracked<'a, C> for &'a mut T
where
    C: Trackable,
    T: 'static,
    &'a mut T: 'a + Trackable,
{
    type Tracked = Tracked<'a, Self>;
    fn to_tracked(self, changes: &'a Changes<C>) -> Self::Tracked {
        Tracked::new(
            self,
            changes.get_atomic::<&mut T>().expect("Type not tracked"),
        )
    }
}

impl<'a, C, T> ToTracked<'a, C> for Option<T>
where
    C: Trackable,
    T: 'a + Trackable + ToTracked<'a, C>,
{
    type Tracked = Option<<T as ToTracked<'a, C>>::Tracked>;
    fn to_tracked(self, changes: &'a Changes<C>) -> Self::Tracked {
        self.map(|value| value.to_tracked(changes))
    }
}

pub struct Changes<CT>
where
    CT: Trackable,
{
    changes: BTreeMap<TypeInfo, AtomicBool>,
    _phantom: PhantomData<CT>,
}

impl<CT> Changes<CT>
where
    CT: Trackable,
{
    fn get_atomic<T: Trackable>(&self) -> Option<&AtomicBool> {
        self.changes.get(&TypeInfo::of::<<T as Trackable>::Deref>())
    }

    pub fn for_each_changed(&self, mut f: impl FnMut(TypeInfo)) {
        self.changes.iter().for_each(|(t, c)| {
            if c.load(Ordering::Relaxed) {
                f(*t)
            }
        })
    }

    pub fn set_changed<T: Trackable>(&mut self) {
        use std::collections::btree_map::Entry;
        <T as Trackable>::for_each_type(|id, _| match self.changes.entry(id) {
            Entry::Vacant(_) => (), // FIXME What we should do if item not found?
            Entry::Occupied(entry) => {
                entry.into_mut().store(true, Ordering::Relaxed);
            }
        });
    }

    // TODO drop
    // pub fn is_changed<T: Trackable>(&self) -> bool {
    //     let mut ret = false;
    //     <T as Trackable>::for_each_type(|id, _| {
    //         ret = match self.changes.get(&id) {
    //             Some(value) => value.load(Ordering::Relaxed),
    //             None => false, // FIXME What we should do if item not found?
    //         };
    //     });
    //     ret
    // }
}

impl<CT> Default for Changes<CT>
where
    CT: Trackable,
{
    fn default() -> Self {
        use std::collections::btree_map::Entry;
        let mut changes = BTreeMap::default();
        <CT as Trackable>::for_each_type(|id, _| match changes.entry(id) {
            Entry::Vacant(entry) => {
                entry.insert(AtomicBool::new(false));
            }
            Entry::Occupied(_) => (),
        });
        Self {
            changes,
            _phantom: PhantomData,
        }
    }
}

pub struct Tracked<'a, T>
where
    T: 'a + Trackable,
{
    value: T,
    changed: &'a AtomicBool,
}

impl<'a, T> Tracked<'a, T>
where
    T: 'a + Trackable,
{
    pub fn new(value: T, changed: &'a AtomicBool) -> Self {
        Self { value, changed }
    }
    pub fn set_changed(&self) {
        self.changed.store(true, Ordering::Relaxed);
    }
}

impl<'a, T> Deref for Tracked<'a, T>
where
    T: 'a + Trackable + Deref,
{
    type Target = <T as Deref>::Target;
    fn deref(&self) -> &Self::Target {
        &*(self.value)
    }
}

impl<'a, T> DerefMut for Tracked<'a, T>
where
    T: 'a + Trackable + DerefMut,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.set_changed();
        &mut *(self.value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tracked_reference() {
        let mut value = 0u32;
        let reference = &mut value;
        let changes = Changes::<&mut u32>::default();
        let mut tracked = reference.to_tracked(&changes);
        let mut changed_types = vec![];

        changes.for_each_changed(|t| changed_types.push(t));
        assert_eq!(changed_types.len(), 0);
        assert_eq!(*tracked, 0);
        assert_eq!(changed_types.len(), 0);

        *tracked = 1;
        changes.for_each_changed(|t| changed_types.push(t));
        assert_eq!(changed_types.len(), 1);
        assert_eq!(*tracked, 1);
        assert_eq!(changed_types.first(), Some(TypeInfo::of::<u32>()).as_ref());

        assert_eq!(value, 1);
    }
}
