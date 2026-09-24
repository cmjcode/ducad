pub mod commands;
pub mod solver;
pub mod types;

#[cfg(test)]
mod tests;

pub use commands::{AddConstraint, RemoveConstraint, UpdateConstraint};
pub use solver::{
    analyze_dof, constraint_is_resolvable, solve, ConstraintState, DofReport, SolveResult,
};
pub use types::{point_ref_position, Constraint, PointRef};
