#![no_std]

extern crate alloc;

// Add wee_alloc for wasm32 targets
#[cfg(target_arch = "wasm32")]
extern crate wee_alloc;

#[cfg(target_arch = "wasm32")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

pub mod contract;
#[cfg(test)]
pub mod tests;
