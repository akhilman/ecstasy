use core::any::{type_name, TypeId};

#[derive(Debug, Copy, Clone, PartialEq, PartialOrd, Eq, Ord)]
pub struct TypeInfo {
    id: TypeId,
    #[cfg(debug_assertions)]
    name: &'static str,
}

impl TypeInfo {
    pub fn of<T>() -> Self
    where
        T: 'static,
    {
        Self {
            id: TypeId::of::<T>(),
            name: type_name::<T>(),
        }
    }

    pub fn id(&self) -> TypeId {
        self.id
    }

    #[cfg(debug_assertions)]
    pub fn name(&self) -> &'static str {
        self.name
    }
}

impl core::fmt::Display for TypeInfo {
    #[cfg(debug_assertions)]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.name)
    }
    #[cfg(not(debug_assertions))]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!("type_id:{}", self.id)
    }
}

impl core::hash::Hash for TypeInfo {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}
