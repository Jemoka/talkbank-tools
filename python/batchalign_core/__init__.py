from pkgutil import extend_path

# Re-export everything from the native Rust extension module.
# When tests/typecheck run from the repository root, this source package shadows
# the installed wheel. Extend the package search path so Python can still find
# the wheel-installed extension module under site-packages.
__path__ = extend_path(__path__, __name__)

from .batchalign_core import *  # noqa: F401,F403
