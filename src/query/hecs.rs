use super::{
    tracked::{AccessMode, Changes, Trackable},
    type_info::TypeInfo,
};

use hecs::Or;

impl<'a, L, R> Trackable<'a> for Or<L, R>
where
    L: Trackable<'a>,
    R: Trackable<'a>,
{
    type Tracked = Or<L::Tracked, R::Tracked>;

    fn count_types() -> usize {
        L::count_types() + R::count_types()
    }

    fn for_each_type(mut f: impl FnMut(TypeInfo, AccessMode)) {
        L::for_each_type(|t, m| f(t, m));
        R::for_each_type(|t, m| f(t, m));
    }

    fn to_tracked(self, changes: &'a Changes) -> Self::Tracked {
        match self {
            Or::Left(l) => Or::Left(l.to_tracked(changes)),
            Or::Right(r) => Or::Right(r.to_tracked(changes)),
            Or::Both(l, r) => Or::Both(l.to_tracked(changes), r.to_tracked(changes)),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::query::{AccessMode, Changes, Trackable, TypeInfo};
    use hecs::Or;

    #[test]
    fn tracked_or_metadata() {
        type QueryType<'a> = Or<&'a mut u32, &'a f32>;

        assert_eq!(QueryType::count_types(), 2);

        let mut all_types = vec![];
        QueryType::for_each_type(|t, m| all_types.push((t, m)));
        assert_eq!(all_types.len(), 2);
        assert!(all_types.contains(&(TypeInfo::of::<u32>(), AccessMode::ReadWrite)));
        assert!(all_types.contains(&(TypeInfo::of::<f32>(), AccessMode::ReadOnly)));
    }

    #[test]
    fn tracked_or_left() {
        type QueryType<'a> = Or<&'a mut u32, &'a mut f32>;

        let mut value = 0u32;
        let or_value: QueryType = Or::new(Some(&mut value), None).unwrap();

        let changes = Changes::new_for(&or_value);
        let mut tracked = or_value.to_tracked(&changes);

        tracked
            .as_ref()
            .map(|l| assert_eq!(**l, 0), |_| unreachable!());
        let mut changed_types = vec![];
        changes.for_each_changed(|t| changed_types.push(t));
        assert!(changed_types.is_empty());

        tracked.as_ref().right().map(|_| unreachable!());
        assert!(changed_types.is_empty());

        tracked.as_mut().left().map(|l| **l = 1);
        tracked
            .as_ref()
            .map(|l| assert_eq!(**l, 1), |r| assert_eq!(**r, 0.0));
        changes.for_each_changed(|t| changed_types.push(t));
        assert_eq!(changed_types.as_slice(), &[TypeInfo::of::<u32>()]);
    }

    #[test]
    fn tracked_or_right() {
        type QueryType<'a> = Or<&'a mut u32, &'a mut f32>;

        let mut value = 0f32;
        let or_value: QueryType = Or::new(None, Some(&mut value)).unwrap();

        let changes = Changes::new_for(&or_value);
        let mut tracked = or_value.to_tracked(&changes);

        tracked
            .as_ref()
            .map(|_| unreachable!(), |l| assert_eq!(**l, 0.0));
        let mut changed_types = vec![];
        changes.for_each_changed(|t| changed_types.push(t));
        assert!(changed_types.is_empty());

        tracked.as_ref().left().map(|_| unreachable!());
        assert!(changed_types.is_empty());

        tracked.as_mut().right().map(|l| **l = 1.0);
        tracked
            .as_ref()
            .map(|l| assert_eq!(**l, 0), |r| assert_eq!(**r, 1.0));
        changes.for_each_changed(|t| changed_types.push(t));
        assert_eq!(changed_types.as_slice(), &[TypeInfo::of::<f32>()]);
    }

    #[test]
    fn tracked_or_both() {
        type QueryType<'a> = Or<&'a mut u32, &'a mut f32>;

        let mut left = 0u32;
        let mut right = 0f32;
        let or_value: QueryType = Or::new(Some(&mut left), Some(&mut right)).unwrap();

        let changes = Changes::new_for(&or_value);
        let mut tracked = or_value.to_tracked(&changes);

        tracked
            .as_ref()
            .map(|l| assert_eq!(**l, 0), |r| assert_eq!(**r, 0.0));
        let mut changed_types = vec![];
        changes.for_each_changed(|t| changed_types.push(t));
        assert!(changed_types.is_empty());

        changed_types.clear();
        tracked.as_mut().left().map(|l| **l = 1);
        tracked
            .as_ref()
            .map(|l| assert_eq!(**l, 1), |r| assert_eq!(**r, 0.0));
        changes.for_each_changed(|t| changed_types.push(t));
        assert_eq!(changed_types.as_slice(), &[TypeInfo::of::<u32>()]);

        changed_types.clear();
        tracked.as_mut().right().map(|r| **r = 2.0);
        tracked
            .as_ref()
            .map(|l| assert_eq!(**l, 1), |r| assert_eq!(**r, 2.0));
        changes.for_each_changed(|t| changed_types.push(t));
        let expected_changed_types = &mut [TypeInfo::of::<u32>(), TypeInfo::of::<f32>()];
        changed_types.sort();
        expected_changed_types.sort();
        assert_eq!(changed_types.as_slice(), expected_changed_types,);
    }
}
