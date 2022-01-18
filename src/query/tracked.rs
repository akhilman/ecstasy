use core::{
    ops::{Deref, DerefMut},
    sync::atomic::{AtomicBool, Ordering},
};
use std::collections::BTreeMap;

use super::ElementTypeId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AccessMode {
    ReadOnly,
    ReadWrite,
}

pub trait Trackable<'a> {
    type Tracked;

    fn count_types() -> usize;

    /// Invoke `f` for every type that may be borrowed and whether the borrow is unique
    fn for_each_type(f: impl FnMut(ElementTypeId, AccessMode));

    fn into_tracked(self, changes: &'a Changes) -> Self::Tracked;
}

impl<'a, T> Trackable<'a> for &'a T
where
    T: 'static,
{
    type Tracked = TrackedRef<'a, T>;

    fn count_types() -> usize {
        1
    }

    fn for_each_type(mut f: impl FnMut(ElementTypeId, AccessMode)) {
        f(ElementTypeId::of::<T>(), AccessMode::ReadOnly);
    }

    fn into_tracked(self, changes: &'a Changes) -> Self::Tracked {
        TrackedRef::new(self, changes)
    }
}

impl<'a, T> Trackable<'a> for &'a mut T
where
    T: 'static,
{
    type Tracked = TrackedMut<'a, T>;

    fn count_types() -> usize {
        1
    }

    fn for_each_type(mut f: impl FnMut(ElementTypeId, AccessMode)) {
        f(ElementTypeId::of::<T>(), AccessMode::ReadWrite);
    }

    fn into_tracked(self, changes: &'a Changes) -> Self::Tracked {
        TrackedMut::new(self, changes)
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

    fn for_each_type(f: impl FnMut(ElementTypeId, AccessMode)) {
        <T as Trackable>::for_each_type(f)
    }

    fn into_tracked(self, changes: &'a Changes) -> Self::Tracked {
        self.map(|value| value.into_tracked(changes))
    }
}

pub struct Changes {
    changes: BTreeMap<ElementTypeId, AtomicBool>,
}

impl Changes {
    pub(crate) fn new() -> Self {
        Self {
            changes: BTreeMap::new(),
        }
    }
    pub(crate) fn new_for<'a, T: Trackable<'a>>(_: &T) -> Self {
        let mut changes = Self::new();
        T::for_each_type(|t, _| changes.reserve(t));
        changes
    }

    pub(crate) fn reserve(&mut self, type_id: ElementTypeId) {
        use std::collections::btree_map::Entry;
        match self.changes.entry(type_id) {
            Entry::Vacant(entry) => {
                entry.insert(AtomicBool::new(false));
            }
            Entry::Occupied(_) => (),
        }
    }

    pub fn for_each_changed(&self, mut f: impl FnMut(ElementTypeId)) {
        self.changes.iter().for_each(|(t, c)| {
            if c.load(Ordering::Relaxed) {
                f(*t)
            }
        })
    }

    pub(crate) fn get_atomic(&self, type_id: ElementTypeId) -> Option<&AtomicBool> {
        self.changes.get(&type_id)
    }

    pub fn set_changed(&self, type_id: ElementTypeId) {
        if let Some(value) = self.changes.get(&type_id) {
            value.store(true, Ordering::Relaxed);
        } else {
            panic!("Changed flag for {} is not reserved", type_id);
        }
    }

    pub fn is_changed(&self, type_id: ElementTypeId) -> bool {
        match self.changes.get(&type_id) {
            Some(value) => value.load(Ordering::Relaxed),
            None => false,
        }
    }
}

pub struct TrackedRef<'a, T>
where
    T: 'static,
{
    value: &'a T,
    changes: &'a Changes,
}

impl<'a, T> TrackedRef<'a, T>
where
    T: 'static,
{
    fn new(value: &'a T, changes: &'a Changes) -> Self {
        Self { value, changes }
    }
    pub fn set_changed(&self) {
        self.changes.set_changed(ElementTypeId::of::<T>())
    }
}

pub struct TrackedMut<'a, T>
where
    T: 'static,
{
    value: &'a mut T,
    changes: &'a Changes,
}

impl<'a, T> TrackedMut<'a, T>
where
    T: 'static,
{
    fn new(value: &'a mut T, changes: &'a Changes) -> Self {
        Self { value, changes }
    }
    pub fn set_changed(&self) {
        self.changes.set_changed(ElementTypeId::of::<T>())
    }
}

impl<'a, T> core::fmt::Debug for TrackedRef<'a, T>
where
    T: core::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct(format!("TrackedRef<{}>", std::any::type_name::<T>()).as_str())
            .field("value", &self.value)
            .field(
                "changed",
                &self.changes.is_changed(ElementTypeId::of::<T>()),
            )
            .finish()
    }
}

