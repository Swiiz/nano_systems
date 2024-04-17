/// Allow to easily access globals.
/// TODO: It also sorts globals by their id
/// TODO: when locking them to avoid deadlocks.
/// Syntax example:
/// ```
/// access! { globals |
///    &user: User::SINGLETON,
///    &mut language: Language::SINGLETON
/// };
/// ```
#[macro_export]
macro_rules! access {
    (@ids $refids:ident $mutids:ident $g:ident | , & $name:ident : $k:expr , $($tail:tt)*) => {
        let k = $k;
        let Some($name) = $g.id_of($crate::globals::IntoGlobalKey::into(k)) else { return; };
        let id = $name;
        $refids.push(id);
        $crate::access!(@ids $refids $mutids $g | , $($tail)*)
    };
    (@ids $refids:ident $mutids:ident $g:ident |, &mut $name:ident : $k:expr , $($tail:tt)*) => {
        let k = $k;
        let Some($name) = $g.id_of($crate::globals::IntoGlobalKey::into(k)) else { return; };
        $mutids.push($name);
        $crate::access!(@ids $refids $mutids $g | , $($tail)*)
    };
    (@ids $refids:ident $mutids:ident $g:ident | , ,) => { $crate::access!(@drop $g)};
    (@ids $refids:ident $mutids:ident $g:ident | ,) => { $crate::access!(@drop $g)};

    (@guards $refguards:ident $mutguards:ident $g:ident | , & $name:ident : $k:expr , $($tail:tt)*) => {
        let $name = $crate::macros::extract_v_ty_ref($k, $crate::globals::map_read_guard($refguards.remove(&$name).unwrap()));
        $crate::access!(@guards $refguards $mutguards $g | , $($tail)*)
    };
    (@guards $refguards:ident $mutguards:ident $g:ident |, &mut $name:ident : $k:expr , $($tail:tt)*) => {
        let mut $name = $crate::macros::extract_v_ty_mut($k, $crate::globals::map_write_guard($mutguards.remove(&$name).unwrap()));
        $crate::access!(@guards $refguards $mutguards $g | , $($tail)*)
    };
    (@guards $refguards:ident $mutguards:ident $g:ident | , ,) => { $crate::access!(@drop $g)};
    (@guards $refguards:ident $mutguards:ident $g:ident | ,) => { $crate::access!(@drop $g)};

    (@drop $g:ident) => {drop($g);};
    ($g:ident | $($tail:tt)*) => {
        let GlobalAccess { inner_may_deadlock } = $g;
        let mut refids = Vec::new();
        let mut mutids = Vec::new();
        $crate::access!(@ids refids mutids inner_may_deadlock | , $($tail)* , );
        refids.sort(); mutids.sort();

        let mut refguards: std::collections::HashMap<_, _> =
            refids.into_iter().map(|id| (id, inner_may_deadlock.read_entry(id).unwrap())).collect();
        let mut mutguards: std::collections::HashMap<_, _> =
            mutids.into_iter().map(|id| (id, inner_may_deadlock.write_entry(id).unwrap())).collect();

        $crate::access!(@guards refguards mutguards inner_may_deadlock | , $($tail)* , );

        // let [a, b]  = ids; // a and b are ids
        // ids.sort();
        // let guards: HashMap<GlobalId, _> = ids.map(|id| (id, g.#read/write#_value(id))).collect();
        // let [a, b] = (map_#read/write#_guard::<A>(guards[a]), map_#read/write#_guard::<B>(guards[b]));


    };
}

pub fn extract_v_ty_ref<'a, T: crate::globals::IntoGlobalKey>(
    _k: T,
    v: crate::globals::GlobalRef<'a, T::Value>,
) -> crate::globals::GlobalRef<'a, T::Value> {
    v
}

pub fn extract_v_ty_mut<'a, T: crate::globals::IntoGlobalKey>(
    _k: T,
    v: crate::globals::GlobalMut<'a, T::Value>,
) -> crate::globals::GlobalMut<'a, T::Value> {
    v
}
