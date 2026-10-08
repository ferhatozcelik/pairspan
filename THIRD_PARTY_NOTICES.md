# Third-party notices

Source code in the following directories is vendored from third-party projects
and keeps its original license headers. Packages were renamed where noted.

| Path (under `android/app/src/main/java`) | Origin | License |
|---|---|---|
| `com/pairspan/app/adb/` | libadb-android 3.1.1 (package renamed, pairing code removed, BouncyCastle `Base64` replaced with `java.util.Base64`) | Apache-2.0 (used under the Apache-2.0 option of GPL-3.0-or-later OR Apache-2.0); contains files under BSD-3-Clause and MIT as noted in each file header |
| `com/journeyapps/barcodescanner/`, `com/google/zxing/client/android/`, `com/google/zxing/integration/android/IntentResult.java` | zxing-android-embedded 4.3.0 | Apache-2.0 |
| `com/google/zxing/` (core) | ZXing core 3.4.1 | Apache-2.0 |

Resources `res/**/zxing_*` and `res/raw/zxing_beep.ogg` come from zxing-android-embedded 4.3.0 (Apache-2.0).

Apache License 2.0: https://www.apache.org/licenses/LICENSE-2.0
