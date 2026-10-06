// Spec: specs/sim/intents-events.md
//! Little-endian field reads and writes for [`super::ServerMsg`] layouts.

pub trait WireField: Sized {
    /// Bytes on the wire.
    const LEN: usize;
    fn read(b: &[u8], off: usize) -> Self;
    fn write(&self, out: &mut [u8], off: usize);
}

macro_rules! int_field {
    ($($t:ty),*) => {$(
        impl WireField for $t {
            const LEN: usize = std::mem::size_of::<$t>();
            fn read(b: &[u8], off: usize) -> Self {
                let mut a = [0; std::mem::size_of::<$t>()];
                a.copy_from_slice(&b[off..off + Self::LEN]);
                <$t>::from_le_bytes(a)
            }
            fn write(&self, out: &mut [u8], off: usize) {
                out[off..off + Self::LEN].copy_from_slice(&self.to_le_bytes());
            }
        }
    )*};
}
int_field!(u8, u16, i16, u32);

impl<T: WireField + Copy + Default, const N: usize> WireField for [T; N] {
    const LEN: usize = T::LEN * N;
    fn read(b: &[u8], off: usize) -> Self {
        let mut a = [T::default(); N];
        for (i, v) in a.iter_mut().enumerate() {
            *v = T::read(b, off + i * T::LEN);
        }
        a
    }
    fn write(&self, out: &mut [u8], off: usize) {
        for (i, v) in self.iter().enumerate() {
            v.write(out, off + i * T::LEN);
        }
    }
}
