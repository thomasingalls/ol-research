//! Canonical types for the normalized layer.
//!
//! Only what is committed lives here (plan §4): each source's native
//! identifier, with the exact rule it enforces. Everything else about the
//! normalized shape is discovered from data, not declared ahead of it.

pub mod claim;
pub mod code;
pub mod date;
pub mod doi;
pub mod id;
pub mod isbn;
pub mod issn;
pub mod lang;
pub mod mesh;
pub mod orcid;
pub mod ror;

pub use claim::Claim;
pub use code::{CountryCode, CurrencyCode, Error as CodeError};
pub use date::{
    Date, DateTime, Day, Error as DateError, MedlineError as MedlineDateError, Month, PartialDate,
    Quarter, Season, TimeOfDay, Year,
};
pub use doi::{Doi, Error as DoiError, Registrant};
pub use id::{OpenAlexId, ParseError, PmcId, Pmid};
pub use isbn::{Error as IsbnError, Isbn};
pub use issn::{Error as IssnError, Issn};
pub use lang::{Error as LangError, Lang};
pub use mesh::{Error as MeshUiError, MeshUi};
pub use orcid::{Error as OrcidError, Orcid};
pub use ror::{Error as RorError, Ror};
