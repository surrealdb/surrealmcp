#!/bin/bash

set -e

# Enable the GCC toolset
. /opt/rh/gcc-toolset-13/enable

# Set up environment variables for cross-compilation
export CC=/opt/rh/gcc-toolset-13/root/usr/bin/gcc
export CXX=/opt/rh/gcc-toolset-13/root/usr/bin/g++
export AR=/opt/rh/gcc-toolset-13/root/usr/bin/ar
export RANLIB=/opt/rh/gcc-toolset-13/root/usr/bin/ranlib

# Set up OpenSSL environment
export OPENSSL_DIR=/usr
export OPENSSL_LIB_DIR=/usr/lib64
export OPENSSL_INCLUDE_DIR=/usr/include

# Set up pkg-config
export PKG_CONFIG_PATH=/usr/lib64/pkgconfig

# Build the project
exec cargo build "$@"
