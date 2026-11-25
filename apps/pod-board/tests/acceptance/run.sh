#!/bin/bash
# Run the Pod Board UDS acceptance tests (issue #136). Needs python3; the page checks also need Chrome/Chromium.
exec python3 -I "$(dirname "$0")/test_uds_adoption.py" "$@"
