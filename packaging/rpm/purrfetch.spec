# DORMANT PROTOTYPE: deferred pending demand and end-to-end validation.
# This recipe may be stale and is not part of release automation.

%global bin_name purr

Name:           purrfetch
# Overridden by `.copr/Makefile` (--define version <tag>); defaults here so the
# spec lints/builds standalone.
Version:        %{?version}%{!?version:1.0.0}
Release:        1%{?dist}
Summary:        Fast, neofetch-compatible system information tool

License:        MIT
URL:            https://github.com/justin13888/purrfetch
Source0:        %{name}-%{version}.tar.gz
# Vendored crates, so the build runs fully offline in mock. Produced by
# `.copr/Makefile`.
Source1:        %{name}-vendor-%{version}.tar.gz

BuildRequires:  cargo
BuildRequires:  rust
BuildRequires:  gcc

# The binary embeds its ASCII art and build metadata at compile time and carries
# no separate debug info from the release profile, so skip the debuginfo
# subpackage (it would otherwise be empty and fail the build).
%global debug_package %{nil}

%description
purr is a fast, neofetch-compatible system information tool written in Rust. It
prints a neofetch-style report — the same fields, styling, and ${c1}..${c6}
ASCII logo format — with probes running in parallel. The installed command is
%{bin_name}.

%prep
%autosetup -n %{name}-%{version}
# Unpack the vendored crates and point cargo at them for an offline build.
tar -xf %{SOURCE1}
mkdir -p .cargo
cat > .cargo/config.toml <<'EOF'
[source.crates-io]
replace-with = "vendored-sources"

[source.vendored-sources]
directory = "vendor"
EOF

%build
cargo build --release --offline --locked

%install
install -Dm0755 target/release/%{bin_name} %{buildroot}%{_bindir}/%{bin_name}
strip %{buildroot}%{_bindir}/%{bin_name}
install -Dm0644 man/%{bin_name}.1 %{buildroot}%{_mandir}/man1/%{bin_name}.1
install -Dm0644 completions/%{bin_name}.bash %{buildroot}%{_datadir}/bash-completion/completions/%{bin_name}
install -Dm0644 completions/_%{bin_name} %{buildroot}%{_datadir}/zsh/site-functions/_%{bin_name}
install -Dm0644 completions/%{bin_name}.fish %{buildroot}%{_datadir}/fish/vendor_completions.d/%{bin_name}.fish

%check
./target/release/%{bin_name} --version

%files
%license LICENSE
%doc README.md
%{_bindir}/%{bin_name}
%{_mandir}/man1/%{bin_name}.1*
%{_datadir}/bash-completion/completions/%{bin_name}
%{_datadir}/zsh/site-functions/_%{bin_name}
%{_datadir}/fish/vendor_completions.d/%{bin_name}.fish

%changelog
* Tue Jun 23 2026 justin13888 <20733699+justin13888@users.noreply.github.com> - 1.0.0-1
- Initial COPR packaging.
