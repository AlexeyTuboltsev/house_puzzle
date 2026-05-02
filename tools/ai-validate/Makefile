# tools/ai-validate/Makefile — local build helpers.
#
# CI is the source of truth for shipping artifacts (see
# ../../.github/workflows/release-ai-validate.yml). These targets exist
# for local smoke-testing before pushing a tag.

VERSION := $(shell tr -d '[:space:]' < packaging/version.txt)
DIST    := dist
BUNDLE  := $(DIST)/ai-validate-$(VERSION).jsx
PKG     := $(DIST)/ai-validate-$(VERSION).pkg

.PHONY: bundle pkg-mac clean version

bundle:
	./packaging/bundle.sh

pkg-mac: bundle
	./packaging/macos/build-pkg.sh

# Windows .exe build needs Inno Setup, which doesn't run on macOS.
# CI handles it — no local target.

version:
	@echo $(VERSION)

clean:
	rm -rf $(DIST)
