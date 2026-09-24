use super::*;

#[derive(Debug, Serialize, Deserialize)]
pub struct AssetId<T> {
    pub index: u32,
    pub generation: u32,
    #[serde(skip)]
    marker: PhantomData<fn() -> T>,
}

impl<T> Copy for AssetId<T> {}
impl<T> Clone for AssetId<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> PartialEq for AssetId<T> {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index && self.generation == other.generation
    }
}
impl<T> Eq for AssetId<T> {}
impl<T> std::hash::Hash for AssetId<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.index.hash(state);
        self.generation.hash(state);
    }
}

impl<T> AssetId<T> {
    pub const fn new(index: u32, generation: u32) -> Self {
        Self {
            index,
            generation,
            marker: PhantomData,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Handle<T> {
    pub id: AssetId<T>,
}

impl<T> Copy for Handle<T> {}
impl<T> Clone for Handle<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> PartialEq for Handle<T> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}
impl<T> Eq for Handle<T> {}
impl<T> std::hash::Hash for Handle<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl<T> Handle<T> {
    pub const fn new(id: AssetId<T>) -> Self {
        Self { id }
    }
}