impl<'a, T> core::fmt::Debug for TrackedMut<'a, T>
where
    T: core::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct(format!("TrackedMut<{}>", std::any::type_name::<T>()).as_str())
            .field("value", &self.value)
            .field(
                "changed",
                &self.changes.is_changed(ElementTypeId::of::<T>()),
            )
            .finish()
    }
}

impl<'a, T> Deref for TrackedRef<'a, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &*(self.value)
    }
}

impl<'a, T> Deref for TrackedMut<'a, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &*(self.value)
    }
}

impl<'a, T> DerefMut for TrackedMut<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.set_changed();
        &mut *(self.value)
    }
}

#[cfg(test)]
mod tests {
    use super::{AccessMode, Changes, ElementTypeId, Trackable};

    #[test]
    fn tracked_ref_metadata() {
        type QueryType<'a> = &'a u32;

        assert_eq!(QueryType::count_types(), 1);

        let mut all_types = vec![];
        QueryType::for_each_type(|t, m| all_types.push((t, m)));
        assert_eq!(
            all_types.as_slice(),
            &[(ElementTypeId::of::<u32>(), AccessMode::ReadOnly)]
        );
    }

    #[test]
    fn tracked_mut_metadata() {
        type QueryType<'a> = &'a mut u32;

        assert_eq!(QueryType::count_types(), 1);

        let mut all_types = vec![];
        QueryType::for_each_type(|t, m| all_types.push((t, m)));
        assert_eq!(
            all_types.as_slice(),
            &[(ElementTypeId::of::<u32>(), AccessMode::ReadWrite)]
        );
    }

    #[test]
    fn tracked_option_metadata() {
        type QueryType<'a> = Option<&'a u32>;

        assert_eq!(QueryType::count_types(), 1);

        let mut all_types = vec![];
        QueryType::for_each_type(|t, m| all_types.push((t, m)));
        assert_eq!(
            all_types.as_slice(),
            &[(ElementTypeId::of::<u32>(), AccessMode::ReadOnly)]
        );
    }

    #[test]
    fn tracked_ref() {
        let mut value = 0u32;
        let reference = &mut value;
        let changes = Changes::new_for(&reference);
        let mut tracked = reference.into_tracked(&changes);
        let mut changed_types = vec![];

        changes.for_each_changed(|t| changed_types.push(t));
        assert_eq!(changed_types.len(), 0);
        assert_eq!(*tracked, 0);
        assert_eq!(changed_types.len(), 0);

        *tracked = 1;
        changes.for_each_changed(|t| changed_types.push(t));
        assert_eq!(changed_types.len(), 1);
        assert_eq!(*tracked, 1);
        assert_eq!(
            changed_types.first(),
            Some(ElementTypeId::of::<u32>()).as_ref()
        );

        assert_eq!(value, 1);
    }

    #[test]
    fn tracked_option() {
        let mut value = 0u32;
        let reference = Some(&mut value);
        let changes = Changes::new_for(&reference);
        let mut tracked = reference.into_tracked(&changes);
        let mut changed_types = vec![];

        changes.for_each_changed(|t| changed_types.push(t));
        assert_eq!(changed_types.len(), 0);
        assert_eq!(tracked.as_deref().cloned(), Some(0));
        assert_eq!(changed_types.len(), 0);

        tracked.as_mut().map(|v| **v = 1);
        changes.for_each_changed(|t| changed_types.push(t));
        assert_eq!(changed_types.len(), 1);
        assert_eq!(tracked.as_deref().cloned(), Some(1));
        assert_eq!(
            changed_types.first(),
            Some(ElementTypeId::of::<u32>()).as_ref()
        );

        assert_eq!(value, 1);
    }
}
