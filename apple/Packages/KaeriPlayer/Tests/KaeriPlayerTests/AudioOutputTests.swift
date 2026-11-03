// Which audio output mpv plays through on the Mac. Core Audio takes each sample as the device
// asks for it, so mpv's volume is heard at once. AVFoundation's renderer reads two seconds ahead,
// so a volume change on it is heard that late. apple/patches/mpv/0005 lets Core Audio start on
// macOS 27, which refuses the channel map mpv 0.41 set.

#if os(macOS)
import CoreAudio
import KaeriPlayer
import Foundation
import Testing

extension PlayerTests {
    @Suite struct AudioOutputTests {
        /// Whether the Mac has an output device; a runner with none plays through no Core Audio.
        static var hasOutputDevice: Bool {
            var device = AudioDeviceID(kAudioObjectUnknown)
            var size = UInt32(MemoryLayout<AudioDeviceID>.size)
            var address = AudioObjectPropertyAddress(
                mSelector: kAudioHardwarePropertyDefaultOutputDevice,
                mScope: kAudioObjectPropertyScopeGlobal,
                mElement: kAudioObjectPropertyElementMain)
            let status = AudioObjectGetPropertyData(
                AudioObjectID(kAudioObjectSystemObject), &address, 0, nil, &size, &device)
            return status == noErr && device != kAudioObjectUnknown
        }

        /// mpv on its own audio output, muted so nothing is heard, plays through Core Audio.
        @Test(.enabled(if: hasOutputDevice, "no audio output device"))
        func playsThroughCoreAudio() async throws {
            let (client, _) = try await startedClient { configuration in
                configuration.options["ao"] = nil
                configuration.options["mute"] = "yes"
                configuration.options["volume"] = "0"
            }
            try await play(client, "episode.mkv")
            var output = ""
            for _ in 0..<100 {
                output = (try? await client.get("current-ao").string) ?? ""
                if !output.isEmpty { break }
                try await Task.sleep(for: .milliseconds(20))
            }
            #expect(output == "coreaudio")
            await client.shutdown()
        }
    }
}
#endif
