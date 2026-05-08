.PHONY: check-fast check-push check-deep coverage crap mutation install-hooks

check-fast:
	scripts/quality/fast.sh

check-push:
	scripts/quality/push.sh

check-deep:
	scripts/quality/deep.sh

coverage:
	scripts/quality/coverage.sh

crap:
	scripts/quality/crap.sh

mutation:
	scripts/quality/mutation.sh

install-hooks:
	scripts/quality/install-hooks.sh
