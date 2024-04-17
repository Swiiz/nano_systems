use std::{any::type_name, hash::Hash};
use std::{
    any::{Any, TypeId},
    collections::HashMap,
    marker::PhantomData,
};

use dyn_clone::DynClone;
use parking_lot::{
    MappedRwLockReadGuard, MappedRwLockWriteGuard, RwLock, RwLockReadGuard, RwLockWriteGuard,
};

pub type GlobalEntryId = usize;

pub trait GlobalsExt {
    fn init_accessors() -> HashMap<TypeId, Box<dyn KeyAccessor>>;
}

/// The `Globals` struct contains all the data accessible from the systems
/// The data is stored using multiple keys per value
/// The keys and values can be of any type
/// However each key can only be backed by one type
pub struct Globals {
    accessors: HashMap<TypeId, Box<dyn KeyAccessor>>, // Key typeid to accessor
    entries: Vec<Option<RwLock<GlobalEntry>>>,
    free_entries: Vec<GlobalEntryId>,
}

pub fn map_read_guard<'a, T: Any>(guard: RwLockReadGuard<'a, GlobalEntry>) -> GlobalRef<'a, T> {
    RwLockReadGuard::map(guard, |e: &GlobalEntry| e.value.downcast_ref().unwrap())
}

pub fn map_write_guard<'a, T: Any>(guard: RwLockWriteGuard<'a, GlobalEntry>) -> GlobalMut<'a, T> {
    RwLockWriteGuard::map(guard, |e: &mut GlobalEntry| e.value.downcast_mut().unwrap())
}

pub type GlobalRef<'a, T> = MappedRwLockReadGuard<'a, T>;
pub type GlobalMut<'a, T> = MappedRwLockWriteGuard<'a, T>;

impl Globals {
    pub fn new() -> Self {
        let mut _self = Globals {
            accessors: HashMap::new(),
            entries: Vec::new(),
            free_entries: Vec::new(),
        };
        // extend Globals, adding singletons (1type=1key=1value)
        // needed by default
        _self.add_support_for::<SingletonGlobals>();
        _self
    }

    pub fn add_support_for<T: GlobalsExt>(&mut self) {
        for (k, v) in T::init_accessors() {
            self.accessors.entry(k).or_insert(v);
        }
    }

    pub fn define<C>(&mut self, new_entries: impl IntoGlobalEntries<C>) {
        let new_entries = new_entries.into_global_entries();

        for entry in new_entries {
            let need_resize = self.free_entries.is_empty();
            let id = if need_resize {
                self.entries.len()
            } else {
                self.free_entries.pop().unwrap()
            };

            for (tid, part) in &entry.key.parts {
                if let Some(redefined_id) = self
                    .accessors
                    .get_mut(tid)
                    .expect("Tried to define global with invalid key!")
                    .insert(dyn_clone::clone_box(&**part), id)
                {
                    let redefined = &mut self.entries[redefined_id].as_mut().unwrap().get_mut();
                    redefined.key.parts.remove(tid);
                    if redefined.key.parts.is_empty() {
                        let _ = &mut self.entries[redefined_id].take().unwrap();
                    }
                }
            }
            let new_entry = Some(RwLock::new(entry));
            if need_resize {
                self.entries.push(new_entry);
            } else {
                self.entries[id] = new_entry;
            }
        }
    }

    pub fn remove<T: IntoGlobalKey>(&mut self, key: T) -> Option<T::Value> {
        let id = self.id_of(key.into())?;
        let Some(entry) = self.entries[id].take() else {
            return None;
        };
        let entry = entry.into_inner();
        for (tid, part) in entry.key.parts {
            self.accessors.get_mut(&tid).unwrap().remove(part);
        }
        Some(*entry.value.downcast().unwrap())
    }

    pub fn id_of(&self, key: GlobalKey) -> Option<GlobalEntryId> {
        let mut id_found = None;
        for (tid, part) in key.parts {
            if let Some(id) = self
                .accessors
                .get(&tid)
                .expect("Tried to get id of global with invalid key!")
                .get(part)
            {
                id_found = Some(id);
                break;
            }
        }
        id_found
    }

    pub fn read_entry(&self, id: GlobalEntryId) -> Option<RwLockReadGuard<GlobalEntry>> {
        self.entries[id].as_ref()?.read().into()
    }

    pub fn write_entry(&self, id: GlobalEntryId) -> Option<RwLockWriteGuard<GlobalEntry>> {
        self.entries[id].as_ref()?.write().into()
    }

