use crate::query::{AccessMode, Changes, ElementTypeId, Trackable};
use hecs::{Entity, Query, QueryBorrow, QueryItem, QueryIter};
use std::iter::{IntoIterator, Iterator};

impl<'w, Q> Trackable<'w> for QueryBorrow<'w, Q>
where
    Q: Query,
    QueryItem<'w, Q>: Trackable<'w>,
{
    type Tracked = TrackedQueryBorrow<'w, Q>;

    fn count_types() -> usize {
        <QueryItem<'w, Q> as Trackable>::count_types()
    }

    fn for_each_type(f: impl FnMut(ElementTypeId, AccessMode)) {
        <QueryItem<'w, Q> as Trackable>::for_each_type(f)
    }

    fn into_tracked(self, changes: &'w Changes) -> Self::Tracked {
        TrackedQueryBorrow::new(self, changes)
    }
}

pub struct TrackedQueryBorrow<'w, Q>
where
    Q: Query,
    QueryItem<'w, Q>: Trackable<'w>,
{
    inner: QueryBorrow<'w, Q>,
    changes: &'w Changes,
}

impl<'w, Q> TrackedQueryBorrow<'w, Q>
where
    Q: Query,
    QueryItem<'w, Q>: Trackable<'w>,
{
    fn new(inner: QueryBorrow<'w, Q>, changes: &'w Changes) -> Self {
        Self {
            inner,
            changes,
        }
    }

    // The lifetime narrowing here is required for soundness.
    pub fn iter(&mut self) -> TrackedQueryIter<'_, Q> {
        let iter = self.inner.iter();
        TrackedQueryIter::new(iter, self.changes)
    }
}

impl<'q, Q> IntoIterator for &'q mut TrackedQueryBorrow<'q, Q>
where
    Q: Query,
    QueryItem<'q, Q>: Trackable<'q>,
{
    type IntoIter = TrackedQueryIter<'q, Q>;
    type Item = (Entity, <QueryItem<'q, Q> as Trackable<'q>>::Tracked);

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

pub struct TrackedQueryIter<'q, Q>
where
    Q: Query,
{
    inner: QueryIter<'q, Q>,
    changes: &'q Changes,
}

impl<'q, Q> TrackedQueryIter<'q, Q>
where
    Q: Query,
{
    fn new(inner: QueryIter<'q, Q>, changes: &'q Changes) -> Self {
        Self {
            inner,
            changes,
        }
    }
}

impl<'q, Q> Iterator for TrackedQueryIter<'q, Q>
where
    Q: Query,
    QueryItem<'q, Q>: Trackable<'q>,
{
    type Item = (Entity, <QueryItem<'q, Q> as Trackable<'q>>::Tracked);
    fn next(&mut self) -> Option<Self::Item> {
        self.inner
            .next()
            .map(|(entity, components)| (entity, components.into_tracked(self.changes)))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl<'q, Q> ExactSizeIterator for TrackedQueryIter<'q, Q>
where
    Q: Query,
    QueryItem<'q, Q>: Trackable<'q>,
{
    fn len(&self) -> usize {
        self.inner.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hecs::*;

    #[test]
    fn tracked_query() {
        fn nullify_ten_plus(world: &mut World, changes: &Changes) {
            world
                .query::<(&mut u32, &i32)>()
                .into_tracked(&changes)
                .iter()
                .for_each(|(_, (mut v, _))| {
                    if *v >= 10 {
                        *v = 0;
                    }
                });
        }

        let mut world = World::default();
        let changes = Changes::new::<(&u32, &i32, &String)>();

        world.spawn((0u32, 0i32, "hello".to_string()));
        world.spawn((1u32, 1i32, "hello".to_string()));
        nullify_ten_plus(&mut world, &changes);

        let mut changed_types = vec![];
        changes.for_each_changed(|t| changed_types.push(t));
        assert!(changed_types.is_empty());

        world.spawn((10u32, 10i32, "hello".to_string()));
        world.spawn((11u32, 11i32, "hello".to_string()));
        nullify_ten_plus(&mut world, &changes);

        let mut changed_types = vec![];
        changes.for_each_changed(|t| changed_types.push(t));
        assert_eq!(changed_types.as_slice(), &[ElementTypeId::of::<u32>()]);
    }
}
