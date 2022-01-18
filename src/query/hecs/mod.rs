use super::Trackable;
use hecs::{Query, QueryItem};

pub trait TrackableQuery<'a>
where
    Self: Query + Trackable<'a>,
    QueryItem<'a, Self>: Trackable<'a>,
{
}

impl<'a, Q> TrackableQuery<'a> for Q
where
    Q: Query + Trackable<'a>,
    QueryItem<'a, Q>: Trackable<'a>,
{
}

mod or;
mod query;

// pub use query::{TrackedQueryBorrow, TrackedQueryIter};
