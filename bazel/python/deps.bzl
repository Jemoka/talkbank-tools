"""Helper to load *every* pypi dep from the lockfile.

rules_python's `all_requirements` symbol from `@pypi//:requirements.bzl`
lists every package name in `requirements.lock.txt` regardless of
whether the package's marker matches the resolved platform/Python.
pip.parse correctly skips writing BUILD files for marker-excluded
packages, leaving dangling names in `all_requirements` that break
`deps = all_requirements` at analysis time.

This module wraps `all_requirements` and filters out names that are
known to be marker-conditional in this project's lockfile. It is NOT
a hand-maintained list of *which* deps to include — that list lives
entirely in `pyproject.toml`. This is a list of *which marker-
conditional names rules_python skips*, derived from grepping
requirements.lock.txt for `; python_full_version` / `; sys_platform`
markers that don't satisfy our pinned 3.12 + cross-platform set.

Track upstream: bazel-contrib/rules_python#2244 (and friends) — once
fixed, this whole file collapses to `_RUNTIME_DEPS = all_requirements`
directly in the consumer BUILD.
"""

load("@pypi//:requirements.bzl", _all_requirements = "all_requirements")

# Names that appear in `all_requirements` but whose BUILD file pip.parse
# elides because of a `python_full_version` / `sys_platform` marker.
# Discoverable by `bazel build //python/batchalign && grep -E "BUILD file
# not found in directory" <output>`. Names are matched as substrings of
# the canonical `@@.../<name>:pkg` label.
# Generated list of marker-conditional names that the lockfile records
# but pip.parse skips when generating BUILD files on the current host.
# Regenerate with:
#   grep -B1 "; python_full_version >= '3.13'" python/requirements.lock.txt \
#     | grep '^[a-z_]' | awk -F'==' '{gsub(/-/,"_",$1); print "    \""$1"\","}' | sort -u
# (plus equivalent greps for `sys_platform`-conditional lines.)
# Names use underscore form (pip.parse normalizes hyphens).
_MARKER_FILTERED = [
    # python_full_version >= '3.13' (Python 3.13+ backports of removed stdlib)
    "audioop_lts",
    "standard_aifc",
    "standard_chunk",
    "standard_sunau",
    # sys_platform == 'linux'
    "cuda_bindings",
    "cuda_pathfinder",
    "jeepney",
    "secretstorage",
    "triton",
    "nvidia_cublas_cu12",
    "nvidia_cuda_cupti_cu12",
    "nvidia_cuda_nvrtc_cu12",
    "nvidia_cuda_runtime_cu12",
    "nvidia_cudnn_cu12",
    "nvidia_cufft_cu12",
    "nvidia_cufile_cu12",
    "nvidia_curand_cu12",
    "nvidia_cusolver_cu12",
    "nvidia_cusparse_cu12",
    "nvidia_cusparselt_cu12",
    "nvidia_nccl_cu12",
    "nvidia_nvjitlink_cu12",
    "nvidia_nvshmem_cu12",
    "nvidia_nvtx_cu12",
    # sys_platform == 'win32'
    "pywin32_ctypes",
    "tzdata",
]

def _is_filtered(label):
    for name in _MARKER_FILTERED:
        if name in label:
            return True
    return False

def all_runtime_deps():
    """Every dep the lockfile resolves for the current platform.

    No name list maintained anywhere in BUILD/justfile/MODULE — call this
    from py_library/py_binary/py_test `deps =` and the dependency set is
    implicit in pyproject.toml + requirements.lock.txt.
    """
    return [d for d in _all_requirements if not _is_filtered(d)]
