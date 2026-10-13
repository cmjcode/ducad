//! Checks: desain berbasis spesifikasi (P7). Persyaratan user ditulis
//! sebagai `Check`, dievaluasi terhadap geometri sesi.

pub mod dfm;
pub mod holes;
pub mod rules;
pub mod run;
pub mod types;

pub use holes::{find_holes, Hole};
pub use run::{
    all_pass, evaluate_min_wall, resolve_min_wall, run_checks, run_checks_on, CheckSummary,
};
pub use rules::{
    expand_rule_pack, rule_pack, rule_pack_catalog, rule_packs, RulePack, RulePackRef,
    RULE_DISCLAIMER,
};
pub use types::{
    BodySel, Check, CheckAxis, CheckItem, CheckResult, CheckStatus, FitKind, Process, Severity,
    ToleranceClass,
};
