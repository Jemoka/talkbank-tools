# Top-level Makefile — convenience entrypoints for deploy + ops.
#
# The canonical build system is Bazel + Cargo; this Makefile is *not*
# a substitute. It exists for:
#
#   make deploy HOST=<inventory-host>   # ship batchalign3 via Ansible
#   make deploy-check HOST=<host>       # ansible --check (dry run)
#   make help                           # list targets
#
# For day-to-day development, use:
#   bazel build //...
#   bazel test //...
#   uv run --no-sync batchalign3 <command>

ANSIBLE      ?= ansible-playbook
ANSIBLE_OPTS ?=
PLAYBOOK     := deploy/ansible/site.yml

HOST    ?=
VERSION ?= HEAD

.PHONY: help deploy deploy-check

help:
	@awk 'BEGIN{FS=":.*##"} /^[a-zA-Z_-]+:.*##/ {printf "  %-18s %s\n", $$1, $$2}' $(MAKEFILE_LIST)

deploy: ## Ship batchalign3 to HOST (set HOST=<addr>, optional VERSION=<sha>)
	@if [ -z "$(HOST)" ]; then echo "HOST=<addr> required" >&2; exit 1; fi
	$(ANSIBLE) -i "$(HOST)," $(PLAYBOOK) \
	    -e batchalign_version=$(VERSION) $(ANSIBLE_OPTS)

deploy-check: ## Dry-run the deploy against HOST
	@if [ -z "$(HOST)" ]; then echo "HOST=<addr> required" >&2; exit 1; fi
	$(ANSIBLE) -i "$(HOST)," $(PLAYBOOK) \
	    -e batchalign_version=$(VERSION) --check $(ANSIBLE_OPTS)
