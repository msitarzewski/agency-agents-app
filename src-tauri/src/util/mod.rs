//! Cross-cutting helpers shared by multiple `commands::*` modules.
//!
//! The current contents are filesystem helpers (`fs`) used by the
//! catalog (Phase 12a) and re-used by upcoming sub-phases:
//!
//! - 12c (`github-cache/*.json`)
//! - 12d (`settings.json`)
//!
//! Keeping them here (rather than co-located with the first caller) means
//! the security-critical write/read paths have a single canonical
//! implementation that the security review can pin tests against once.

pub mod fs;
pub mod net;
pub mod proc;
