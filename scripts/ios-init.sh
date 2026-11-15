#!/bin/sh
# Generate the Xcode project for iOS (src-tauri/gen/apple, not committed, as
# Android's gen/android is not) and put Reach's icons in it. Run on a Mac from
# the repository root; see src-tauri/ios/README.md.
set -eu

npx tauri ios init

# init makes the project with Tauri's placeholder icon; Reach's app icons,
# named the way `tauri icon` names them, replace it.
cp src-tauri/icons/ios/*.png src-tauri/gen/apple/Assets.xcassets/AppIcon.appiconset/

echo "Xcode project ready: src-tauri/gen/apple"
