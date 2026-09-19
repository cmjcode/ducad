//! Checks: desain berbasis spesifikasi (P7). Persyaratan user ditulis
//! sebagai `Check`, dievaluasi terhadap geometri sesi.

pub mod holes;
pub mod run;
pub mod types;

pub use holes::{find_holes, Hole};
pub use run::{all_pass, run_checks, run_checks_on, CheckSummary};
pub use types::{BodySel, Check, CheckItem, CheckResult, CheckStatus};
