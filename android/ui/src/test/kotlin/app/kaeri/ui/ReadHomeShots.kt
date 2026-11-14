package app.kaeri.ui

import app.kaeri.engine.model.KaeriIsFor
import app.kaeri.store.FixtureScene
import org.junit.Test
import org.robolectric.annotation.Config

/** Reading's Home: Read beside Watch, Home with Read alone, and the tablet's. */
class ReadHomeShots : ReadingShotHarness() {
    @Test
    @Config(qualifiers = ReadingShotHarness.PHONE)
    fun phone_read_home() = shoot("phone-read-home", FixtureScene.READING_HOME, Destination.Read) { it.reading.home.value != null }

    @Test
    @Config(qualifiers = ReadingShotHarness.PHONE)
    fun phone_read_alone_home() = shoot("phone-read-alone-home", FixtureScene.READING_HOME, Destination.Home, medium = KaeriIsFor.READ) {
        it.reading.home.value != null
    }

    @Test
    @Config(qualifiers = ReadingShotHarness.TABLET)
    fun tablet_read_home() = shoot("tablet-read-home", FixtureScene.READING_HOME, Destination.Read) { it.reading.home.value != null }
}
