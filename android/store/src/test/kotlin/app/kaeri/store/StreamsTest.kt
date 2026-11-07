package app.kaeri.store

import java.io.ByteArrayInputStream
import java.io.InputStream
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Test

class StreamsTest {
    private fun bytes(count: Int) = ByteArray(count) { it.toByte() }

    /** A stream that hands out at most [step] bytes a read, as a content provider's pipe can. */
    private class Trickle(private val data: ByteArray, private val step: Int) : InputStream() {
        private var at = 0
        override fun read(): Int = if (at < data.size) data[at++].toInt() and 0xff else -1
        override fun read(b: ByteArray, off: Int, len: Int): Int {
            if (at >= data.size) return -1
            val n = minOf(len, step, data.size - at)
            data.copyInto(b, off, at, at + n)
            at += n
            return n
        }
    }

    @Test
    fun a_shorter_stream_is_read_whole() {
        assertArrayEquals(bytes(10), ByteArrayInputStream(bytes(10)).readAtMost(100))
    }

    @Test
    fun a_longer_stream_is_read_up_to_the_limit_and_no_further() {
        val stream = ByteArrayInputStream(bytes(200_000))
        assertArrayEquals(bytes(150_001), stream.readAtMost(150_001))
        assertEquals(200_000 - 150_001, stream.available())
    }

    @Test
    fun short_reads_are_gathered_until_the_limit() {
        assertArrayEquals(bytes(1000), Trickle(bytes(5000), step = 7).readAtMost(1000))
    }

    @Test
    fun a_limit_of_nothing_reads_nothing() {
        assertEquals(0, ByteArrayInputStream(bytes(3)).readAtMost(0).size)
    }
}
