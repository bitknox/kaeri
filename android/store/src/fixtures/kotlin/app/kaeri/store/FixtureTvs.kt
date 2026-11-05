// The fixtures' TVs: two made-up TVs on the loopback interface, each a real remote server with a
// scripted player behind it (`startFakeTv`), found by a browse that lists them as DNS-SD would.
// They pair, take commands and send their state as a TV does; what they play is made up.

package app.kaeri.store

import android.util.Log
import kotlinx.coroutines.delay
import app.kaeri.engine.ffi.FfiDiscoverySink
import app.kaeri.engine.ffi.FfiEngine
import app.kaeri.engine.ffi.FfiException
import app.kaeri.engine.ffi.FfiFakeTv
import app.kaeri.engine.ffi.FfiTvBrowser
import app.kaeri.engine.ffi.FfiTvException

/** A browse that finds the fake TVs. */
class FixtureTvBrowser(val tvs: List<FfiFakeTv>) : FfiTvBrowser {
    override fun start(sink: FfiDiscoverySink) {
        for (tv in tvs) {
            val advert = tv.advert()
            sink.found(advert.instance, listOf(tv.host()), tv.port(), advert.txt)
        }
    }

    override fun stop() {}
}

object FixtureTvs {
    /** The fake TVs, Living room and Bedroom, started on [engine], and the browse that finds them. */
    fun start(engine: FfiEngine): FixtureTvBrowser? = try {
        FixtureTvBrowser(listOf(engine.startFakeTv("Living room"), engine.startFakeTv("Bedroom")))
    } catch (e: FfiException) {
        Log.w("kaeri", "the fake TVs did not start: ${e.message}")
        null
    }

    /** The QR link of the [index]th fake TV in [browser], with a code it has open: it pairs at once. */
    fun openLink(browser: FfiTvBrowser, index: Int): String? =
        (browser as? FixtureTvBrowser)?.tvs?.getOrNull(index)?.let { it.pairLink(it.openCode()) }

    /** Has the [index]th fake TV in [browser] send [url] to the phones paired with it; how many it reached. */
    fun sendLink(browser: FfiTvBrowser, index: Int, url: String): UInt =
        (browser as FixtureTvBrowser).tvs[index].sendLink(url)

    /** Stops the [index]th fake TV in [browser], as a TV going off the network. */
    fun stopTv(browser: FfiTvBrowser, index: Int) {
        (browser as FixtureTvBrowser).tvs[index].stop()
    }

    /**
     * Set up [scene] on [model], whose browse is [fakes]: Bedroom paired first in every scene, then
     * the picker, Living room's code, or Living room playing [play] with its card open, and in the lost
     * scene then gone from the network.
     */
    suspend fun stage(model: TvRemoteModel, scene: TvScene, browser: FfiTvBrowser, play: TvPlayRequest) {
        val sender = model.attachedSender ?: return
        val fakes = browser as? FixtureTvBrowser ?: return
        if (fakes.tvs.size != 2) return
        val (living, bedroom) = fakes.tvs
        try {
            sender.pair(bedroom.pairLink(bedroom.openCode())).pair(null)
            model.refresh()
            when (scene) {
                TvScene.PAIRED -> Unit
                TvScene.PICKER -> model.openPicker()
                TvScene.CODE -> {
                    model.openPicker()
                    model.begin(model.rows.value.firstOrNull { it.name == "Living room" }?.key ?: living.pairLink(null))
                }
                TvScene.REMOTE, TvScene.LOST, TvScene.PARTY -> {
                    val tv = sender.pair(living.pairLink(living.openCode())).pair(null)
                    model.refresh()
                    model.offer(play)
                    model.use(tv.fingerprint)
                    if (scene == TvScene.LOST) {
                        var waited = 0
                        while (model.card.value?.card?.now?.loading != false && waited < 100) {
                            delay(100)
                            waited++
                        }
                        living.stop()
                    }
                    // In a room of four with the chat board's conversation so far.
                    if (scene == TvScene.PARTY) living.joinParty(FixtureParty.ROOM, play.title, play.episode)
                }
            }
        } catch (e: FfiTvException) {
            Log.w("kaeri", "the TV scene did not set up: $e")
        }
    }
}
