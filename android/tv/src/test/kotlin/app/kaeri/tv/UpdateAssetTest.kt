package app.kaeri.tv

import org.junit.Assert.assertEquals
import org.junit.Test
import app.kaeri.engine.model.Release
import app.kaeri.engine.model.ReleaseAsset

class UpdateAssetTest {
    private fun asset(name: String) = ReleaseAsset(name, "https://example.invalid/$name", "ab".repeat(32), 1uL)

    private val release = Release(
        version = "0.10.3",
        publishedAt = 0,
        pageUrl = "https://example.invalid/release",
        notes = "",
        androidTv = asset("kaeri-0.10.3-android-tv-arm64.apk"),
        androidTvArmv7 = asset("kaeri-0.10.3-android-tv-armv7.apk"),
    )

    @Test
    fun a32BitSystemTakesTheArmv7File() {
        val abis = arrayOf("armeabi-v7a", "armeabi")
        assertEquals("kaeri-0.10.3-android-tv-armv7.apk", tvUpdateAsset(release, abis)?.name)
    }

    @Test
    fun a64BitSystemTakesTheArm64File() {
        val abis = arrayOf("arm64-v8a", "armeabi-v7a", "armeabi")
        assertEquals("kaeri-0.10.3-android-tv-arm64.apk", tvUpdateAsset(release, abis)?.name)
    }
}
