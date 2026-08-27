#!/bin/bash

if [ "$1" == "" ]; then
  if [ "$2" == "64" ]; then
    # set environment variables used by OCCT
    export CSF_FPE=0

    export TCL_DIR=""
    export TK_DIR=""
    export FREETYPE_DIR=""
    export FREEIMAGE_DIR=""
    export TBB_DIR=""
    export VTK_DIR=""
    export FFMPEG_DIR=""

    if [ "x@3RDPARTY_QT_DIR" != "x" ]; then
      export QTDIR=""
    fi

    export TCL_VERSION_WITH_DOT=""
    export TK_VERSION_WITH_DOT=""

    export CSF_OCCTBinPath="/Users/jayuda/Documents/PROJECT/DUCAD/target/debug/build/occt-sys-2bdb7d4a22ebfe6a/out/build/mac64/clang/bin"
    export CSF_OCCTLibPath="/Users/jayuda/Documents/PROJECT/DUCAD/target/debug/build/occt-sys-2bdb7d4a22ebfe6a/out/build/mac64/clang/lib"
    export CSF_OCCTIncludePath="/Users/jayuda/Documents/PROJECT/DUCAD/target/debug/build/occt-sys-2bdb7d4a22ebfe6a/out/build/inc"
    export CSF_OCCTResourcePath="/Users/jayuda/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/occt-sys-0.2.0/OCCT/src"
    export CSF_OCCTDataPath="/Users/jayuda/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/occt-sys-0.2.0/OCCT/data"
    export CSF_OCCTSamplesPath="/Users/jayuda/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/occt-sys-0.2.0/OCCT/samples"
    export CSF_OCCTTestsPath="/Users/jayuda/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/occt-sys-0.2.0/OCCT/tests"
    export CSF_OCCTDocPath="/Users/jayuda/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/occt-sys-0.2.0/OCCT/doc"

    # for compatibility with external application using CASROOT
    export CASROOT="/Users/jayuda/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/occt-sys-0.2.0/OCCT"
  fi
fi

