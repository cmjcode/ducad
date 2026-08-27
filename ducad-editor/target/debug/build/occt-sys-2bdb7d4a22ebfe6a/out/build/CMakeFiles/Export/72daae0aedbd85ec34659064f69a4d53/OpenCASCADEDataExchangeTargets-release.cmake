#----------------------------------------------------------------
# Generated CMake target import file for configuration "Release".
#----------------------------------------------------------------

# Commands may need to know the format version.
set(CMAKE_IMPORT_FILE_VERSION 1)

# Import target "TKXSBase" for configuration "Release"
set_property(TARGET TKXSBase APPEND PROPERTY IMPORTED_CONFIGURATIONS RELEASE)
set_target_properties(TKXSBase PROPERTIES
  IMPORTED_LINK_INTERFACE_LANGUAGES_RELEASE "CXX"
  IMPORTED_LOCATION_RELEASE "${_IMPORT_PREFIX}/lib\${OCCT_INSTALL_BIN_LETTER}/libTKXSBase.a"
  )

list(APPEND _cmake_import_check_targets TKXSBase )
list(APPEND _cmake_import_check_files_for_TKXSBase "${_IMPORT_PREFIX}/lib\${OCCT_INSTALL_BIN_LETTER}/libTKXSBase.a" )

# Import target "TKSTEPBase" for configuration "Release"
set_property(TARGET TKSTEPBase APPEND PROPERTY IMPORTED_CONFIGURATIONS RELEASE)
set_target_properties(TKSTEPBase PROPERTIES
  IMPORTED_LINK_INTERFACE_LANGUAGES_RELEASE "CXX"
  IMPORTED_LOCATION_RELEASE "${_IMPORT_PREFIX}/lib\${OCCT_INSTALL_BIN_LETTER}/libTKSTEPBase.a"
  )

list(APPEND _cmake_import_check_targets TKSTEPBase )
list(APPEND _cmake_import_check_files_for_TKSTEPBase "${_IMPORT_PREFIX}/lib\${OCCT_INSTALL_BIN_LETTER}/libTKSTEPBase.a" )

# Import target "TKSTEPAttr" for configuration "Release"
set_property(TARGET TKSTEPAttr APPEND PROPERTY IMPORTED_CONFIGURATIONS RELEASE)
set_target_properties(TKSTEPAttr PROPERTIES
  IMPORTED_LINK_INTERFACE_LANGUAGES_RELEASE "CXX"
  IMPORTED_LOCATION_RELEASE "${_IMPORT_PREFIX}/lib\${OCCT_INSTALL_BIN_LETTER}/libTKSTEPAttr.a"
  )

list(APPEND _cmake_import_check_targets TKSTEPAttr )
list(APPEND _cmake_import_check_files_for_TKSTEPAttr "${_IMPORT_PREFIX}/lib\${OCCT_INSTALL_BIN_LETTER}/libTKSTEPAttr.a" )

# Import target "TKSTEP209" for configuration "Release"
set_property(TARGET TKSTEP209 APPEND PROPERTY IMPORTED_CONFIGURATIONS RELEASE)
set_target_properties(TKSTEP209 PROPERTIES
  IMPORTED_LINK_INTERFACE_LANGUAGES_RELEASE "CXX"
  IMPORTED_LOCATION_RELEASE "${_IMPORT_PREFIX}/lib\${OCCT_INSTALL_BIN_LETTER}/libTKSTEP209.a"
  )

list(APPEND _cmake_import_check_targets TKSTEP209 )
list(APPEND _cmake_import_check_files_for_TKSTEP209 "${_IMPORT_PREFIX}/lib\${OCCT_INSTALL_BIN_LETTER}/libTKSTEP209.a" )

# Import target "TKSTEP" for configuration "Release"
set_property(TARGET TKSTEP APPEND PROPERTY IMPORTED_CONFIGURATIONS RELEASE)
set_target_properties(TKSTEP PROPERTIES
  IMPORTED_LINK_INTERFACE_LANGUAGES_RELEASE "CXX"
  IMPORTED_LOCATION_RELEASE "${_IMPORT_PREFIX}/lib\${OCCT_INSTALL_BIN_LETTER}/libTKSTEP.a"
  )

list(APPEND _cmake_import_check_targets TKSTEP )
list(APPEND _cmake_import_check_files_for_TKSTEP "${_IMPORT_PREFIX}/lib\${OCCT_INSTALL_BIN_LETTER}/libTKSTEP.a" )

# Import target "TKSTL" for configuration "Release"
set_property(TARGET TKSTL APPEND PROPERTY IMPORTED_CONFIGURATIONS RELEASE)
set_target_properties(TKSTL PROPERTIES
  IMPORTED_LINK_INTERFACE_LANGUAGES_RELEASE "CXX"
  IMPORTED_LOCATION_RELEASE "${_IMPORT_PREFIX}/lib\${OCCT_INSTALL_BIN_LETTER}/libTKSTL.a"
  )

list(APPEND _cmake_import_check_targets TKSTL )
list(APPEND _cmake_import_check_files_for_TKSTL "${_IMPORT_PREFIX}/lib\${OCCT_INSTALL_BIN_LETTER}/libTKSTL.a" )

# Commands beyond this point should not need to know the version.
set(CMAKE_IMPORT_FILE_VERSION)
