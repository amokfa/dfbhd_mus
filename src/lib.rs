use std::mem::size_of;

pub unsafe fn mm<T: ?Sized>(r : &T) -> &mut T {
    #[allow(mutable_transmutes)]
    std::mem::transmute::<_, &mut T>(r)
}

pub unsafe fn ms<T: ?Sized>(r : &T) -> &'static T {
    std::mem::transmute::<_, &'static T>(r)
}

pub unsafe fn msm<T: ?Sized>(r : &mut T) -> &'static mut T {
    std::mem::transmute::<_, &'static mut T>(r)
}

pub fn set<T>(dst: &mut T, val: T) {
    use std::mem::{forget, replace};
    forget(replace(dst, val));
}

pub fn array_transmute<T, U>(data: &[T]) -> &[U] {
    let bytes_count = data.len() * size_of::<T>();
    assert_eq!(bytes_count % size_of::<U>(), 0);
    unsafe { std::slice::from_raw_parts(data.as_ptr() as *const U, bytes_count / size_of::<U>()) }
}
