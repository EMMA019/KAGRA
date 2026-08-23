// src/lib.rs
use pyo3::prelude::*;

mod error;
mod color;
mod input;
mod audio;
mod text;
mod window;
mod instance_renderer;
mod boids;
mod boids_gpu;
mod fbx_loader;
mod renderer;
mod vrm;
mod rig;
mod gltf;
mod gltf_common;
mod camera;
mod pick;
mod vrm_humanoid;
mod vrm_lookat_meta;
mod vrm_expression;
mod vrm_constraint;
mod vrm_first_person;
mod vrm_spring;
mod mtoon;

mod engine;

pub use engine::Engine;

#[pymodule]
fn kagra_core(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Engine>()?;
    Ok(())
}