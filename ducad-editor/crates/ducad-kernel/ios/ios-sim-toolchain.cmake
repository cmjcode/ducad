# Toolchain CMake untuk cross-compile occt-sys (OCCT) ke aarch64-apple-ios-sim (iPad Simulator).

set(CMAKE_SYSTEM_NAME iOS)
set(CMAKE_SYSTEM_PROCESSOR arm64)

# Path SDK iPhoneSimulator
set(CMAKE_OSX_SYSROOT "/Applications/Xcode.app/Contents/Developer/Platforms/iPhoneSimulator.platform/Developer/SDKs/iPhoneSimulator26.5.sdk" CACHE PATH "iOS Simulator SDK sysroot" FORCE)
set(CMAKE_OSX_ARCHITECTURES arm64 CACHE STRING "iOS arch" FORCE)
set(CMAKE_OSX_DEPLOYMENT_TARGET 15.0)

set(CMAKE_XCODE_ATTRIBUTE_ENABLE_BITCODE NO)
set(CMAKE_C_USE_RESPONSE_FILE_FOR_ARCHIVES 1)
set(CMAKE_CXX_USE_RESPONSE_FILE_FOR_ARCHIVES 1)
set(CMAKE_C_FLAGS "${CMAKE_C_FLAGS} -fno-stack-check")
set(CMAKE_CXX_FLAGS "${CMAKE_CXX_FLAGS} -fno-stack-check")