    pub fn get<T: IntoGlobalKey>(&self, key: T) -> Option<GlobalRef<T::Value>> {
        let id = self.id_of(key.into())?;
        Some(map_read_guard(self.entries[id].as_ref()?.read()))
    }

    pub fn get_mut<T: IntoGlobalKey>(&self, key: T) -> Option<GlobalMut<T::Value>> {
        let id = self.id_of(key.into())?;
        Some(map_write_guard(self.entries[id].as_ref()?.write()))
    }
}

pub trait IntoGlobalEntries<C = ()> {
    fn into_global_entries(self) -> Vec<GlobalEntry>;
}
impl<C: IntoGlobalEntries<C>, T: Into<Vec<C>>> IntoGlobalEntries<C> for T {
    fn into_global_entries(self) -> Vec<GlobalEntry> {
        self.into()
            .into_iter()
            .map(|s| s.into_global_entries())
            .flatten()
            .collect()
    }
}

pub struct GlobalEntry {
    pub key: GlobalKey,
    pub value: Box<dyn Any + Send + Sync>,
}

pub trait AnyKey: DynClone + Any + Send + Sync {
    fn anykey_type_id(&self) -> TypeId;
    fn boxed_any(self: Box<Self>) -> Box<dyn Any>;
}
dyn_clone::clone_trait_object!(AnyKey);
impl<T: DynClone + Any + Send + Sync> AnyKey for T {
    fn anykey_type_id(&self) -> TypeId {
        TypeId::of::<T>()
    }
    fn boxed_any(self: Box<Self>) -> Box<dyn Any> {
        Box::new(*self)
    }
}

pub struct GlobalKey {
    pub parts: HashMap<TypeId, Box<dyn AnyKey>>,
}

pub trait IntoGlobalKey {
    type Value: 'static;
    fn into(self) -> GlobalKey;
}

pub trait KeyAccessor: Send + Sync {
    fn insert(&mut self, key: Box<dyn AnyKey>, id: GlobalEntryId) -> Option<usize>;
    fn remove(&mut self, key: Box<dyn AnyKey>);
    fn get(&self, key: Box<dyn AnyKey>) -> Option<usize>;
}

fn k<K: Eq + Hash + AnyKey>(k: Box<dyn AnyKey>) -> K {
    if let Ok(k) = k.boxed_any().downcast() {
        *k
    } else {
        panic!(
            "Could not downcast key to appropriate accessor type: {} !",
            type_name::<K>()
        )
    }
}
impl<K: Eq + Hash + AnyKey + Send + Sync> KeyAccessor for HashMap<K, GlobalEntryId> {
    fn insert(&mut self, key: Box<dyn AnyKey>, id: GlobalEntryId) -> Option<usize> {
        self.insert(k(key), id)
    }

    fn remove(&mut self, key: Box<dyn AnyKey>) {
        self.remove(&k(key));
    }

    fn get(&self, key: Box<dyn AnyKey>) -> Option<usize> {
        self.get(&k(key)).copied()
    }
}

////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

/// Singleton<T> Globals

pub struct SingletonGlobals;
impl GlobalsExt for SingletonGlobals {
    fn init_accessors() -> HashMap<TypeId, Box<dyn KeyAccessor>> {
        let mut a = HashMap::new();
        let boxed: Box<dyn KeyAccessor> = Box::new(HashMap::<TypeId, GlobalEntryId>::new());
        a.insert(TypeId::of::<TypeId>(), boxed);
        a
    }
}

pub struct Singleton<T: 'static>(pub T);
pub struct SingletonKey<T: 'static>(PhantomData<T>);
impl<T: 'static> Singleton<T> {
    pub const fn key() -> SingletonKey<T> {
        SingletonKey(PhantomData)
    }
}
pub trait IntoSingletonKey: Sized + 'static {
    const SINGLETON: SingletonKey<Self>;
}
impl<T: Sized + 'static> IntoSingletonKey for T {
    const SINGLETON: SingletonKey<Self> = Singleton::<T>::key();
}

impl<T: 'static + Send + Sync> IntoGlobalEntries for Singleton<T> {
    fn into_global_entries(self) -> Vec<GlobalEntry> {
        vec![GlobalEntry {
            key: <SingletonKey<T> as IntoGlobalKey>::into(Self::key()),
            value: Box::new(self.0),
        }]
    }
}
impl<T> IntoGlobalKey for SingletonKey<T> {
    type Value = T;
    fn into(self) -> GlobalKey {
        GlobalKey {
            parts: {
                let arr: [(TypeId, Box<dyn AnyKey>); 1] =
                    [(TypeId::of::<TypeId>(), Box::new(TypeId::of::<T>()))];
                arr.into_iter().collect()
            },
        }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
