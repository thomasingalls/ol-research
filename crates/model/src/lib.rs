//! Canonical types for the normalized layer.
//!
//! Only what is committed lives here (plan §4): each source's native
//! identifier, with the exact rule it enforces. Everything else about the
//! normalized shape is discovered from data, not declared ahead of it.

pub mod claim;
pub mod date;
pub mod doi;
pub mod id;
pub mod lang;
pub mod mesh;

pub use claim::Claim;
pub use date::{
    Date, Day, Error as DateError, MedlineError as MedlineDateError, Month, PartialDate, Quarter,
    Season, Year,
};
pub use doi::{Doi, Error as DoiError};
pub use id::{OpenAlexId, ParseError, PmcId, Pmid};
pub use lang::{Error as LangError, Lang};
pub use mesh::{Error as MeshUiError, MeshUi};
