#!/usr/bin/env bash

echo "BUILD_HASH $(git rev-parse --short HEAD)$(git diff --quiet || echo '-dirty')"

