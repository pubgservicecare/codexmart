//! Pure business rules. Nothing in this module touches the database, so
//! every rule can be unit-tested and reused by a future server.

pub mod costing;
pub mod payments;
pub mod pricing;
pub mod validation;
