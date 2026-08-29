// src/color.rs
use pyo3::exceptions::PyTypeError;
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyTuple};

#[pyclass(module = "kagra.kagra_core")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Color {
    #[pyo3(get, set)]
    pub r: u8,
    #[pyo3(get, set)]
    pub g: u8,
    #[pyo3(get, set)]
    pub b: u8,
    #[pyo3(get, set)]
    pub a: u8,
}

#[pymethods]
impl Color {
    #[new]
    #[pyo3(signature = (r, g, b, a=255))]
    pub const fn py_new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    #[staticmethod]
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    #[staticmethod]
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub fn to_tuple(&self) -> (u8, u8, u8, u8) {
        (self.r, self.g, self.b, self.a)
    }

    fn __iter__(slf: PyRef<'_, Self>) -> PyResult<Py<PyAny>> {
        let py = slf.py();
        let tuple = PyTuple::new_bound(py, [slf.r, slf.g, slf.b, slf.a]);
        Ok(tuple.into_any().unbind())
    }

    fn __repr__(&self) -> String {
        format!("Color({}, {}, {}, {})", self.r, self.g, self.b, self.a)
    }
}

impl Color {
    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub const fn rgb_const(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    pub const fn rgba_const(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub fn to_f32(&self) -> [f32; 4] {
        [
            self.r as f32 / 255.0,
            self.g as f32 / 255.0,
            self.b as f32 / 255.0,
            self.a as f32 / 255.0,
        ]
    }

    pub fn to_f64(&self) -> [f64; 4] {
        [
            self.r as f64 / 255.0,
            self.g as f64 / 255.0,
            self.b as f64 / 255.0,
            self.a as f64 / 255.0,
        ]
    }

    pub fn from_py_any(value: &Bound<'_, PyAny>, default_alpha: u8) -> PyResult<Self> {
        if let Ok(color) = value.extract::<Color>() {
            return Ok(color);
        }

        if let Ok((r, g, b)) = value.extract::<(u8, u8, u8)>() {
            return Ok(Self::new(r, g, b, default_alpha));
        }

        if let Ok((r, g, b, a)) = value.extract::<(u8, u8, u8, u8)>() {
            return Ok(Self::new(r, g, b, a));
        }

        if let Ok(gray) = value.extract::<u8>() {
            return Ok(Self::new(gray, gray, gray, default_alpha));
        }

        Err(PyTypeError::new_err(
            "color must be kagra.Color, (r,g,b), (r,g,b,a), or grayscale int",
        ))
    }

    pub fn from_optional_py_any(
        value: Option<&Bound<'_, PyAny>>,
        default: Self,
    ) -> PyResult<Self> {
        match value {
            Some(v) => Self::from_py_any(v, default.a),
            None => Ok(default),
        }
    }
}

impl Default for Color {
    fn default() -> Self {
        Self::rgb_const(255, 255, 255)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_f32_normalizes() {
        let c = Color::new(255, 128, 0, 255);
        let f = c.to_f32();
        assert!((f[0] - 1.0).abs() < 1e-6);
        assert!((f[1] - 128.0 / 255.0).abs() < 1e-6);
        assert!((f[2] - 0.0).abs() < 1e-6);
        assert!((f[3] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn default_is_white() {
        assert_eq!(Color::default(), Color::new(255, 255, 255, 255));
    }

    #[test]
    fn from_rgb_tuple_sets_alpha() {
        let c: Color = (10, 20, 30).into();
        assert_eq!(c, Color::new(10, 20, 30, 255));
    }
}

impl From<(u8, u8, u8)> for Color {
    fn from(value: (u8, u8, u8)) -> Self {
        Self::new(value.0, value.1, value.2, 255)
    }
}

impl From<(u8, u8, u8, u8)> for Color {
    fn from(value: (u8, u8, u8, u8)) -> Self {
        Self::new(value.0, value.1, value.2, value.3)
    }
}