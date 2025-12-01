#![no_std]

extern crate alloc;

// Add wee_alloc for wasm32 targets
#[cfg(target_arch = "wasm32")]
extern crate wee_alloc;

#[cfg(target_arch = "wasm32")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

mod contract;
mod storage;
#[cfg(test)]
mod test;
mod transact;
mod treasury;
