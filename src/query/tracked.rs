use core::{
    ops::{Deref, DerefMut},
    sync::atomic::{AtomicBool, Ordering},
};
use std::collections::BTreeMap;

use super::type_info::TypeInfo;

pub enum AccessMode {
    ReadOnly,
    ReadWrite,
}

pub trait Trackable<'a>
where
    Self: 'a,
{
    type Tracked: 'a;

    fn count_types() -> usize;

    /// Invoke `f` for every type that may be borrowed and whether the borrow is unique
    fn for_each_type(f: impl FnMut(TypeInfo, AccessMode));

    fn to_tracked(self, changes: &'a Changes) -> Self::Tracked;
}

impl<'a, T> Trackable<'a> for &'a T
where
    T: 'static,
{
    type Tracked = Tracked<'a, Self>;

    fn count_types() -> usize {
        1
    }

    fn for_each_type(mut f: impl FnMut(TypeInfo, AccessMode)) {
        f(TypeInfo::of::<T>(), AccessMode::ReadOnly);
    }

    fn to_tracked(self, changes: &'a Changes) -> Self::Tracked {
        Tracked::new(self, changes)
    }
}

impl<'a, T> Trackable<'a> for &'a mut T
where
    T: 'static,
{
    type Tracked = Tracked<'a, Self>;

    fn count_types() -> usize {
        1
    }

    fn for_each_type(mut f: impl FnMut(TypeInfo, AccessMode)) {
        f(TypeInfo::of::<T>(), AccessMode::ReadWrite);
    }

    fn to_tracked(self, changes: &'a Changes) -> Self::Tracked {
        Tracked::new(self, changes)
    }
}

impl<'a, T> Trackable<'a> for Option<T>
where
    T: Trackable<'a>,
{
    type Tracked = Option<<T as Trackable<'a>>::Tracked>;

    fn count_types() -> usize {
        <T as Trackable>::count_types()
    }

    fn for_each_type(f: impl FnMut(TypeInfo, AccessMode)) {
        <T as Trackable>::for_each_type(f)
    }

    fn to_tracked(self, changes: &'a Changes) -> Self::Tracked {
        self.map(|value| value.to_tracked(changes))
    }
}

pub struct Changes {
    changes: BTreeMap<TypeInfo, AtomicBool>,
}

impl Changes {
    pub(crate) fn new_for<'a, T: Trackable<'a>>(_: &T) -> Self {
        use std::collections::btree_map::Entry;
        let mut changes = BTreeMap::default();
        <T as Trackable>::for_each_type(|id, _| match changes.entry(id) {
            Entry::Vacant(entry) => {
                entry.insert(AtomicBool::new(false));
            }
            Entry::Occupied(_) => (),
        });
        Self { changes }
    }

    pub fn for_each_changed(&self, mut f: impl FnMut(TypeInfo)) {
        self.changes.iter().for_each(|(t, c)| {
            if c.load(Ordering::Relaxed) {
                f(*t)
            }
        })
    }

    pub fn set_changed<'a, T: Trackable<'a>>(&self) {
        <T as Trackable>::for_each_type(|id, _| {
            if let Some(value) = self.changes.get(&id) {
                value.store(true, Ordering::Relaxed);
            } else {
                // FIXME What we should do if item not found?
            }
        });
    }

    pub fn is_changed<'a, T: Trackable<'a>>(&self) -> bool {
        let mut ret = false;
        <T as Trackable>::for_each_type(|id, _| {
            ret = ret
                || match self.changes.get(&id) {
                    Some(value) => value.load(Ordering::Relaxed),
                    None => false, // FIXME What we should do if item not found?
                };
        });
        ret
    }
}

pub struct Tracked<'a, T>
where
    T: 'a + Trackable<'a>,
{
    value: T,
    changes: &'a Changes,
}

impl<'a, T> Tracked<'a, T>
where
    T: 'a + Trackable<'a>,
{
    fn new(value: T, changes: &'a Changes) -> Self {
        Self { value, changes }
    }
    pub fn set_changed(&self) {
        self.changes.set_changed::<T>();
    }
}

impl<'a, T> Deref for Tracked<'a, T>
where
    T: 'a + Deref + Trackable<'a>,
{
    type Target = <T as Deref>::Target;
    fn deref(&self) -> &Self::Target {
        &*(self.value)
    }
}

impl<'a, T> DerefMut for Tracked<'a, T>
where
    T: 'a + DerefMut + Trackable<'a>,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.set_changed();
        &mut *(self.value)
    }
}

#[cfg(test)]
mod tests {
    use super::{Changes, Trackable, TypeInfo};

    #[test]
    fn tracked_reference() {
        let mut value = 0u32;
        let reference = &mut value;
        let changes = Changes::new_for(&reference);
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

    #[test]
    fn tracked_option() {
        let mut value = 0u32;
        let reference = Some(&mut value);
        let changes = Changes::new_for(&reference);
        let mut tracked = reference.to_tracked(&changes);
        let mut changed_types = vec![];

        changes.for_each_changed(|t| changed_types.push(t));
        assert_eq!(changed_types.len(), 0);
        assert_eq!(tracked.as_deref().cloned(), Some(0));
        assert_eq!(changed_types.len(), 0);

        tracked
            .as_mut()
            .map(|v| {**v = 1});
        changes.for_each_changed(|t| changed_types.push(t));
        assert_eq!(changed_types.len(), 1);
        assert_eq!(tracked.as_deref().cloned(), Some(1));
        assert_eq!(changed_types.first(), Some(TypeInfo::of::<u32>()).as_ref());

        assert_eq!(value, 1);
    }
}
