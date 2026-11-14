// A Kavita OPDS link pasted as an address, https://host/api/opds/{apiKey}, and a plugin store's
// invite link. Both are recognised on paste, locally, so the field can show the host and a chip
// with the last four characters before the engine answers; the secret part never shows.

package app.kaeri.store

/**
 * A recognised Kavita link: the server's [root] (the link with `/api/opds/…` cut off, as the
 * engine's `kavita_link` gives it), its [host], and the key's last four characters for the chip.
 */
data class KavitaLink(val root: String, val host: String, val last4: String) {
    companion object {
        private val keyChars = Regex("[A-Za-z0-9_-]{8,128}")

        /**
         * The link in [text], or null when it is anything else. Mirrors the engine: a link without
         * a scheme is taken as https, the path may sit under a reverse proxy's prefix, `api/opds`
         * in any case, a key of 8 to 128 letters, digits, `-` or `_`, and no user in the address.
         */
        fun parse(text: String): KavitaLink? {
            val typed = text.trim()
            if (typed.isEmpty() || typed.any { it.isWhitespace() }) return null
            val withScheme = if (typed.startsWith("http://") || typed.startsWith("https://")) typed else "https://$typed"
            val uri = runCatching { java.net.URI(withScheme) }.getOrNull() ?: return null
            val host = uri.host?.lowercase() ?: return null
            if (uri.rawUserInfo != null) return null
            val segments = (uri.rawPath ?: "").split('/').drop(1)
            val at = segments.windowed(2).indexOfFirst {
                it[0].equals("api", ignoreCase = true) && it[1].equals("opds", ignoreCase = true)
            }
            if (at < 0) return null
            val key = segments.getOrNull(at + 2) ?: return null
            if (!keyChars.matches(key)) return null
            val root = buildString {
                append(uri.scheme.lowercase()).append("://").append(host)
                if (uri.port != -1) append(':').append(uri.port)
                segments.take(at).filter { it.isNotEmpty() }.forEach { append('/').append(it) }
            }
            return KavitaLink(root, host, key.takeLast(4))
        }
    }
}

/** A plugin store's invite link as pasted: its host for the field, and the code's last four for the chip. */
data class InviteChip(val host: String, val last4: String) {
    companion object {
        /** The invite in [text] (`https://host/join/{code}#{key}`, or the app's own link to it), or null. */
        fun parse(text: String): InviteChip? {
            val link = InviteLinks.invite(text) ?: return null
            val uri = runCatching { java.net.URI(link) }.getOrNull() ?: return null
            val host = uri.host?.lowercase() ?: return null
            val code = (uri.rawPath ?: "").substringAfter("/join/", "").trim('/').substringBefore('/')
            if (code.length < 4) return null
            return InviteChip(host, code.takeLast(4).uppercase())
        }
    }
}
