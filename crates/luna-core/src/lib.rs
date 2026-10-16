//! A run: scan → classify → clip assembly → identity → cards → worker pool →
//! report model → atomic output. Settings load/save. No UI, no decoding.

pub mod assemble;
pub mod identity;
pub mod run;
pub mod scan;
pub mod select;
pub mod settings;

#[cfg(test)]
mod test_support;
