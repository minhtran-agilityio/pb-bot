# qa-verifier fixture

`clean/tally.sh` is a small shell CLI that meets three acceptance criteria. `broken/tally.sh` is the
same file with `mul` changed so that it adds: AC2 fails. They were inputs to manual qa-verifier evals,
which have been removed. Nothing in the suite runs them.
