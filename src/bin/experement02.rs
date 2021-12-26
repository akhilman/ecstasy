use hecs::*;
use std::{
    any::{type_name, TypeId},
    collections::BTreeSet,
    marker::PhantomData,
    ops::{Deref, DerefMut},
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
};

/// Imagine macro parameters, but more like those Russian dolls.
///
/// Calls m!(A, B, C), m!(A, B), m!(B), and m!() for i.e. (m, A, B, C)
/// where m is any macro, for any number of parameters.
macro_rules! smaller_tuples_too {
    ($m: ident, $ty: ident) => {
        $m!{}
        $m!{$ty}
    };
    ($m: ident, $ty: ident, $($tt: ident),*) => {
        smaller_tuples_too!{$m, $($tt),*}
        $m!{$ty, $($tt),*}
    };
}

struct TrackedQueryBorrow<'w, Q, F>
where
    Q: Query + ToTracked<F>,
    F: Fn(TypeId) + Clone + Send + Sync,
{
    query_borrow: QueryBorrow<'w, Q>,
    on_change: F,
}

impl<'w, Q, F> TrackedQueryBorrow<'w, Q, F>
where
    Q: Query + ToTracked<F>,
    F: Fn(TypeId) + Clone + Send + Sync,
{
    fn new(query_borrow: QueryBorrow<'w, Q>, on_change: F) -> Self {
        Self {
            query_borrow,
            on_change,
        }
    }

    fn iter(&mut self) -> TrackedQueryIter<'_, Q, F> {
        TrackedQueryIter::new(self.query_borrow.iter(), &self.on_change)
    }
}

impl<'w, Q, F> ToTracked<F> for QueryBorrow<'w, Q>
where
    Q: Query+ToTracked<F>,
    F: Fn(TypeId) + Clone + Send + Sync,
{
    type Target = TrackedQueryBorrow<'w, Q, F>;
    fn to_tracked(self, on_change: F) -> Self::Target {
        TrackedQueryBorrow::new(self, on_change)
    }
}

struct TrackedQueryIter<'q, Q, F>
where
    Q: Query + ToTracked<F>,
    F: Fn(TypeId) + Clone + Send + Sync,
{
    iter: QueryIter<'q, Q>,
    on_change: &'q F,
}

impl<'q, Q, F> TrackedQueryIter<'q, Q, F>
where
    Q: Query + ToTracked<F>,
    F: Fn(TypeId) + Clone + Send + Sync,
{
    fn new(iter: QueryIter<'q, Q>, on_change: &'q F) -> Self {
        Self { iter, on_change }
    }
}

impl<'q, Q, F> std::iter::Iterator for TrackedQueryIter<'q, Q, F>
where
    Q: Query + ToTracked<F>,
    QueryItem<'q, Q>: ToTracked<F>,
    F: Fn(TypeId) + Clone + Send + Sync,
{
    type Item = (Entity, <QueryItem<'q, Q> as ToTracked<F>>::Target);
    fn next(&mut self) -> Option<Self::Item> {
        self.iter
            .next()
            .map(|(e, q)| (e, q.to_tracked(self.on_change.clone())))
    }
}

trait ComponentType {
    type Component: Component;
}

impl<'q, C: Component> ComponentType for &'q C {
    type Component = C;
}

impl<'q, C: Component> ComponentType for &'q mut C {
    type Component = C;
}

struct Tracked<T, F>
where
    T: Query + ComponentType,
    F: Fn(TypeId) + Send + Sync,
{
    value: T,
    on_change: F,
}

impl<T,F> std::fmt::Debug for Tracked<T, F>
where
    T: Query + ComponentType + std::fmt::Debug,
    F: Fn(TypeId) + Send + Sync,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Tracked")
            .field("value", &self.value)
            .field("on_change", &type_name::<F>())
            .finish()
    }
}

impl<T, F> Tracked<T, F>
where
    T: Query + ComponentType,
    F: Fn(TypeId) + Send + Sync,
{
    fn new(value: T, on_change: F) -> Self {
        Self { value, on_change }
    }

    fn set_changed(&self) {
        (self.on_change)(TypeId::of::<<T as ComponentType>::Component>());
    }
}

