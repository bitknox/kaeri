package app.kaeri.store

import java.nio.file.Files
import java.util.Locale
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * The engine starts in the language the app shows, so its own words, reading's chapter groups and
 * the TV's lines come in it too. Without a context at hand, that is the process's language.
 */
class EngineLanguageTest {
    private val before = Locale.getDefault()

    @After
    fun restore() = Locale.setDefault(before)

    @Test
    fun theEngineStartsInTheLanguageTheProcessRunsIn() {
        val home = EngineHome.override(Files.createTempDirectory("kaeri-language").toFile())
        val record = { EngineLaunch(engine = EngineLaunch.Engine.App, home = home, platform = null, pageDecoder = null).record }
        Locale.setDefault(Locale.GERMANY)
        assertEquals("de", record().language)
        Locale.setDefault(Locale.forLanguageTag("pt-BR"))
        assertEquals("pt-BR", record().language)
        Locale.setDefault(Locale.forLanguageTag("ko-KR"))
        assertEquals("a language the app lacks starts in English", "en", record().language)
    }
}
