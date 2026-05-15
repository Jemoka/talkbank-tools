//! Error path tests — verify graceful failures for bad inputs.
//!
//! Most tests in this module use direct local execution with real workers so
//! model-hitting failures are exercised without the server control plane. The
//! malformed-request coverage stays on the HTTP path because it is explicitly a
//! request-validation test.

mod request_boundary;