impl<C, F> Deref for Tracked<&C, F>
where
    C: Component,
    F: Fn(TypeId) + Send + Sync,
{
    type Target = C;
    fn deref(&self) -> &Self::Target {
        &*self.value
    }
}

impl<C, F> Deref for Tracked<&mut C, F>
where
    C: Component,
    F: Fn(TypeId) + Send + Sync,
{
    type Target = C;
    fn deref(&self) -> &Self::Target {
        &*self.value
    }
}

impl<C, F> DerefMut for Tracked<&mut C, F>
where
    C: Component,
    F: Fn(TypeId) + Send + Sync,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.set_changed();
        &mut *self.value
    }
}

trait ToTracked<F>
where
    F: Fn(TypeId) + Send + Sync,
{
    type Target;
    fn to_tracked(self, on_change: F) -> Self::Target;
}

impl<'q, F, C> ToTracked<F> for &'q C
where
    F: Fn(TypeId) + Send + Sync,
    C: Component,
{
    type Target = Tracked<&'q C, F>;
    fn to_tracked(self, on_change: F) -> Self::Target {
        Tracked::new(self, on_change)
    }
}
impl<'q, F, C> ToTracked<F> for &'q mut C
where
    F: Fn(TypeId) + Send + Sync,
    C: Component,
{
    type Target = Tracked<&'q mut C, F>;
    fn to_tracked(self, on_change: F) -> Self::Target {
        Tracked::new(self, on_change)
    }
}

impl<'q, F, C> ToTracked<F> for Option<&'q C>
where
    F: Fn(TypeId) + Send + Sync,
    C: Component,
{
    type Target = Option<Tracked<&'q C, F>>;
    fn to_tracked(self, on_change: F) -> Self::Target {
        self.map(|value| Tracked::new(value, on_change))
    }
}
impl<'q, F, C> ToTracked<F> for Option<&'q mut C>
where
    F: Fn(TypeId) + Send + Sync,
    C: Component,
{
    type Target = Option<Tracked<&'q mut C, F>>;
    fn to_tracked(self, on_change: F) -> Self::Target {
        self.map(|value| Tracked::new(value, on_change))
    }
}

macro_rules! tracked_tuple_impl {
    ($($name: ident),*) => {
        impl<'q, FUNC, $($name),*> ToTracked<FUNC> for ($($name,)*)
        where
            FUNC: Fn(TypeId) + Clone + Send + Sync,
            $(
                $name: Query + ToTracked<FUNC>,
            )*
        {
            type Target = (
                $(
                    <$name as ToTracked<FUNC>>::Target,
                )*
            );
            #[allow(unused_variables)]
            fn to_tracked(self, on_change: FUNC) -> Self::Target {
                #[allow(non_snake_case)]
                let ($($name,)*) = self;
                (
                    $(
                        $name.to_tracked(on_change.clone()),
                    )*
                )
            }
        }
    };
}

smaller_tuples_too!(tracked_tuple_impl, B, A);
////smaller_tuples_too!(tuple_impl, O, N, M, L, K, J, I, H, G, F, E, D, C, B, A);

fn print_type<T: std::any::Any>() {
    println!("{:?} {:?}", type_name::<T>(), TypeId::of::<T>());
}

fn main() {
    let mut world = World::default();
    print_type::<i32>();
    print_type::<f32>();
    print_type::<String>();
    world.spawn((0i32, 0f32, "hello".to_string()));

    let changed = Mutex::new(vec![]);
    let on_change = |x| {
        println!("closure {:?}", x);
        // changed.store(true, Ordering::Relaxed);
        changed.lock().unwrap().push(x);
    };
    world
        .query::<(&f32, Option<&mut i32>)>()
        .to_tracked(on_change)
        //.into_iter()
        .iter()
        .for_each(|(_, ab)| {
            println!("{:?}", ab);
            // let changed = AtomicBool::new(false);
            let (ta, mut tb) = ab;
            if let Some(tb) = &mut tb {
                **tb = 2;
            }
            println!("{:?}", (&*ta, &tb.as_deref()));
            // println!("Changed: {:?}", changed.load(Ordering::Relaxed));
        });
    //let mut tracked_query = TrackedQueryBorrow::new(query, |x| println!("deref mut {:?}", x));
    println!("Changed: {:?}", changed.lock().unwrap());
}
