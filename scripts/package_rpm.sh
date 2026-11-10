#!/usr/bin/env bash
set -e

# Change directory to the workspace root
cd "$(dirname "$0")/.."

# Check if rpmbuild is installed
if ! command -v rpmbuild &> /dev/null; then
    echo "Error: rpmbuild is not installed."
    echo "Please install it using: sudo dnf install rpm-build"
    exit 1
fi

echo "Building optimized binary..."
cargo build --profile optimized

# Extract version from Cargo.toml
VERSION=$(grep -E "^version\s*=\s*" Cargo.toml | head -n 1 | cut -d '"' -f 2)
if [ -z "$VERSION" ]; then
    echo "Error: Could not extract version from Cargo.toml"
    exit 1
fi

# Detect architecture
ARCH=$(rpm --eval '%{_arch}' 2>/dev/null || uname -m)

echo "Packaging version $VERSION for architecture $ARCH..."

# Define RPM build root directory (under target/rpm)
RPM_TOPDIR="$(pwd)/target/rpm"
rm -rf "$RPM_TOPDIR"
mkdir -p "$RPM_TOPDIR/SOURCES"
mkdir -p "$RPM_TOPDIR/SPECS"
mkdir -p "$RPM_TOPDIR/BUILD"
mkdir -p "$RPM_TOPDIR/RPMS"
mkdir -p "$RPM_TOPDIR/SRPMS"

# Copy binary to SOURCES
cp target/optimized/dr-md "$RPM_TOPDIR/SOURCES/"
chmod 755 "$RPM_TOPDIR/SOURCES/dr-md"

# Copy icons of all sizes to SOURCES
for size in 16 32 48 64 128 256 512; do
    if [ -f "assets/icons/dr-md_${size}x${size}.png" ]; then
        cp "assets/icons/dr-md_${size}x${size}.png" "$RPM_TOPDIR/SOURCES/"
    fi
done

# Write desktop entry to SOURCES
cat << 'EOF' > "$RPM_TOPDIR/SOURCES/dr-md.desktop"
[Desktop Entry]
Name=dr.md
Comment=Doctor Markdown - A sleek markdown editor
Exec=dr-md %F
Icon=dr-md
Terminal=false
Type=Application
Categories=Utility;TextEditor;Development;
MimeType=text/markdown;text/plain;
EOF
chmod 644 "$RPM_TOPDIR/SOURCES/dr-md.desktop"

# Write the RPM Spec file
cat << 'EOF' > "$RPM_TOPDIR/SPECS/dr-md.spec"
Name:           dr-md
Version:        %{version}
Release:        1%{?dist}
Summary:        Sleek, high-performance offline Markdown editor built with egui
License:        MIT
Group:          Applications/Editors
Vendor:         lnoxsian
Packager:       lnoxsian

# Disable debuginfo package generation because we are packaging a pre-compiled binary
%define debug_package %{nil}

%description
Doctor Markdown (dr.md) is a lightweight editor featuring live preview,
syntax highlighting, workspace file tree, and built-in theme support.

%install
# Create directories
mkdir -p %{buildroot}%{_bindir}
mkdir -p %{buildroot}%{_datadir}/applications
mkdir -p %{buildroot}%{_datadir}/pixmaps

# Copy binary
cp %{_sourcedir}/dr-md %{buildroot}%{_bindir}/dr-md
chmod 755 %{buildroot}%{_bindir}/dr-md

# Copy desktop file
cp %{_sourcedir}/dr-md.desktop %{buildroot}%{_datadir}/applications/dr-md.desktop
chmod 644 %{buildroot}%{_datadir}/applications/dr-md.desktop

# Copy icons of all sizes
for size in 16 32 48 64 128 256 512; do
    if [ -f "%{_sourcedir}/dr-md_${size}x${size}.png" ]; then
        mkdir -p "%{buildroot}%{_datadir}/icons/hicolor/${size}x${size}/apps"
        cp "%{_sourcedir}/dr-md_${size}x${size}.png" "%{buildroot}%{_datadir}/icons/hicolor/${size}x${size}/apps/dr-md.png"
    fi
done

# Copy fallback icon to pixmaps
if [ -f "%{_sourcedir}/dr-md_48x48.png" ]; then
    cp %{_sourcedir}/dr-md_48x48.png %{buildroot}%{_datadir}/pixmaps/dr-md.png
elif [ -f "%{_sourcedir}/dr-md_256x256.png" ]; then
    cp %{_sourcedir}/dr-md_256x256.png %{buildroot}%{_datadir}/pixmaps/dr-md.png
fi

%post
/usr/bin/update-desktop-database &> /dev/null || :
/usr/bin/gtk-update-icon-cache %{_datadir}/icons/hicolor &> /dev/null || :

%postun
/usr/bin/update-desktop-database &> /dev/null || :
/usr/bin/gtk-update-icon-cache %{_datadir}/icons/hicolor &> /dev/null || :

%files
%{_bindir}/dr-md
%{_datadir}/applications/dr-md.desktop
%{_datadir}/icons/hicolor/*/apps/dr-md.png
%{_datadir}/pixmaps/dr-md.png

%changelog
* Thu Jul 09 2026 lnoxsian - %{version}-1
- Initial release
EOF

# Build package
echo "Building RPM package..."
rpmbuild --define "_topdir $RPM_TOPDIR" --define "version $VERSION" -bb "$RPM_TOPDIR/SPECS/dr-md.spec"

# Find and copy output RPM to target/
RPM_FILE=$(find "$RPM_TOPDIR/RPMS" -name "*.rpm" -type f | head -n 1)
if [ -n "$RPM_FILE" ]; then
    cp "$RPM_FILE" target/
    echo "RPM package successfully built at: target/$(basename "$RPM_FILE")"
else
    echo "Error: RPM build completed but package file was not found."
    exit 1
fi
