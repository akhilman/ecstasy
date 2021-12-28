use super::{
    tracked::{AccessMode, Changes, Trackable},
    type_id::ElementTypeId,
};

macro_rules! tracked_tuple_impl {
    ($($name: ident), *) => {
        impl<'a, $($name),*> Trackable<'a> for ($($name,)*)
        where
            $(
                $name: Trackable<'a>,
            )*
        {
            type Tracked = (
                $(
                    <$name as Trackable<'a>>::Tracked,
                )*
            );


            #[allow(unused_mut)]
            fn count_types() -> usize {
                let mut count = 0;
                $(count += <$name as Trackable<'a>>::count_types();)*
                count

            }

            #[allow(unused_variables, unused_mut)]
            fn for_each_type(mut f: impl FnMut(ElementTypeId, AccessMode)) {
                $(
                    <$name as Trackable<'a>>::for_each_type(|t, m| f(t,m));
                )*
            }

            #[allow(unused_variables)]
            fn to_tracked(self, changes: &'a Changes) -> Self::Tracked {
                #[allow(non_snake_case)]
                let ($($name,)*) = self;
                (
                    $(
                        $name.to_tracked(changes),
                    )*
                )
            }
        }
    }
}

// smaller_tuples_too!(tracked_tuple_impl, B, A);

#[rustfmt::skip]
smaller_tuples_too!(tracked_tuple_impl, O, N, M, L, K, J, I, H, G, F, E, D, C, B, A);

#[cfg(test)]
mod tests {
    use crate::query::{AccessMode, Changes, ElementTypeId, Trackable};

    #[test]
    fn tracked_tuple_metadata() {
        type QueryType<'a> = (Option<&'a mut bool>, &'a mut u32, (&'a f32, &'a f64));

        assert_eq!(QueryType::count_types(), 4);

        let mut all_types = vec![];
        QueryType::for_each_type(|t, m| all_types.push((t, m)));
        all_types.sort();
        let mut expected_types = [
            (ElementTypeId::of::<bool>(), AccessMode::ReadWrite),
            (ElementTypeId::of::<u32>(), AccessMode::ReadWrite),
            (ElementTypeId::of::<f32>(), AccessMode::ReadOnly),
            (ElementTypeId::of::<f64>(), AccessMode::ReadOnly),
        ];
        expected_types.sort();
        assert_eq!(all_types.as_slice(), &expected_types);
    }

    #[test]
    fn tracked_tuple() {
        let mut value = (Some(false), 0u32);
        let reference = (value.0.as_mut(), &mut value.1);

        let changes = Changes::new_for(&reference);
        let tracked = reference.to_tracked(&changes);

        let (mut a, mut b) = tracked;
        a.as_ref()
            .map_or_else(|| unreachable!("a is None"), |a| assert_eq!(**a, false));
        assert_eq!(*b, 0);

        let mut changed_types = vec![];
        changes.for_each_changed(|t| changed_types.push(t));
        assert!(changed_types.is_empty());

        *b = 1;
        changes.for_each_changed(|t| changed_types.push(t));
        assert_eq!(changed_types.as_slice(), &[ElementTypeId::of::<u32>()]);

        a.as_mut().map(|a| **a = true);
        let mut changed_types = vec![];
        changes.for_each_changed(|t| changed_types.push(t));
        let expected_changed_types = &mut [ElementTypeId::of::<u32>(), ElementTypeId::of::<bool>()];
        changed_types.sort();
        expected_changed_types.sort();
        assert_eq!(changed_types.as_slice(), expected_changed_types);

        assert_eq!(value, (Some(true), 1u32));
    }
}
