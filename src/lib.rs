pub mod cmd;
pub mod sbf;

use std::mem::size_of;

pub fn mm<T: ?Sized>(r : &T) -> &mut T {
    unsafe {
        #[allow(mutable_transmutes)]
        std::mem::transmute::<_, &mut T>(r)
    }
}

pub fn array_transmute<T, U>(data: &[T]) -> &[U] {
    let bytes_count = data.len() * size_of::<T>();
    assert_eq!(bytes_count % size_of::<U>(), 0);
    log("ding");
    let result = unsafe { std::slice::from_raw_parts(data.as_ptr() as *const U, bytes_count / size_of::<U>()) };
    log("dong");
    result
}

pub fn log(s: impl Into<String>) {
    let _ = std::fs::write("/dev/ttys011", format!("{}\n", s.into()));
}

