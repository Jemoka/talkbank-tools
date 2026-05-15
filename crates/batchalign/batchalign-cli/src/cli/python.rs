//! Python runtime resolution — re-exports from the worker crate.
//!
//! The canonical implementation lives in `crate::worker::python`.

pub use crate::worker::python::resolve_python_executable;

#[cfg(test)]
mod tests {
    // Tests live in the worker crate now.
    #[test]
    fn reexport_works() {
        // Just verify the reexport compiles and returns something.
        let path = super::resolve_python_executable();
        assert!(!path.is_empty());
    }
}
