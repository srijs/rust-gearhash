#[cfg(target_arch = "x86_64")]
pub use crate::simd::avx2;
#[cfg(target_arch = "aarch64")]
pub use crate::simd::neon;
#[cfg(target_arch = "x86_64")]
pub use crate::simd::sse42;
