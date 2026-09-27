//! Tono's reusable practice-video engine.
//!
//! Frontends construct [`pipeline::prep::PrepOptions`] and call
//! [`pipeline::output::run`] for staged MP4 publication, or
//! [`pipeline::prep::run_prep`] for a project-directory output.
//! CLI parsing belongs to the binary, not this library.

pub mod analysis;
pub mod commands;
pub mod instruments;
pub mod media;
pub mod music;
pub mod paths;
pub mod pipeline;
pub mod render;

pub mod runtime;
