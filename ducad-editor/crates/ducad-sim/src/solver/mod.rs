//! Solver linier jarang.

pub mod cg;
pub mod coarse;
pub mod eigen;

pub use cg::{solve_pcg, BlockJacobi3, CgOptions, CgStats, Jacobi, Preconditioner};
pub use coarse::TwoLevel;
pub use eigen::{lobpcg, EigenOptions, EigenResult, LinOp, NodeScalarOp};
