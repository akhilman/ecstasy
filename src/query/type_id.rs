use core::any;

#[derive(Debug, Copy, Clone, PartialEq, PartialOrd, Eq, Ord)]
pub struct ElementTypeId {
    id: any::TypeId,
    #[cfg(debug_assertions)]
    name: &'static str,
}

impl ElementTypeId {
    pub fn of<T>() -> Self
    where
        T: 'static,
    {
        Self {
            id: any::TypeId::of::<T>(),
            name: any::type_name::<T>(),
        }
    }

    pub fn id(&self) -> any::TypeId {
        self.id
    }

    #[cfg(debug_assertions)]
    pub fn name(&self) -> &'static str {
        self.name
    }
}

impl core::fmt::Display for ElementTypeId {
    #[cfg(debug_assertions)]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.name)
    }
    #[cfg(not(debug_assertions))]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!("type_id:{:#x}", self.id)
    }
}

impl core::hash::Hash for ElementTypeId {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}
