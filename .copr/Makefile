# DORMANT PROTOTYPE: deferred pending demand and end-to-end validation. This
# recipe may be stale and is not part of release automation.
#
# Produces a source RPM for Fedora COPR from the current git checkout.
#
# COPR's "Custom" / SCM build method runs `make -f .copr/Makefile srpm
# outdir=<dir>` in a container WITH network access. We vendor all crates here so
# the resulting SRPM builds fully offline in mock. See packaging/rpm/purrfetch.spec.
#
# Self-verify locally:
#   podman run --rm --user 0 -v "$PWD":/w:Z -w /w fedora:latest \
#     make -f .copr/Makefile srpm outdir=/tmp

NAME    := purrfetch
SPEC    := packaging/rpm/$(NAME).spec
# Version from the latest git tag (strip leading v), else the Cargo.toml version.
# `grep .` forces a non-empty result so the `||` fallback fires when there is no
# tag (an empty `git describe` otherwise slips through the `sed`).
VERSION := $(shell git describe --tags --abbrev=0 2>/dev/null | sed 's/^v//' | grep . || \
                   grep -m1 '^version' Cargo.toml | cut -d'"' -f2)
TOPDIR  := $(CURDIR)/.copr/rpmbuild
SOURCES := $(TOPDIR)/SOURCES

.PHONY: srpm
srpm:
	dnf -y install cargo rust rpm-build git tar
	mkdir -p $(SOURCES) $(outdir)
	# Source0: the committed tree as $(NAME)-$(VERSION)/
	git archive --format=tar.gz --prefix=$(NAME)-$(VERSION)/ \
	  -o $(SOURCES)/$(NAME)-$(VERSION).tar.gz HEAD
	# Source1: vendored crates (needs network, available in the srpm step)
	cargo vendor --versioned-dirs vendor
	tar -czf $(SOURCES)/$(NAME)-vendor-$(VERSION).tar.gz vendor
	rpmbuild -bs $(SPEC) \
	  --define "_topdir $(TOPDIR)" \
	  --define "version $(VERSION)"
	cp $(TOPDIR)/SRPMS/*.src.rpm $(outdir)/
